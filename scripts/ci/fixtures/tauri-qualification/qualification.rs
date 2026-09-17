//! Qualification-only observation hooks. All logging/level calls use the real
//! application commands and supplied native backend; no policy is bypassed.
use sc_observability_binding_runtime::{HostLoggingBackend, ProducerOrigin};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct Reports(Mutex<BTreeMap<String, Value>>);

pub fn seed(backend: &dyn HostLoggingBackend) -> Result<(), String> {
    let event = sc_observability_dto::decode_event(json!({
        "schema_version": 1, "level": "info", "target": "tauri-example",
        "action": "rust-host", "correlation_id": "tauri-qualification",
        "message": "Bearer qualification-secret", "fields": {}
    })).map_err(|error| format!("seed validation failed: {error:?}"))?;
    backend.try_log(event, ProducerOrigin::RustHost)
        .map_err(|error| format!("seed admission failed: {error:?}"))?;
    Ok(())
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Reports::default());
    tauri::WebviewWindowBuilder::new(
        app, "forbidden", tauri::WebviewUrl::App("index.html?forbidden=1".into()),
    ).title("Unauthorized qualification caller").build()?;
    Ok(())
}

#[tauri::command]
pub fn qualification_report(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, Reports>,
    report: Value,
) -> Result<(), String> {
    let mut reports = state.0.lock().map_err(|_| "report lock poisoned")?;
    if !matches!(window.label(), "main" | "forbidden") {
        return Err("unexpected qualification window".into());
    }
    reports.insert(window.label().to_owned(), report);
    let path = std::env::var("SC_TAURI_QUALIFICATION_REPORT")
        .map_err(|_| "host did not configure evidence path")?;
    let body = serde_json::to_vec_pretty(&*reports).map_err(|error| error.to_string())?;
    std::fs::write(path, body).map_err(|error| error.to_string())?;
    if reports.len() == 2 {
        let passed = reports.values().all(|value| value.get("passed") == Some(&Value::Bool(true)));
        app.exit(if passed { 0 } else { 1 });
    }
    Ok(())
}
