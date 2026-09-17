//! Minimal host composition sketch used by the packed consumer fixture.
//! The application owns the logger and composes the supplied plugin with its
//! own `app_observability_level_change` command.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sc_observability_dto::{
    Failure, LevelChangeDto, LevelChangeRequest, LevelRequestDto, WireEnvelope,
    decode_level_request,
};
use sc_observability_tauri::{AdapterPolicy, plugin};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct OwnerState(Arc<Mutex<sc_observability_log::LogGuard>>);

fn level_envelope(result: Result<LevelChangeDto, Failure>) -> WireEnvelope<LevelChangeDto> {
    match result {
        Ok(value) => WireEnvelope::Ok {
            schema_version: 1,
            value,
        },
        Err(error) => WireEnvelope::Error {
            schema_version: 1,
            error,
        },
    }
}

fn invalid(field: &str, message: &str) -> Failure {
    Failure::Validation {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
            message,
        )),
        field: field.to_owned(),
    }
}

/// Application-owned level request. The frontend supplies no source and never
/// receives the owner; the host fixes the source to `user_request`.
#[tauri::command]
fn app_observability_level_change<R: tauri::Runtime>(
    window: tauri::Window<R>,
    request: serde_json::Value,
    state: tauri::State<'_, OwnerState>,
) -> WireEnvelope<LevelChangeDto> {
    if window.label() != "main" {
        return level_envelope(Err(Failure::PermissionDenied {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_PERMISSION_DENIED,
                "only the main window may request a level change",
            )),
        }));
    }
    let Some(object) = request.as_object() else {
        return level_envelope(Err(invalid("request", "level request must be an object")));
    };
    if object.keys().any(|key| !matches!(key.as_str(), "schema_version" | "change")) {
        return level_envelope(Err(invalid("request", "unknown level request field")));
    }
    if object.get("schema_version") != Some(&serde_json::Value::from(1u32)) {
        return level_envelope(Err(invalid("schema_version", "schema_version must be 1")));
    }
    let Some(change) = object.get("change").and_then(serde_json::Value::as_object) else {
        return level_envelope(Err(invalid("change", "change must be an object")));
    };
    if change.keys().any(|key| !matches!(key.as_str(), "kind" | "level")) {
        return level_envelope(Err(invalid("change", "unknown level change field")));
    }
    let parsed: Result<LevelChangeRequest, _> = serde_json::from_value(request);
    let Ok(request) = parsed else {
        return level_envelope(Err(Failure::Validation {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
                "level request does not match schema v1",
            )),
            field: "request".to_owned(),
        }));
    };
    let change = match serde_json::to_value(request.change) {
        Ok(value) => match decode_level_request(value) {
            Ok(change) => change,
            Err(error) => return level_envelope(Err(error)),
        },
        Err(_) => {
            return level_envelope(Err(Failure::Internal {
                diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                    "level request could not be serialized",
                )),
            }));
        }
    };
    let Ok(mut owner) = state.0.try_lock() else {
        return level_envelope(Err(Failure::QueueFull {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
                "level owner is busy",
            )),
        }));
    };
    let result: Result<LevelChangeDto, Failure> = match change {
        LevelRequestDto::Elevate { level } => owner.elevate_level(
            level.into(),
            sc_observability_types::LevelChangeSource::UserRequest,
        ).map_err(sc_observability_dto::from_level_error)
            .and_then(sc_observability_dto::from_level_change),
        LevelRequestDto::Reset {} => {
            owner
                .reset_level(sc_observability_types::LevelChangeSource::UserRequest)
                .map_err(sc_observability_dto::from_level_error)
                .and_then(sc_observability_dto::from_level_change)
        }
    };
    level_envelope(result)
}

fn main() {
    let service = match sc_observability_types::ServiceName::new("tauri-example") {
        Ok(service) => service,
        Err(error) => {
            eprintln!("invalid example service name: {error}");
            return;
        }
    };
    let config = sc_observability::LoggerConfig::default_for(service, PathBuf::from("logs"));
    let default_action = match sc_observability_types::ActionName::new("log.record") {
        Ok(action) => action,
        Err(error) => {
            eprintln!("could not configure default action: {error}");
            return;
        }
    };
    let guard = match sc_observability_log::init(
        config,
        sc_observability_log::BridgeOptions {
            default_action,
            parse_bracket_action: true,
        },
    ) {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("could not start observability host: {error:?}");
            return;
        }
    };
    let control = guard.control();
    let backend = match sc_observability_binding_runtime::bridge_backend(control.clone()) {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("could not attach observability bridge: {error:?}");
            return;
        }
    };
    let correlation_id = match sc_observability_log::CorrelationId::new("tauri-example-startup") {
        Ok(correlation_id) => correlation_id,
        Err(error) => {
            eprintln!("could not configure host event correlation: {error}");
            return;
        }
    };
    let host_event = sc_observability_log::BridgeEvent {
        level: sc_observability_log::EventLevel::Info,
        target: match sc_observability_log::TargetCategory::new("tauri-example") {
            Ok(target) => target,
            Err(error) => {
                eprintln!("could not configure host event target: {error}");
                return;
            }
        },
        action: match sc_observability_log::ActionName::new("startup.rust") {
            Ok(action) => Some(action),
            Err(error) => {
                eprintln!("could not configure host event action: {error}");
                return;
            }
        },
        message: Some("Rust host initialized".to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: Some(correlation_id),
        trace: None,
    };
    if let Err(error) = control.try_log(host_event) {
        eprintln!("could not submit host startup event: {error}");
    }
    let policy = AdapterPolicy {
        allowed_window_labels: BTreeSet::from(["main".to_owned()]),
        allowed_targets: BTreeSet::from(["tauri-example".to_owned()]),
        max_request_bytes: 65_536,
        max_depth: 32,
        redacted_field_keys: BTreeSet::from(["secret".to_owned()]),
    };
    let adapter = match plugin::<tauri::Wry>(Arc::new(backend), policy) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("could not configure observability adapter: {error:?}");
            return;
        }
    };
    let result = tauri::Builder::default()
        .manage(OwnerState(Arc::new(Mutex::new(guard))))
        .plugin(adapter)
        .invoke_handler(tauri::generate_handler![app_observability_level_change])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Tauri host exited with an error: {error}");
    }
}
