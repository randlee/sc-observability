//! Downstream compatibility coverage for the released sink trait objects.

use std::io;
use std::sync::Arc;

use sc_observability::typed::{TypedLogSink, legacy_sink, typed_sink};
use sc_observability::v2::LogSink as CanonicalLogSink;
use sc_observability::*;
use sc_observability_types::typed::LogSinkFailure;
use sc_observability_types::v2::LogSinkError as CanonicalLogSinkError;
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
    fn write(&self, _: &LogEvent) -> Result<(), LogSinkFailure> {
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

/// Canonical registration sink, kept apart from the released typed fixture.
struct CanonicalSink;

impl CanonicalLogSink for CanonicalSink {
    fn write(&self, _: &LogEvent) -> Result<(), CanonicalLogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("canonical-registration").expect("static sink name"),
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
    typed_builder.register_sink(SinkRegistration::new(legacy_sink(typed)));
    typed_builder
        .register_typed_sink(Arc::new(CanonicalSink))
        .expect("canonical typed registration");
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
    // The released typed failure wraps the preserved context, whose own
    // source is the original legacy sink error.
    assert_eq!(
        std::error::Error::source(&error)
            .and_then(std::error::Error::source)
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

/// Compile-time pins of the released 1.4.1 sink trait and adapter signatures.
///
/// The semver gate cannot see a changed associated-type error in a trait that
/// downstream crates implement, so these coercions fail to compile if the
/// released forms drift.
#[expect(
    deprecated,
    reason = "the released root LogSink error wrapper is the contract under test"
)]
#[test]
fn released_sink_trait_and_adapter_signatures_are_pinned() {
    let _: fn(&LegacySink, &LogEvent) -> Result<(), sc_observability_types::LogSinkError> =
        <LegacySink as LogSink>::write;
    let _: fn(&LegacySink) -> Result<(), sc_observability_types::LogSinkError> =
        <LegacySink as LogSink>::flush;
    let _: fn(&LegacySink) -> SinkHealth = <LegacySink as LogSink>::health;

    let _: fn(&TypedSink, &LogEvent) -> Result<(), LogSinkFailure> =
        <TypedSink as TypedLogSink>::write;
    let _: fn(&TypedSink) -> Result<(), LogSinkFailure> = <TypedSink as TypedLogSink>::flush;
    let _: fn(&TypedSink) -> SinkHealth = <TypedSink as TypedLogSink>::health;

    let _: fn(Arc<dyn TypedLogSink>) -> Arc<dyn LogSink> = legacy_sink;
    let _: fn(Arc<dyn LogSink>) -> Arc<dyn TypedLogSink> = typed_sink;
    let _: fn(Arc<dyn LogSink>) -> SinkRegistration = SinkRegistration::new;

    // The released traits stay open and object safe, and keep their default
    // flush.
    let typed: Arc<dyn TypedLogSink> = Arc::new(TypedSink);
    typed.flush().expect("released typed default flush");
    let legacy: Arc<dyn LogSink> = Arc::new(LegacySink);
    legacy.flush().expect("released root default flush");
}

#[test]
fn canonical_and_released_sink_contracts_are_distinct_trait_slots() {
    let _: fn(&CanonicalSink, &LogEvent) -> Result<(), CanonicalLogSinkError> =
        <CanonicalSink as CanonicalLogSink>::write;
    let _: fn(&CanonicalSink) -> Result<(), CanonicalLogSinkError> =
        <CanonicalSink as CanonicalLogSink>::flush;
    let _: fn(&CanonicalSink) -> SinkHealth = <CanonicalSink as CanonicalLogSink>::health;
    let _: fn(SinkRegistration, Arc<dyn LogFilter>) -> SinkRegistration =
        SinkRegistration::with_filter;
    let _: fn(Arc<dyn CanonicalLogSink>) -> SinkRegistration = SinkRegistration::typed;

    let canonical: Arc<dyn CanonicalLogSink> = Arc::new(CanonicalSink);
    canonical.flush().expect("canonical default flush");
}
