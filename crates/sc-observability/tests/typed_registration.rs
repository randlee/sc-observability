//! Consumer coverage for typed sink registration in the logging builder.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use sc_observability::typed::{TypedLogSink, legacy_sink};
use sc_observability::*;
use sc_observability_types::DiagnosticInfo;
use sc_observability_types::v2::LogSinkError;
use serde_json::Map;

fn assert_diagnostic_info<T: DiagnosticInfo>(_: &T) {}

struct RecordingTypedSink {
    writes: AtomicUsize,
    flushes: AtomicUsize,
    state: RwLock<SinkHealthState>,
}

impl Default for RecordingTypedSink {
    fn default() -> Self {
        Self {
            writes: AtomicUsize::new(0),
            flushes: AtomicUsize::new(0),
            state: RwLock::new(SinkHealthState::Healthy),
        }
    }
}

impl RecordingTypedSink {
    fn health_snapshot(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("typed-registration").expect("static sink name"),
            state: *self.state.read().expect("sink health poisoned"),
            last_error: None,
        }
    }
}

impl TypedLogSink for RecordingTypedSink {
    fn write(&self, _: &LogEvent) -> Result<(), LogSinkError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn flush(&self) -> Result<(), LogSinkError> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health_snapshot()
    }
}

struct AllowInfo;

impl LogFilter for AllowInfo {
    fn accepts(&self, event: &LogEvent) -> bool {
        event.level == Level::Info
    }
}

fn config() -> LoggerConfig {
    let root = tempfile::tempdir().expect("temporary log root");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("typed-registration").expect("static service name"),
        root.path().to_path_buf(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config
}

fn event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("static schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("typed-registration").expect("static service name"),
        target: TargetCategory::new("typed.registration").expect("static target"),
        action: ActionName::new("register").expect("static action"),
        message: Some("typed sink registration".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Map::default(),
    }
}

#[test]
fn typed_registration_entry_points_preserve_metadata_chaining_and_single_dispatch() {
    let registration_sink = Arc::new(RecordingTypedSink::default());
    let builder_sink = Arc::new(RecordingTypedSink::default());
    let chained_sink = Arc::new(RecordingTypedSink::default());
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");

    builder.register_sink(
        SinkRegistration::typed(registration_sink.clone()).with_filter(Arc::new(AllowInfo)),
    );
    builder
        .register_typed_sink(builder_sink.clone())
        .expect("first typed registration")
        .register_typed_sink(chained_sink.clone())
        .expect("chained typed registration");

    let logger = builder.build_canonical().expect("build typed logger");
    logger.log(event()).expect("admit event");
    logger.flush().expect("flush typed sinks");

    for sink in [&registration_sink, &builder_sink, &chained_sink] {
        assert_eq!(sink.writes.load(Ordering::SeqCst), 1);
        // The explicit flush is the sole sink flush for the admitted event.
        // Registration never duplicates either operation.
        assert_eq!(sink.flushes.load(Ordering::SeqCst), 1);
        assert_eq!(sink.health_snapshot().state, SinkHealthState::Healthy);
    }
}

#[test]
fn builder_rejects_zero_sinks_at_build_and_accepts_a_registered_sink() {
    let zero_sink = LoggerBuilder::new(config()).expect("valid zero-sink builder");
    let Err(error) = zero_sink.build_canonical() else {
        panic!("zero-sink logger construction must fail");
    };
    assert_eq!(
        error.diagnostic().code,
        error_codes::LOGGER_INIT_FAILED,
        "zero-sink construction should use the logger initialization diagnostic"
    );
    assert!(
        error
            .diagnostic()
            .message
            .contains("at least one registered sink")
    );

    let mut valid = LoggerBuilder::new(config()).expect("valid builder");
    valid
        .register_typed_sink(Arc::new(RecordingTypedSink::default()))
        .expect("register a healthy sink");
    let logger = valid
        .build_canonical()
        .expect("a registered sink should permit construction");
    logger.shutdown();
}

#[test]
fn typed_registration_reports_duplicate_invalid_and_closed_sinks() {
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");
    let duplicate = Arc::new(RecordingTypedSink::default());
    builder
        .register_typed_sink(duplicate.clone())
        .expect("initial registration");
    let Err(error) = builder.register_typed_sink(duplicate) else {
        panic!("duplicate typed sink must fail");
    };
    assert_diagnostic_info(&error);
    assert_eq!(
        error.diagnostic().code,
        error_codes::SC_LOG_SINK_REGISTRATION_DUPLICATE
    );

    let invalid = Arc::new(RecordingTypedSink::default());
    *invalid.state.write().expect("sink health poisoned") = SinkHealthState::DegradedDropping;
    let Err(error) = builder.register_typed_sink(invalid) else {
        panic!("degraded typed sink must fail");
    };
    assert_diagnostic_info(&error);
    assert_eq!(
        error.diagnostic().code,
        error_codes::SC_LOG_SINK_REGISTRATION_INVALID
    );

    let closed = Arc::new(RecordingTypedSink::default());
    *closed.state.write().expect("sink health poisoned") = SinkHealthState::Unavailable;
    let Err(error) = builder.register_typed_sink(closed) else {
        panic!("unavailable typed sink must fail");
    };
    assert_diagnostic_info(&error);
    assert_eq!(
        error.diagnostic().code,
        error_codes::SC_LOG_SINK_REGISTRATION_CLOSED
    );
}

#[test]
fn typed_registration_adapter_preserves_failure_diagnostic_and_source() {
    struct FailingTypedSink;

    impl TypedLogSink for FailingTypedSink {
        fn write(&self, _: &LogEvent) -> Result<(), LogSinkError> {
            Err(LogSinkError::Write {
                context: Box::new(
                    ErrorContext::new(
                        ErrorCode::new_static("TYPED_REGISTRATION_WRITE"),
                        "typed sink write failed",
                        Remediation::recoverable("repair the typed sink", ["retry registration"]),
                    )
                    .source(Box::new(io::Error::other("typed source"))),
                ),
            })
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("typed-failure").expect("static sink name"),
                state: SinkHealthState::DegradedDropping,
                last_error: None,
            }
        }
    }

    let sink = legacy_sink(Arc::new(FailingTypedSink));
    let error = sink.write(&event()).expect_err("typed sink should fail");

    assert_eq!(error.diagnostic().code.as_str(), "TYPED_REGISTRATION_WRITE");
    let context = std::error::Error::source(&error).expect("legacy context source");
    assert_eq!(
        context.to_string(),
        "typed sink write failed; caused by: typed source"
    );
    assert!(
        context
            .source()
            .is_some_and(|source| source.is::<io::Error>())
    );
    assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
}
