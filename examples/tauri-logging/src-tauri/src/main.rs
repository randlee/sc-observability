//! Minimal host composition sketch used by the packed consumer fixture.
//! The application owns the logger and composes the supplied plugin with its
//! own `app_observability_level_change` command.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sc_observability_binding_runtime::HostLoggingBackend;
use sc_observability_dto::{
    Failure, LevelChangeDto, LevelChangeRequest, LevelRequestDto, WireEnvelope,
    decode_level_request,
};
use sc_observability_tauri::{AdapterPolicy, plugin};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

struct OwnerState(Arc<Mutex<sc_observability_binding_runtime::CoreLoggerOwner>>);

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
    let Ok(mut owner) = state.0.lock() else {
        return level_envelope(Err(Failure::Internal {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                "level owner state is unavailable",
            )),
        }));
    };
    let result = match change {
        LevelRequestDto::Elevate { level } => owner.elevate_level(
            level.into(),
            sc_observability_types::LevelChangeSource::UserRequest,
        ),
        LevelRequestDto::Reset {} => {
            owner.reset_level(sc_observability_types::LevelChangeSource::UserRequest)
        }
    };
    level_envelope(result)
}

fn main() {
    // A production application creates LoggerConfig and retains CoreLoggerOwner
    // in application state. The adapter receives only the backend capability.
    let _backend_type: Option<Arc<dyn HostLoggingBackend>> = None;
    let _policy = AdapterPolicy {
        allowed_window_labels: BTreeSet::from(["main".to_owned()]),
        allowed_targets: BTreeSet::from(["tauri-example".to_owned()]),
        max_request_bytes: 65_536,
        max_depth: 32,
        redacted_field_keys: BTreeSet::from(["secret".to_owned()]),
    };
    let _ = plugin::<tauri::Wry>;
    let _ = app_observability_level_change::<tauri::Wry>;
    eprintln!("construct the host logger before installing sc-observability-tauri");
}
