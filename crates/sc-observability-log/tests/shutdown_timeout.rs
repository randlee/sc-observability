//! Owner timeout retains one shutdown operation for controls to observe later.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration fixture owns one process-global bridge"
)]

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sc_observability::{RedactionPolicy, Redactor};
use sc_observability_log::{
    ActionName, AdmissionOutcome, BridgeEvent, BridgeOptions, EmitError, EventLevel, LevelFilter,
    LifecyclePhase, LoggerConfig, ServiceName, ShutdownError, ShutdownOutcome, TargetCategory,
    WaitError,
};
use sc_observability_types::WriterState;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(150);
const LATE_COMPLETION_DEADLINE: Duration = Duration::from_secs(20);

#[derive(Debug)]
struct Gate {
    entered: SyncSender<()>,
    release: Mutex<Receiver<()>>,
}

static GATE: OnceLock<Gate> = OnceLock::new();

struct BlockingRedactor;

impl Redactor for BlockingRedactor {
    fn redact(&self, key: &str, _value: &mut serde_json::Value) {
        if key == "block" {
            let gate = GATE.get().unwrap();
            gate.entered.send(()).unwrap();
            gate.release
                .lock()
                .unwrap()
                .recv_timeout(LATE_COMPLETION_DEADLINE)
                .expect("test release signal timed out");
        }
    }
}

fn event(message: &str, fields: serde_json::Map<String, serde_json::Value>) -> BridgeEvent {
    BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("shutdown_timeout").unwrap(),
        action: None,
        message: Some(message.to_owned()),
        outcome: None,
        fields,
        request_id: None,
        correlation_id: None,
        trace: None,
    }
}

#[test]
fn timed_out_owner_shutdown_completes_late_for_repeated_control_waiters() {
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    GATE.set(Gate {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    })
    .unwrap();

    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("shutdown-timeout").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    config.redaction = RedactionPolicy {
        custom_redactors: vec![Box::new(BlockingRedactor)],
        ..RedactionPolicy::default()
    };
    let guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let control = guard.control();
    let path = control.active_log_path().unwrap().unwrap();
    let blocked_control = control.clone();
    let (blocked_tx, blocked_rx) = sync_channel(1);
    let _blocked = std::thread::spawn(move || {
        let result = blocked_control.try_log(event(
            "admitted before late completion",
            serde_json::Map::from_iter([("block".to_owned(), serde_json::json!(true))]),
        ));
        let _ = blocked_tx.send(result);
    });
    entered_rx
        .recv_timeout(LATE_COMPLETION_DEADLINE)
        .expect("redactor did not enter before shutdown timeout");

    let started = Instant::now();
    let result = guard.shutdown(SHUTDOWN_TIMEOUT);
    assert!(matches!(
        result,
        Err(ShutdownError::TimedOut { timeout }) if timeout == SHUTDOWN_TIMEOUT
    ));
    assert!(started.elapsed() < SHUTDOWN_TIMEOUT * 10);
    assert_eq!(
        control.health().unwrap().lifecycle,
        LifecyclePhase::Stopping
    );
    assert!(matches!(
        control.try_log(event("rejected while stopping", serde_json::Map::new())),
        Err(EmitError::NotRunning {
            phase: LifecyclePhase::Stopping,
        })
    ));
    assert!(matches!(
        control.wait_stopped(Duration::from_millis(10)),
        Err(WaitError::TimedOut { .. })
    ));

    release_tx.send(()).unwrap();
    assert_eq!(
        blocked_rx
            .recv_timeout(LATE_COMPLETION_DEADLINE)
            .expect("blocked submission did not finish")
            .unwrap(),
        AdmissionOutcome::Accepted
    );
    let first = control
        .wait_stopped(LATE_COMPLETION_DEADLINE)
        .expect("late completion was not published");
    let second = control.wait_stopped(Duration::ZERO).unwrap();
    assert!(matches!(first.outcome, ShutdownOutcome::Stopped));
    assert_eq!(first.health.lifecycle, LifecyclePhase::Stopped);
    assert_eq!(first.health.logging.writer_state, WriterState::Stopped);
    assert_eq!(first.health.logging.queue_depth, 0);
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("admitted before late completion")
    );
}
