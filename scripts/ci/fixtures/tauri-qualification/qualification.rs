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
    app.manage(OwnerHold::default());
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

#[derive(Default)]
pub struct OwnerHold(Mutex<Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>>);

/// Hold only the existing owner mutex; subsequent level IPC runs unchanged.
/// The holder is bounded to one thread and releases automatically after 30 s.
#[tauri::command]
pub fn qualification_owner_gate(
    held: bool,
    owner: tauri::State<'_, super::OwnerState>,
    state: tauri::State<'_, OwnerHold>,
) -> Result<(), String> {
    use std::{sync::mpsc, time::Duration};
    let mut slot = state.0.lock().map_err(|_| "holder state poisoned")?;
    if !held {
        if let Some((release, done)) = slot.take() {
            release.send(()).map_err(|_| "holder exited early")?;
            done.recv_timeout(Duration::from_secs(2)).map_err(|_| "holder did not release")?;
        }
        return Ok(());
    }
    if slot.is_some() {
        return Err("owner holder already active".into());
    }
    let owner = owner.0.clone();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new().name("qualification-owner-holder".into()).spawn(move || {
        let guard = owner.lock();
        if ready_tx.send(guard.is_ok()).is_err() { return; }
        if guard.is_ok() {
            let _ = release_rx.recv_timeout(Duration::from_secs(30));
        }
        drop(guard);
        let _ = done_tx.send(());
    }).map_err(|error| error.to_string())?;
    if !ready_rx.recv_timeout(Duration::from_secs(2)).map_err(|_| "owner acquisition timed out")? {
        return Err("owner mutex poisoned".into());
    }
    *slot = Some((release_tx, done_rx));
    Ok(())
}
