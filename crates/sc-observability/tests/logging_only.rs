use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sc_observability::constants::{DEFAULT_LOG_DIR_NAME, DEFAULT_LOG_FILE_SUFFIX};
use sc_observability::typed::{TypedLogSink, legacy_sink};
use sc_observability::*;
use serde_json::json;

struct TestRoot(tempfile::TempDir);

impl TestRoot {
    fn path_buf(&self) -> PathBuf {
        self.0.path().to_path_buf()
    }
}

impl Deref for TestRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

fn temp_root(name: &str) -> TestRoot {
    TestRoot(
        tempfile::Builder::new()
            .prefix(&format!("sc-observability-integration-{name}-"))
            .tempdir()
            .expect("create temporary test root"),
    )
}

fn service_name() -> ServiceName {
    ServiceName::new("logging-only-app").expect("valid service name")
}

fn event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("app.core").expect("valid target"),
        action: ActionName::new("startup").expect("valid action"),
        message: Some("boot complete".to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST"),
            message: "integration".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect log output"]),
            docs: None,
            details: serde_json::Map::new(),
        }),
        state_transition: None,
        fields: serde_json::Map::from_iter([("attempt".to_string(), json!(1))]),
    }
}

#[test]
#[expect(
    deprecated,
    reason = "integration coverage intentionally exercises the deprecated emit() compatibility path"
)]
fn logging_only_consumer_can_emit_without_routing_or_otlp() {
    let root = temp_root("logging-only");
    let logger =
        Logger::new(LoggerConfig::default_for(service_name(), root.path_buf())).expect("logger");

    logger.emit(event()).expect("emit");
    logger.flush().expect("flush");

    let path = root
        .join(DEFAULT_LOG_DIR_NAME)
        .join(format!("logging-only-app{DEFAULT_LOG_FILE_SUFFIX}"));
    let contents = fs::read_to_string(path).expect("read log output");
    assert!(contents.contains("\"action\":\"startup\""));
    assert!(contents.contains("\"message\":\"boot complete\""));
}

#[test]
fn typed_sink_consumer_explicitly_imports_the_opt_in_trait() {
    struct ConsumerTypedSink;

    impl TypedLogSink for ConsumerTypedSink {
        fn write(
            &self,
            _event: &LogEvent,
        ) -> Result<(), sc_observability_types::typed::LogSinkFailure> {
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("typed-consumer").expect("valid sink name"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    let sink = legacy_sink(std::sync::Arc::new(ConsumerTypedSink));
    sink.write(&event()).expect("legacy adapter write");
    sink.flush().expect("default typed flush");
}

#[test]
#[allow(
    deprecated,
    reason = "the writer regression uses the retained LogSink boundary that owns the flush command"
)]
fn flush_command_flushes_each_sink_once_after_an_admitted_event() {
    struct CountingSink {
        writes: AtomicUsize,
        flushes: AtomicUsize,
    }

    impl LogSink for CountingSink {
        fn write(&self, _: &LogEvent) -> Result<(), sc_observability_types::LogSinkError> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn flush(&self) -> Result<(), sc_observability_types::LogSinkError> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("flush-counting").expect("valid sink name"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    let sink = Arc::new(CountingSink {
        writes: AtomicUsize::new(0),
        flushes: AtomicUsize::new(0),
    });
    let root = temp_root("single-flush");
    let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = Logger::builder(config).expect("valid builder");
    builder.register_sink(SinkRegistration::new(sink.clone()));
    let logger = builder.build();

    logger.log_typed(event()).expect("admit event");
    logger.flush_typed().expect("flush barrier");

    assert_eq!(sink.writes.load(Ordering::SeqCst), 1);
    assert_eq!(sink.flushes.load(Ordering::SeqCst), 1);
}
