//! Released facade queue-full admission against a real saturated writer queue.
//!
//! A capacity-1 logger writes to a sink that holds its first event until the
//! test releases it. Once the writer thread is inside that write, one queued
//! event fills the queue, so every released non-blocking admission path must
//! report its `QueueFull` variant with `LOGGER_QUEUE_FULL`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sc_observability::v2::LogSink as CanonicalLogSink;
use sc_observability::*;
use sc_observability_types::v2::LogSinkError as CanonicalLogSinkError;
use serde_json::Map;

/// Upper bound on any single handshake, so a regression fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

/// Holds the first write until the test drops the release sender.
struct GatedSink {
    held: AtomicBool,
    entered: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl CanonicalLogSink for GatedSink {
    fn write(&self, _: &LogEvent) -> Result<(), CanonicalLogSinkError> {
        if !self.held.swap(true, Ordering::SeqCst) {
            self.entered
                .lock()
                .expect("entered sender")
                .send(())
                .expect("test waits for the writer to enter the sink");
            let outcome = self
                .release
                .lock()
                .expect("release receiver")
                .recv_timeout(WATCHDOG);
            assert!(
                !matches!(outcome, Err(RecvTimeoutError::Timeout)),
                "the test never released the held write"
            );
        }
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("released-queue-full").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

/// Releases the held write on drop, including when an assertion panics.
struct ReleaseOnDrop(Option<Sender<()>>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        drop(self.0.take());
    }
}

fn event(action: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("static version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("released-queue-full").expect("static service name"),
        target: TargetCategory::new("released.queue").expect("static target"),
        action: ActionName::new(action).expect("static action"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Map::new(),
    }
}

fn assert_queue_full_code(context: &ErrorContext) {
    assert_eq!(context.diagnostic().code, error_codes::LOGGER_QUEUE_FULL);
}

#[test]
#[expect(
    deprecated,
    reason = "the released try_log and try_log_with_outcome results are the contract under test"
)]
fn released_try_log_paths_report_queue_full_on_saturated_queue() {
    let root = tempfile::tempdir().expect("temporary root");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("released-queue-full").expect("static service name"),
        root.path().to_path_buf(),
    );
    config.queue_capacity = 1;
    config.enable_file_sink = false;
    config.enable_console_sink = false;

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut builder = LoggerBuilder::new_typed(config).expect("released typed builder");
    builder
        .register_typed_sink(Arc::new(GatedSink {
            held: AtomicBool::new(false),
            entered: Mutex::new(entered_tx),
            release: Mutex::new(release_rx),
        }))
        .expect("gated sink registration");
    let logger = builder.build_typed().expect("released typed build");
    // Declared after the logger so an unwinding assertion releases the writer
    // before the logger is dropped.
    let release = ReleaseOnDrop(Some(release_tx));

    logger
        .log_typed(event("held"))
        .expect("held event admitted");
    entered_rx
        .recv_timeout(WATCHDOG)
        .expect("writer entered the gated sink");
    logger
        .try_log_typed(event("queued"))
        .expect("the single queue slot accepts one event");

    match logger.try_log(event("full")) {
        Err(TryLogError::QueueFull(context)) => assert_queue_full_code(&context),
        other => panic!("released try_log must report QueueFull, got {other:?}"),
    }
    match logger.try_log_typed(event("full")) {
        Err(TryLogFailure::QueueFull(context)) => assert_queue_full_code(&context),
        other => panic!("released try_log_typed must report QueueFull, got {other:?}"),
    }
    match logger.try_log_with_outcome(event("full")) {
        Err(TryLogError::QueueFull(context)) => assert_queue_full_code(&context),
        other => panic!("released try_log_with_outcome must report QueueFull, got {other:?}"),
    }
    match logger.try_log_with_outcome_typed(event("full")) {
        Err(TryLogFailure::QueueFull(context)) => assert_queue_full_code(&context),
        other => {
            panic!("released try_log_with_outcome_typed must report QueueFull, got {other:?}")
        }
    }

    drop(release);
    let _stopped = logger.shutdown();
}
