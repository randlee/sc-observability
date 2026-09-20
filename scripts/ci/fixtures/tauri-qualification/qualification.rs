//! Qualification-only observation hooks. All logging/level calls use the real
//! application commands and supplied native backend; no policy is bypassed.
use sc_observability_binding_runtime::{HostLoggingBackend, ProducerOrigin};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct Reports(Mutex<BTreeMap<String, Value>>);

pub fn seed(backend: &dyn HostLoggingBackend) -> Result<(), String> {
    policy_matrix()?;
    let event = sc_observability_dto::decode_event(json!({
        "schema_version": 1, "level": "info", "target": "tauri-example",
        "action": "rust-host", "correlation_id": "tauri-qualification",
        "message": "Bearer qualification-secret", "fields": {}
    }))
    .map_err(|error| format!("seed validation failed: {error:?}"))?;
    backend
        .try_log(event, ProducerOrigin::RustHost)
        .map_err(|error| format!("seed admission failed: {error:?}"))?;
    let private_event = sc_observability_dto::decode_event(json!({
        "schema_version": 1, "level": "info", "target": "host-private",
        "action": "private-host-record", "fields": {}
    }))
    .map_err(|error| format!("private host seed validation failed: {error:?}"))?;
    match backend.try_log(private_event, ProducerOrigin::RustHost) {
        Ok(sc_observability_dto::AdmissionDto::Accepted) => {}
        result => return Err(format!("private host seed was not admitted: {result:?}")),
    }
    Ok(())
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Reports::default());
    app.manage(OwnerHold::default());
    app.manage(HostFlush::default());
    app.manage(HostShutdown::default());
    tauri::WebviewWindowBuilder::new(
        app,
        "forbidden",
        tauri::WebviewUrl::App("index.html?forbidden=1".into()),
    )
    .title("Unauthorized qualification caller")
    .build()?;
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
        let passed = reports
            .values()
            .all(|value| value.get("passed") == Some(&Value::Bool(true)));
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
    app: tauri::AppHandle,
    state: tauri::State<'_, OwnerHold>,
) -> Result<(), String> {
    use std::{sync::mpsc, time::Duration};
    let mut slot = state.0.lock().map_err(|_| "holder state poisoned")?;
    if !held {
        if let Some((release, done)) = slot.take() {
            release.send(()).map_err(|_| "holder exited early")?;
            done.recv_timeout(Duration::from_secs(2))
                .map_err(|_| "holder did not release")?;
        }
        return Ok(());
    }
    if slot.is_some() {
        return Err("owner holder already active".into());
    }
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("qualification-owner-holder".into())
        .spawn(move || {
            let owner = app.state::<super::OwnerState>();
            let guard = owner.guard.lock();
            if ready_tx.send(guard.is_ok()).is_err() {
                return;
            }
            if guard.is_ok() {
                let _ = release_rx.recv_timeout(Duration::from_secs(30));
            }
            drop(guard);
            let _ = done_tx.send(());
        })
        .map_err(|error| error.to_string())?;
    if !ready_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "owner acquisition timed out")?
    {
        return Err("owner mutex poisoned".into());
    }
    *slot = Some((release_tx, done_rx));
    Ok(())
}

/// Enable an additional real console sink for the held-pipe fault scenario.
/// All existing root/level/redaction/queue policy remains host-owned and intact.
pub fn configure(config: &mut sc_observability::LoggerConfig) {
    config.enable_console_sink = true;
}

#[tauri::command]
pub async fn qualification_output_gate(paused: bool, token: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::time::{Duration, Instant};
        let base = std::env::var("SC_TAURI_QUALIFICATION_CONTROL")
            .map_err(|_| "host did not configure output controller")?;
        std::fs::write(
            format!("{base}.request"),
            serde_json::to_vec(&json!({"paused": paused, "token": token}))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(format!("{base}.ack")) {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("token").and_then(Value::as_str) == Some(token.as_str()) {
                        return Ok(());
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("output controller did not acknowledge".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub struct Backend(pub std::sync::Arc<dyn HostLoggingBackend>);
#[derive(Default)]
pub struct HostFlush(
    Mutex<Option<sc_observability_binding_runtime::Operation<sc_observability_dto::CompletionDto>>>,
);

/// Reserve an actual host flush before frontend query/flush overlap probes.
/// This calls the supplied backend unchanged with the public maximum timeout.
#[tauri::command]
pub fn qualification_host_flush(
    start: bool,
    backend: tauri::State<'_, Backend>,
    state: tauri::State<'_, HostFlush>,
) -> Result<Value, String> {
    let mut slot = state
        .0
        .lock()
        .map_err(|_| "host flush observation poisoned")?;
    if start {
        if slot.is_some() {
            return Err("host flush already reserved".into());
        }
        *slot = Some(
            backend
                .0
                .start_flush(std::time::Duration::from_secs(60))
                .map_err(|error| format!("host flush start failed: {error:?}"))?,
        );
    }
    let operation = slot.as_ref().ok_or("host flush has not started")?;
    Ok(match operation.state() {
        sc_observability_binding_runtime::OperationState::Pending => json!({"pending": true}),
        sc_observability_binding_runtime::OperationState::Completed { result } => {
            json!({"pending": false, "completed": result.is_ok(), "result": format!("{result:?}")})
        }
    })
}

pub fn policy_matrix() -> Result<(), String> {
    use std::collections::BTreeSet;
    let base = sc_observability_tauri::AdapterPolicy {
        allowed_window_labels: BTreeSet::from(["main".into()]),
        allowed_targets: BTreeSet::from(["tauri-example".into()]),
        max_request_bytes: 65536,
        max_depth: 32,
        redacted_field_keys: BTreeSet::new(),
    };
    let mut cases = Vec::new();
    for (name, labels) in [
        ("empty-window-set", vec![]),
        ("empty-window", vec![""]),
        ("nul-window", vec!["bad\0label"]),
        ("invalid-window", vec!["bad\nlabel"]),
    ] {
        let mut policy = base.clone();
        policy.allowed_window_labels = labels.into_iter().map(str::to_owned).collect();
        cases.push((name, policy));
    }
    for (name, targets) in [
        ("empty-target-set", vec![]),
        ("empty-target", vec![""]),
        ("nul-target", vec!["bad\0target"]),
        ("invalid-target", vec!["bad target"]),
    ] {
        let mut policy = base.clone();
        policy.allowed_targets = targets.into_iter().map(str::to_owned).collect();
        cases.push((name, policy));
    }
    for (name, size, depth) in [
        ("zero-bytes", 0, 32),
        ("excess-bytes", 65537, 32),
        ("zero-depth", 65536, 0),
        ("excess-depth", 65536, 33),
    ] {
        let mut policy = base.clone();
        policy.max_request_bytes = size;
        policy.max_depth = depth;
        cases.push((name, policy));
    }
    for key in [
        "sc_observability.binding.language",
        "sc_observability::binding::future",
    ] {
        let mut policy = base.clone();
        policy.redacted_field_keys.insert(key.into());
        cases.push((key, policy));
    }
    let mut records = vec![json!({"name": "valid-policy", "passed": base.validate().is_ok()})];
    for (name, policy) in cases {
        let result = policy.validate();
        let error = result
            .err()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| error.to_string())?;
        let passed = error.as_ref().is_some_and(|value| {
            value["kind"] == "validation"
                && value["code"]
                    == sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT
        });
        records.push(json!({"name": name, "passed": passed, "error": error}));
    }
    let passed = records.iter().all(|record| record["passed"] == true);
    let path = std::env::var("SC_TAURI_QUALIFICATION_POLICY")
        .map_err(|_| "policy evidence path absent")?;
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&json!({"passed": passed, "records": records}))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if passed {
        Ok(())
    } else {
        Err("packaged adapter accepted an invalid host policy".into())
    }
}

type ShutdownResult = Result<(), sc_observability_dto::Failure>;
type ShutdownCompletion = std::sync::Arc<Mutex<Option<ShutdownResult>>>;
#[derive(Default)]
pub struct HostShutdown(Mutex<Option<ShutdownCompletion>>);

/// Invoke the production host-only shutdown method on one bounded worker.
/// No fixture creates a lifecycle owner or changes the production handler.
#[tauri::command]
pub fn qualification_shutdown(
    operation: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, HostShutdown>,
) -> Result<Value, String> {
    use std::{sync::Arc, time::Duration};
    if operation == "busy" {
        return serde_json::to_value(app.state::<super::OwnerState>().shutdown(Duration::ZERO))
            .map_err(|error| error.to_string());
    }
    let mut slot = state
        .0
        .lock()
        .map_err(|_| "shutdown observation poisoned")?;
    if operation == "start" {
        if slot.is_some() {
            return Err("shutdown already started".into());
        }
        let result = Arc::new(Mutex::new(None));
        let completed = Arc::clone(&result);
        let shutdown_app = app.clone();
        std::thread::Builder::new()
            .name("qualification-host-shutdown".into())
            .spawn(move || {
                let result = shutdown_app
                    .state::<super::OwnerState>()
                    .shutdown(Duration::ZERO);
                if let Ok(mut completed) = completed.lock() {
                    *completed = Some(result);
                }
            })
            .map_err(|error| error.to_string())?;
        *slot = Some(result);
    } else if operation != "status" {
        return Err("unknown shutdown observation operation".into());
    }
    let result = slot
        .as_ref()
        .ok_or("shutdown not started")?
        .lock()
        .map_err(|_| "shutdown completion poisoned")?;
    let health = app
        .state::<super::OwnerState>()
        .control
        .health()
        .map_err(|error| error.to_string())?;
    let pending = health.lifecycle != sc_observability_log::LifecyclePhase::Stopped;
    Ok(json!({"pending": pending, "returned": result.is_some(), "result": &*result}))
}
