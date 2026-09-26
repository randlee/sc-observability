//! Public integration coverage for the non-owning host attachment lifecycle.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "integration fixtures keep setup failures explicit"
)]

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

#[allow(deprecated)]
use sc_observability::{LogSink, SinkHealth, SinkHealthState, SinkName, SinkRegistration};
use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEvent, BridgeEventDecision, BridgeEventPolicy,
    BridgeOptions, DetachError, EventLevel, FlushError, InitError, LoggerConfig, ServiceName,
    TargetCategory, attach_logger,
};
use sc_observability_types::LogEvent;

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct Admit;

impl BridgeEventPolicy for Admit {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        BridgeEventDecision::Admit
    }
}

struct RecordingSink {
    events: Arc<Mutex<Vec<LogEvent>>>,
}

#[allow(deprecated)]
impl LogSink for RecordingSink {
    fn write(
        &self,
        event: &sc_observability_types::LogEvent,
    ) -> Result<(), sc_observability_types::LogSinkError> {
        self.events
            .lock()
            .expect("recording lock")
            .push(event.clone());
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("attachment-recording").expect("sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

struct Blocking {
    entered: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl BridgeEventPolicy for Blocking {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        self.entered
            .lock()
            .expect("entered lock")
            .take()
            .expect("single blocking call")
            .send(())
            .expect("entered receiver");
        self.release
            .lock()
            .expect("release lock")
            .take()
            .expect("single release")
            .recv()
            .expect("release sender");
        BridgeEventDecision::Admit
    }
}

fn options(policy: Arc<dyn BridgeEventPolicy>) -> AttachmentOptions {
    AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("log.record").expect("action"),
            parse_bracket_action: true,
        },
        policy,
    )
}

fn logger() -> Arc<sc_observability::Logger> {
    let root = tempfile::tempdir().expect("temp root");
    // Keep the root alive for the duration of the process-local fixture by
    // leaking only this test's temporary directory handle.
    let root = Box::leak(Box::new(root));
    Arc::new(
        sc_observability::Logger::new_typed(LoggerConfig::default_for(
            ServiceName::new("attachment").expect("service"),
            root.path().to_path_buf(),
        ))
        .expect("host logger"),
    )
}

fn recording_logger() -> (Arc<sc_observability::Logger>, Arc<Mutex<Vec<LogEvent>>>) {
    let root = tempfile::tempdir().expect("temp root");
    let root = Box::leak(Box::new(root));
    let mut config = LoggerConfig::default_for(
        ServiceName::new("attachment-recording").expect("service"),
        root.path().to_path_buf(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut builder = sc_observability::LoggerBuilder::new_typed(config).expect("builder");
    builder.register_sink(SinkRegistration::new(Arc::new(RecordingSink {
        events: Arc::clone(&events),
    })));
    let logger = builder.build_typed().expect("host logger");
    (Arc::new(logger), events)
}

struct BlockingFlushSink {
    entered: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl BlockingFlushSink {
    fn new(entered: mpsc::Sender<()>, release: mpsc::Receiver<()>) -> Self {
        Self {
            entered: Mutex::new(Some(entered)),
            release: Mutex::new(Some(release)),
        }
    }
}

#[allow(deprecated)]
impl LogSink for BlockingFlushSink {
    fn write(
        &self,
        _event: &sc_observability_types::LogEvent,
    ) -> Result<(), sc_observability_types::LogSinkError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::LogSinkError> {
        if let Some(entered) = self.entered.lock().expect("entered lock").take() {
            entered.send(()).expect("flush entered receiver");
        }
        if let Some(release) = self.release.lock().expect("release lock").take() {
            release.recv().expect("flush release sender");
        }
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("blocking-flush").expect("sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

fn blocking_logger(
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
) -> Arc<sc_observability::Logger> {
    let root = tempfile::tempdir().expect("temp root");
    let root = Box::leak(Box::new(root));
    let mut config = LoggerConfig::default_for(
        ServiceName::new("attachment-blocking-flush").expect("service"),
        root.path().to_path_buf(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = sc_observability::LoggerBuilder::new_typed(config).expect("builder");
    builder.register_sink(SinkRegistration::new(Arc::new(BlockingFlushSink::new(
        entered, release,
    ))));
    Arc::new(builder.build_typed().expect("host logger"))
}

fn event() -> BridgeEvent {
    BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("attachment.test").expect("target"),
        action: None,
        message: Some("direct attachment event".to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    }
}

#[test]
fn attachment_routes_direct_and_macro_calls_and_recovers_host_ownership() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let (host, events) = recording_logger();
    let mut attachment =
        attach_logger(Arc::clone(&host), options(Arc::new(Admit))).expect("attach host logger");
    let control = attachment.control();

    control.try_log(event()).expect("direct event");
    log::info!(target: "attachment::macro", "macro event");
    control.flush(Duration::from_secs(2)).expect("flush");
    let events = events.lock().expect("recording lock");
    assert_eq!(events.len(), 2, "direct and macro events share one sink");
    assert!(
        events
            .iter()
            .any(|event| { event.message.as_deref() == Some("direct attachment event") })
    );
    assert!(
        events
            .iter()
            .any(|event| event.message.as_deref() == Some("macro event"))
    );
    attachment.detach(Duration::from_secs(2)).expect("detach");

    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    host.shutdown();
}

#[test]
fn timeout_retains_attachment_for_retry_and_stale_control_is_rejected() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let policy = Arc::new(Blocking {
        entered: Mutex::new(Some(entered_tx)),
        release: Mutex::new(Some(release_rx)),
    });
    let host = logger();
    let mut attachment = attach_logger(Arc::clone(&host), options(policy)).expect("attach");
    let control = attachment.control();
    let worker = std::thread::spawn(|| log::info!(target: "attachment::blocking", "blocked"));
    entered_rx.recv().expect("policy entered");

    assert!(matches!(
        attachment.detach(Duration::ZERO),
        Err(DetachError::Timeout { .. })
    ));
    release_tx.send(()).expect("release policy");
    worker.join().expect("logging worker");
    attachment
        .detach(Duration::from_secs(2))
        .expect("retry detach");
    assert!(matches!(
        control.try_log(event()),
        Err(sc_observability_log::EmitError::NotRunning { .. })
    ));

    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    let _ = host.shutdown();
}

#[test]
fn timed_out_flush_keeps_attachment_owned_logger_until_helper_drains() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let host = blocking_logger(entered_tx, release_rx);
    let mut attachment =
        attach_logger(Arc::clone(&host), options(Arc::new(Admit))).expect("attach");
    let control = attachment.control();
    let flush = std::thread::spawn(move || control.flush(Duration::ZERO));

    entered_rx.recv().expect("flush entered sink");
    assert!(matches!(
        attachment.detach(Duration::ZERO),
        Err(DetachError::Timeout { .. })
    ));

    release_tx.send(()).expect("release flush");
    assert!(matches!(
        flush.join().expect("flush worker"),
        Err(FlushError::TimedOut { .. })
    ));
    attachment
        .detach(Duration::from_secs(2))
        .expect("drained flush detaches");

    let host = Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases logger"));
    host.shutdown();
}

#[test]
fn reattachment_rejects_old_control_and_init_while_attached() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let host = logger();
    let mut first =
        attach_logger(Arc::clone(&host), options(Arc::new(Admit))).expect("first attach");
    let stale = first.control();
    assert!(matches!(
        sc_observability_log::init(
            LoggerConfig::default_for(
                ServiceName::new("owned-conflict").expect("service"),
                std::env::temp_dir().join("owned-conflict"),
            ),
            BridgeOptions {
                default_action: ActionName::new("log.record").expect("action"),
                parse_bracket_action: false,
            },
        ),
        Err(InitError::AlreadyInitialized)
    ));
    first.detach(Duration::from_secs(2)).expect("first detach");
    let host = Arc::try_unwrap(host).unwrap_or_else(|_| panic!("first detach releases logger"));
    host.shutdown();

    let host = logger();
    let mut second = attach_logger(Arc::clone(&host), options(Arc::new(Admit))).expect("reattach");
    assert!(matches!(
        stale.try_log(event()),
        Err(sc_observability_log::EmitError::NotRunning { .. })
    ));
    second
        .control()
        .try_log(event())
        .expect("new attachment control");
    second
        .detach(Duration::from_secs(2))
        .expect("second detach");
    let host = Arc::try_unwrap(host).unwrap_or_else(|_| panic!("second detach releases logger"));
    host.shutdown();
}
