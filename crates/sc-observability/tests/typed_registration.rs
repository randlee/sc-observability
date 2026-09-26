//! Consumer coverage for typed sink registration in the logging builder.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use sc_observability::typed::{TypedLogSink, legacy_sink};
use sc_observability::*;
use sc_observability_types::v2::LogSinkError;
use serde_json::Map;

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
    let mut builder = LoggerBuilder::new_typed(config()).expect("valid builder");

    builder.register_sink(
        SinkRegistration::typed(registration_sink.clone()).with_filter(Arc::new(AllowInfo)),
    );
    builder
        .register_typed_sink(builder_sink.clone())
        .register_typed_sink(chained_sink.clone());

    let logger = builder.build_typed().expect("build typed logger");
    logger.log_typed(event()).expect("admit event");
    logger.flush_typed().expect("flush typed sinks");

    for sink in [&registration_sink, &builder_sink, &chained_sink] {
        assert_eq!(sink.writes.load(Ordering::SeqCst), 1);
        // The explicit flush is the sole sink flush for the admitted event.
        // Registration never duplicates either operation.
        assert_eq!(sink.flushes.load(Ordering::SeqCst), 1);
        assert_eq!(sink.health_snapshot().state, SinkHealthState::Healthy);
    }
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
    assert_eq!(
        std::error::Error::source(&error)
            .map(ToString::to_string)
            .as_deref(),
        Some("typed sink write failed; caused by: typed source")
    );
    assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
}
