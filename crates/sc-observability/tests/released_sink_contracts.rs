//! Downstream compatibility coverage for the released sink trait objects.

use std::io;
use std::sync::Arc;

use sc_observability::typed::{TypedLogSink, legacy_sink, typed_sink};
use sc_observability::*;
use sc_observability_types::v2::LogSinkError as TypedSinkError;
use sc_observability_types::{DiagnosticInfo, ErrorCode, Remediation};
use serde_json::Map;

fn assert_send_sync<T: Send + Sync>(_: &T) {}

struct LegacySink;

#[expect(
    deprecated,
    reason = "the released root LogSink trait is the compatibility contract under test"
)]
impl LogSink for LegacySink {
    fn write(&self, _: &LogEvent) -> Result<(), sc_observability_types::LogSinkError> {
        Err(sc_observability_types::LogSinkError(Box::new(
            ErrorContext::new(
                ErrorCode::new_static("RELEASED_SINK_FAILURE"),
                "legacy sink failed",
                Remediation::recoverable("replace the sink", ["retry registration"]),
            )
            .source(Box::new(io::Error::other("legacy sink source"))),
        )))
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("released-legacy").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

struct TypedSink;

impl TypedLogSink for TypedSink {
    fn write(&self, _: &LogEvent) -> Result<(), TypedSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("released-typed").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

fn config() -> LoggerConfig {
    let root = tempfile::tempdir().expect("temporary root");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("released-sink-contracts").expect("static service name"),
        root.path().to_path_buf(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config
}

#[test]
#[expect(
    deprecated,
    reason = "the test intentionally verifies released root builder registration and build contracts"
)]
fn released_sink_trait_objects_are_thread_safe_and_registerable() {
    let legacy: Arc<dyn LogSink> = Arc::new(LegacySink);
    let typed: Arc<dyn TypedLogSink> = Arc::new(TypedSink);
    assert_send_sync(&legacy);
    assert_send_sync(&typed);

    let mut legacy_builder = LoggerBuilder::new(config()).expect("released builder");
    legacy_builder.register_sink(SinkRegistration::new(legacy));
    legacy_builder.build().shutdown();

    let mut typed_builder = LoggerBuilder::new(config()).expect("released builder");
    typed_builder
        .register_typed_sink(typed)
        .expect("released typed registration");
    typed_builder
        .build_typed()
        .expect("released typed build")
        .shutdown();
}

#[test]
fn released_sink_adapters_round_trip_and_preserve_failure_sources() {
    let legacy: Arc<dyn LogSink> = Arc::new(LegacySink);
    let typed = typed_sink(legacy.clone());
    let round_trip = legacy_sink(typed.clone());
    let error = typed
        .write(&test_event())
        .expect_err("legacy-to-typed adapter preserves failure");
    assert_eq!(error.diagnostic().code.as_str(), "RELEASED_SINK_FAILURE");
    assert_eq!(
        std::error::Error::source(&error)
            .expect("typed error preserves the legacy sink source")
            .to_string(),
        "legacy sink source"
    );
    let error = round_trip
        .write(&test_event())
        .expect_err("typed-to-legacy adapter preserves failure");
    assert_eq!(error.diagnostic().code.as_str(), "RELEASED_SINK_FAILURE");
    assert_eq!(
        std::error::Error::source(&error)
            .and_then(std::error::Error::source)
            .expect("legacy error preserves the underlying sink source")
            .to_string(),
        "legacy sink source"
    );
}

fn test_event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("static version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("released-sink-contracts").expect("static service name"),
        target: TargetCategory::new("released.sink").expect("static target"),
        action: ActionName::new("test").expect("static action"),
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
