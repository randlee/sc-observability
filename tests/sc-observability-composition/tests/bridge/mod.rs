//! Test-local forwarding adapter; no production integration is implied.
use sc_observability_types::{LogEvent, Observation, SinkHealth, SinkHealthState, SinkName};
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub trait Consumer: Send + Sync + 'static {
    fn forward(&self, event: LogEvent) -> Result<(), sc_observability_types::ObservationError>;
}
impl Consumer for sc_observe::Observability {
    fn forward(&self, event: LogEvent) -> Result<(), sc_observability_types::ObservationError> {
        self.emit(Observation::new(event.service.clone(), event))
    }
}
impl Consumer for sc_observe::v2::Observability {
    fn forward(&self, event: LogEvent) -> Result<(), sc_observability_types::ObservationError> {
        self.emit(Observation::new(event.service.clone(), event))
    }
}
struct Forward<C>(Arc<C>);
impl<C: Consumer> sc_observability::v2::LogSink for Forward<C> {
    fn write(&self, event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
        self.0.forward(event.clone()).map_err(|source| {
            sc_observability_types::v2::LogSinkError::Write {
                context: Box::new(
                    sc_observability_types::ErrorContext::new(
                        sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE,
                        "test-local forwarding consumer rejected the event",
                        sc_observability_types::Remediation::not_recoverable(
                            "rebuild the closed observation consumer",
                        ),
                    )
                    .source(Box::new(source)),
                ),
            }
        })
    }
    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("composition-forwarder").expect("sink"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}
struct Admit;
impl sc_observability_log::BridgeEventPolicy for Admit {
    fn decide(&self, event: &LogEvent) -> sc_observability_log::BridgeEventDecision {
        if event.message.as_deref() == Some("denied-must-not-export") {
            sc_observability_log::BridgeEventDecision::Reject(
                sc_observability_log::PolicyRejection::Denied,
            )
        } else {
            sc_observability_log::BridgeEventDecision::Admit
        }
    }
}

/// Runs each global bridge case in a fresh process with a hard outer bound.
pub fn isolated(name: &str) -> bool {
    let name = name
        .strip_prefix("composition::")
        .expect("integration module");
    if std::env::var("D18_COMPOSITION_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let output = tempfile::tempfile().expect("child output");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", name, "--nocapture"])
        .env("D18_COMPOSITION_CHILD", name)
        .stdout(output.try_clone().expect("stdout"))
        .stderr(output.try_clone().expect("stderr"))
        .spawn()
        .expect("isolated bridge test");
    let deadline = Instant::now() + Duration::from_secs(45);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill timed out test");
            child.wait().expect("reap test");
            panic!("composition child exceeded45s: {name}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut output = output;
    output.rewind().expect("rewind");
    let mut text = String::new();
    output.read_to_string(&mut text).expect("read output");
    assert!(status.success(), "{name}: {status}\n{text}");
    assert!(
        text.contains("1 passed; 0 failed"),
        "must execute exact child: {text}"
    );
    true
}

/// Flush the actual attached core writer before exporter flush is requested.
pub fn deliver<C: Consumer>(message: &str, consumer: Arc<C>, root: &Path, released: bool) {
    use sc_observability_log::{
        ActionName, AttachmentOptions, BridgeEvent, BridgeOptions, EventLevel, TargetCategory,
    };
    let config = sc_observability::LoggerConfig::default_for(
        super::support::service_name(),
        root.join("bridge"),
    );
    let sink = Arc::new(Forward(consumer));
    let logger: sc_observability::v2::Logger = if released {
        let mut builder =
            sc_observability::LoggerBuilder::new_typed(config).expect("released core builder");
        builder
            .register_typed_sink(sink)
            .expect("register forwarder");
        builder.build_typed().expect("released core runtime").into()
    } else {
        let mut builder =
            sc_observability::v2::LoggerBuilder::new(config).expect("canonical core builder");
        builder
            .register_typed_sink(sink)
            .expect("register forwarder");
        builder.build().expect("canonical core runtime")
    };
    let logger = Arc::new(logger);
    let mut attachment = sc_observability_log::attach_logger(
        logger.clone(),
        AttachmentOptions::new(
            BridgeOptions {
                default_action: ActionName::new("composition.forward").expect("action"),
                parse_bracket_action: true,
            },
            Arc::new(Admit),
        ),
    )
    .expect("attach public bridge");
    let control = attachment.control();
    let event = BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("composition.bridge").expect("target"),
        action: None,
        message: Some(message.to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    };
    let mut denied = event.clone();
    denied.message = Some("denied-must-not-export".to_owned());
    assert!(
        matches!(
            control.try_log(denied),
            Err(sc_observability_log::v2::EmitError::InvalidEvent { .. })
        ),
        "policy-negative must reject before forwarding"
    );
    control.try_log(event).expect("bridge admission");
    control
        .flush(Duration::from_secs(5))
        .expect("core writer forwarding barrier");
    assert!(logger.health().last_writer_error.is_none());
    attachment
        .detach(Duration::from_secs(5))
        .expect("detach bridge");
    assert!(
        control.flush(Duration::from_secs(1)).is_err(),
        "stale control must reject"
    );
    let Ok(logger) = Arc::try_unwrap(logger) else {
        panic!("attachment leaked core owner");
    };
    let stopped = logger.shutdown();
    assert!(stopped.health().last_writer_error.is_none());
}

/// Check the real closed-observe rejection and its public core health propagation.
/// The writer health API intentionally carries summaries, not source pointers.
pub fn reject_closed<C: Consumer>(consumer: Arc<C>, root: &Path) {
    use sc_observability::v2::LogSink;
    use std::error::Error;
    let sink = Arc::new(Forward(consumer));
    let event = super::support::log_event("after-observe-shutdown");
    let error = sink
        .write(&event)
        .expect_err("closed observe rejects forwarding");
    assert!(matches!(
        error,
        sc_observability_types::v2::LogSinkError::Write { .. }
    ));
    assert!(matches!(
        error
            .context()
            .source()
            .expect("underlying observe error")
            .downcast_ref::<sc_observability_types::ObservationError>(),
        Some(sc_observability_types::ObservationError::Shutdown)
    ));
    let expected = error.diagnostic().clone();
    let mut config = sc_observability::LoggerConfig::default_for(
        super::support::service_name(),
        root.join("closed"),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder =
        sc_observability::v2::LoggerBuilder::new(config).expect("closed-path core builder");
    builder
        .register_typed_sink(sink)
        .expect("forwarder registration");
    let logger = builder.build().expect("writer");
    logger
        .log(event)
        .expect("queue admission precedes downstream failure");
    logger.flush().expect("writer completion barrier");
    let health = logger.health();
    assert_eq!(health.dropped_events_total, 1);
    assert_eq!(
        health.last_writer_error.expect("writer degraded").code,
        Some(sc_observability::error_codes::LOGGER_WRITER_DEGRADED)
    );
    let last = health.last_error.expect("downstream diagnostic summary");
    assert_eq!(last.code, Some(expected.code));
    assert_eq!(last.message, expected.message);
    logger.shutdown();
}
