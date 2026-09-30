//! Consumer coverage for typed sink registration in the logging builder.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use sc_observability::typed::{self, legacy_sink};
use sc_observability::v2::{LogSink as CanonicalLogSink, LoggerBuilder};
use sc_observability::*;
use sc_observability_types::DiagnosticInfo;
use sc_observability_types::typed::LogSinkFailure;
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

impl CanonicalLogSink for RecordingTypedSink {
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

    let logger = builder.build().expect("build typed logger");
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
    let Err(error) = zero_sink.build() else {
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
        .build()
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
fn typed_registration_detects_a_sink_already_added_through_raw_registration() {
    let duplicate = Arc::new(RecordingTypedSink::default());
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");
    builder.register_sink(SinkRegistration::typed(duplicate.clone()));

    let Err(error) = builder.register_typed_sink(duplicate) else {
        panic!("typed registration must detect the raw duplicate");
    };
    assert_diagnostic_info(&error);
    assert_eq!(
        error.diagnostic().code,
        error_codes::SC_LOG_SINK_REGISTRATION_DUPLICATE
    );
}

#[test]
fn raw_registration_accepts_degraded_sinks_and_preserves_filtering() {
    let degraded = Arc::new(RecordingTypedSink::default());
    *degraded.state.write().expect("sink health poisoned") = SinkHealthState::DegradedDropping;
    let filtered = Arc::new(RecordingTypedSink::default());
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");

    builder.register_sink(SinkRegistration::typed(degraded.clone()));
    builder
        .register_sink(SinkRegistration::typed(filtered.clone()).with_filter(Arc::new(AllowInfo)));
    let logger = builder
        .build()
        .expect("raw registration accepts both sinks");

    logger.log(event()).expect("admit info event");
    let mut rejected = event();
    rejected.level = Level::Error;
    logger.log(rejected).expect("admit error event");
    logger.flush().expect("wait for registered sinks");

    assert_eq!(
        degraded.health_snapshot().state,
        SinkHealthState::DegradedDropping
    );
    assert_eq!(degraded.writes.load(Ordering::SeqCst), 2);
    assert!(degraded.flushes.load(Ordering::SeqCst) >= 1);
    assert_eq!(filtered.writes.load(Ordering::SeqCst), 1);
    assert!(filtered.flushes.load(Ordering::SeqCst) >= 1);
}

#[test]
fn released_typed_adapter_preserves_failure_diagnostic_and_source() {
    struct FailingReleasedSink;

    impl typed::TypedLogSink for FailingReleasedSink {
        fn write(&self, _: &LogEvent) -> Result<(), LogSinkFailure> {
            Err(LogSinkFailure::from_context(Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("TYPED_REGISTRATION_WRITE"),
                    "typed sink write failed",
                    Remediation::recoverable("repair the typed sink", ["retry registration"]),
                )
                .source(Box::new(io::Error::other("typed source"))),
            )))
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("typed-failure").expect("static sink name"),
                state: SinkHealthState::DegradedDropping,
                last_error: None,
            }
        }
    }

    let sink = legacy_sink(Arc::new(FailingReleasedSink));
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
            .is_some_and(<dyn std::error::Error>::is::<io::Error>)
    );
    assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
}

/// Canonical sink that fails every write and flush and counts each attempt.
struct CanonicalFailingSink {
    writes: AtomicUsize,
    flushes: AtomicUsize,
}

impl CanonicalLogSink for CanonicalFailingSink {
    fn write(&self, _: &LogEvent) -> Result<(), LogSinkError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Err(LogSinkError::Write {
            context: Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("CANONICAL_RUNTIME_WRITE"),
                    "canonical runtime write failed",
                    Remediation::recoverable("repair the sink", ["retry the write"]),
                )
                .source(Box::new(io::Error::other("canonical write source"))),
            ),
        })
    }

    fn flush(&self) -> Result<(), LogSinkError> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Err(LogSinkError::Flush {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("CANONICAL_RUNTIME_FLUSH"),
                "canonical runtime flush failed",
                Remediation::recoverable("repair the sink", ["retry the flush"]),
            )),
        })
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("canonical-failing").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

/// Released-root sink that fails every write and flush and counts each attempt.
struct ReleasedFailingSink {
    writes: AtomicUsize,
    flushes: AtomicUsize,
}

#[expect(
    deprecated,
    reason = "the released root LogSink and its error wrapper are the contract under test"
)]
impl sc_observability::LogSink for ReleasedFailingSink {
    fn write(&self, _: &LogEvent) -> Result<(), sc_observability_types::LogSinkError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Err(sc_observability_types::LogSinkError(Box::new(
            ErrorContext::new(
                ErrorCode::new_static("RELEASED_RUNTIME_WRITE"),
                "released runtime write failed",
                Remediation::recoverable("repair the sink", ["retry the write"]),
            )
            .source(Box::new(io::Error::other("released write source"))),
        )))
    }

    fn flush(&self) -> Result<(), sc_observability_types::LogSinkError> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Err(sc_observability_types::LogSinkError(Box::new(
            ErrorContext::new(
                ErrorCode::new_static("RELEASED_RUNTIME_FLUSH"),
                "released runtime flush failed",
                Remediation::recoverable("repair the sink", ["retry the flush"]),
            ),
        )))
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("released-failing").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

#[test]
fn runtime_reports_canonical_sink_failures_with_fail_open_accounting() {
    let failing = Arc::new(CanonicalFailingSink {
        writes: AtomicUsize::new(0),
        flushes: AtomicUsize::new(0),
    });
    let healthy = Arc::new(RecordingTypedSink::default());
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");
    builder
        .register_typed_sink(failing.clone())
        .expect("register failing canonical sink")
        .register_typed_sink(healthy.clone())
        .expect("register healthy canonical sink");
    let logger = builder.build().expect("build logger");

    logger.log(event()).expect("admission stays fail-open");
    let flush = logger.flush().expect_err("flush reports the sink failure");
    assert_eq!(
        flush.diagnostic().code,
        error_codes::LOGGER_FLUSH_FAILED,
        "explicit flush reports the stable flush failure"
    );

    let health = logger.health();
    assert_eq!(
        health.dropped_events_total, 1,
        "one failed write is dropped"
    );
    assert_eq!(
        health
            .last_error
            .as_ref()
            .and_then(|summary| summary.code.as_ref()),
        Some(&error_codes::LOGGER_FLUSH_FAILED),
        "the failed flush is recorded in logger health"
    );
    assert_eq!(failing.writes.load(Ordering::SeqCst), 1);
    assert_eq!(failing.flushes.load(Ordering::SeqCst), 1);
    // The other sink is not starved by the failing sink.
    assert_eq!(healthy.writes.load(Ordering::SeqCst), 1);
    assert_eq!(healthy.flushes.load(Ordering::SeqCst), 1);

    logger.shutdown();
    // Shutdown performs exactly one additional final flush per sink.
    assert_eq!(failing.writes.load(Ordering::SeqCst), 1);
    assert_eq!(failing.flushes.load(Ordering::SeqCst), 2);
    assert_eq!(healthy.flushes.load(Ordering::SeqCst), 2);
}

#[test]
fn runtime_reports_released_sink_failures_with_fail_open_accounting() {
    let failing = Arc::new(ReleasedFailingSink {
        writes: AtomicUsize::new(0),
        flushes: AtomicUsize::new(0),
    });
    let mut builder = LoggerBuilder::new(config()).expect("valid builder");
    builder.register_sink(SinkRegistration::new(failing.clone()));
    let logger = builder.build().expect("build logger");

    logger.log(event()).expect("admission stays fail-open");
    let flush = logger.flush().expect_err("flush reports the sink failure");
    assert_eq!(
        flush.diagnostic().code,
        error_codes::LOGGER_FLUSH_FAILED,
        "explicit flush reports the stable flush failure"
    );

    let health = logger.health();
    assert_eq!(
        health.dropped_events_total, 1,
        "one failed write is dropped"
    );
    assert_eq!(failing.writes.load(Ordering::SeqCst), 1);
    assert_eq!(failing.flushes.load(Ordering::SeqCst), 1);

    logger.shutdown();
    assert_eq!(failing.writes.load(Ordering::SeqCst), 1);
    assert_eq!(failing.flushes.load(Ordering::SeqCst), 2);
}
