//! Public-surface checks for released versus canonical `emit_log` admission.
//!
//! An exporting (active) runtime can only be composed here with a backend
//! feature and a live endpoint, so the buffered-state assertions live next to
//! the private exporter injection seam in `src/facade_tests.rs`. This file
//! covers every state reachable through public entry points. The entity
//! rejection cause check composes a legacy HTTP/JSON runtime that rejects
//! every event before buffering, so it never exports.
#![allow(
    deprecated,
    reason = "the test asserts the retained released root facade and its legacy error variants"
)]

use std::sync::Arc;

use sc_observability_otlp::v2::{
    EventError, Telemetry as V2Telemetry, TelemetryConfigBuilder as V2TelemetryConfigBuilder,
    TelemetryError as V2TelemetryError, TelemetryProjectors as V2TelemetryProjectors,
};
use sc_observability_otlp::{
    LogsConfig, Telemetry, TelemetryConfigBuilder, TelemetryError, TelemetryProjectors,
};
use sc_observability_types::v2::FailureClassification;
use sc_observability_types::{
    ActionName, ErrorContext, Level, LogEvent, LogProjector, Observation, OutcomeLabel,
    ProcessIdentity, ProjectionError, ProjectionRegistration, Remediation, SchemaVersion,
    ServiceName, StateName, StateTransition, TargetCategory, Timestamp, error_codes,
};
use serde_json::Map;

const INVALID_ID: &str = "entity invalid";

fn service_name() -> ServiceName {
    ServiceName::new("released-emit-log").expect("valid service")
}

fn event_with_id(id: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("test.agent").expect("valid target"),
        action: ActionName::new("agent.observe").expect("valid action"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
        diagnostic: None,
        state_transition: Some(StateTransition {
            entity_kind: TargetCategory::new("agent").expect("valid target"),
            entity_id: Some(id.to_string()),
            from_state: StateName::new("idle").expect("valid state"),
            to_state: StateName::new("running").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: Map::default(),
    }
}

fn v2_disabled() -> V2Telemetry {
    let config = V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .build_typed()
        .expect("disabled config");
    V2Telemetry::new(config).expect("disabled telemetry")
}

fn root_disabled() -> Telemetry {
    let config = TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .build_typed()
        .expect("disabled config");
    Telemetry::new_typed(config).expect("disabled telemetry")
}

#[test]
fn v2_emit_log_inactive_with_invalid_id_returns_shutdown() {
    let telemetry = v2_disabled();
    telemetry.shutdown_typed().expect("shutdown");
    let error = telemetry
        .emit_log(&event_with_id(INVALID_ID))
        .expect_err("closed admission precedes the entity check");
    assert!(
        matches!(error, V2TelemetryError::Shutdown { .. }),
        "{error:?}"
    );
    assert_eq!(
        error.failure_classification(),
        FailureClassification::Closed
    );
}

#[test]
fn v2_emit_log_disabled_with_invalid_id_returns_ok() {
    v2_disabled()
        .emit_log(&event_with_id(INVALID_ID))
        .expect("disabled transport returns before the entity check");
}

#[test]
fn root_emit_log_inactive_returns_legacy_shutdown_and_disabled_returns_ok() {
    let closed = root_disabled();
    closed.shutdown_typed().expect("shutdown");
    assert!(matches!(
        closed.emit_log(&event_with_id(INVALID_ID)),
        Err(TelemetryError::Shutdown)
    ));

    root_disabled()
        .emit_log(&event_with_id(INVALID_ID))
        .expect("disabled transport accepts");
}

#[test]
fn event_variant_delegates_to_the_inner_event_error() {
    let native_source = "entity id rejected by admission";
    let context = ErrorContext::new(
        error_codes::DIAGNOSTIC_INVALID,
        "invalid entity id",
        Remediation::recoverable("fix the id", ["rebuild the event"]),
    )
    .source(Box::new(std::io::Error::other(native_source)));
    let error = V2TelemetryError::from(EventError::Validation {
        context: Box::new(context),
    });

    assert!(matches!(error, V2TelemetryError::Event(_)));
    assert_eq!(error.code(), error_codes::DIAGNOSTIC_INVALID);
    assert_eq!(
        error.failure_classification(),
        FailureClassification::validation("event")
    );
    assert_ne!(error.failure_classification(), FailureClassification::Io);
    assert_eq!(error.diagnostic().message, "invalid entity id");

    let context = error.into_context();
    let source =
        std::error::Error::source(context.as_ref()).expect("context preserves the native source");
    assert_eq!(source.to_string(), native_source);
}

struct InvalidIdProjector;

impl LogProjector<u8> for InvalidIdProjector {
    fn project_logs(
        &self,
        _observation: &Observation<u8>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        Ok(vec![event_with_id(INVALID_ID)])
    }
}

impl sc_observability_types::v2::LogProjector<u8> for InvalidIdProjector {
    fn project_logs(
        &self,
        _observation: &Observation<u8>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::v2::ProjectionError> {
        Ok(vec![event_with_id(INVALID_ID)])
    }
}

#[test]
fn projector_helpers_forward_to_their_own_admission_mode() {
    let observation = Observation::new(service_name(), 1_u8);

    let root: ProjectionRegistration<u8> = TelemetryProjectors::new(Arc::new(root_disabled()))
        .with_log_projector(Arc::new(InvalidIdProjector))
        .into_registration();
    let (projector, _, _, _) = root.into_parts();
    let events = projector
        .expect("root log projector")
        .project_logs(&observation)
        .expect("root projector ingress accepts");
    assert_eq!(events.len(), 1);

    let v2: sc_observability_types::v2::ProjectionRegistration<u8> =
        V2TelemetryProjectors::new(Arc::new(v2_disabled()))
            .with_log_projector(Arc::new(InvalidIdProjector))
            .into_registration();
    let (projector, _, _, _) = v2.into_parts();
    let events = projector
        .expect("v2 log projector")
        .project_logs(&observation)
        .expect("a disabled canonical runtime returns before the entity check");
    assert_eq!(events.len(), 1);
}

#[cfg(feature = "legacy-http-json")]
mod entity_rejection_cause {
    use sc_observability::error_codes::LOGGER_INVALID_EVENT;
    use sc_observability_otlp::LogsConfig;
    use sc_observability_otlp::v2::{
        EventError, ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol,
        Telemetry as V2Telemetry, TelemetryConfigBuilder as V2TelemetryConfigBuilder,
        TelemetryError as V2TelemetryError,
    };
    use sc_observability_types::v2::FailureClassification;
    use sc_observability_types::{EntityId, Remediation, ValueValidationError};

    use super::{INVALID_ID, event_with_id, service_name};

    fn enabled_legacy() -> V2Telemetry {
        let mut transport = OtelConfig::default();
        transport.enabled = true;
        transport.backend = ExporterBackend::LegacyHttpJson;
        transport.protocol = OtlpProtocol::HttpJson;
        transport.endpoint =
            Some(OtlpEndpoint::new_typed("http://127.0.0.1:4318").expect("endpoint"));
        let config = V2TelemetryConfigBuilder::new(service_name())
            .enable_logs(LogsConfig::default())
            .with_transport(transport)
            .build_typed()
            .expect("enabled legacy config");
        V2Telemetry::new(config).expect("enabled legacy telemetry")
    }

    fn retained_validation_cause(error: &V2TelemetryError) -> &ValueValidationError {
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
        while let Some(link) = current {
            if let Some(cause) = link.downcast_ref::<ValueValidationError>() {
                return cause;
            }
            current = link.source();
        }
        panic!("entity rejection must retain its ValueValidationError: {error:?}");
    }

    #[test]
    fn v2_entity_rejection_retains_the_distinct_validation_cause() {
        let telemetry = enabled_legacy();
        let empty = telemetry
            .emit_log(&event_with_id(""))
            .expect_err("empty entity_id");
        let grammar = telemetry
            .emit_log(&event_with_id(INVALID_ID))
            .expect_err("entity_id outside the identifier grammar");

        for error in [&empty, &grammar] {
            assert!(
                matches!(
                    error,
                    V2TelemetryError::Event(EventError::Validation { .. })
                ),
                "{error:?}"
            );
            assert_eq!(
                error.failure_classification(),
                FailureClassification::validation("event")
            );
            let diagnostic = error.diagnostic();
            assert_eq!(diagnostic.code, LOGGER_INVALID_EVENT);
            assert_eq!(
                diagnostic.message,
                "log event state transition entity_id is invalid"
            );
            assert_eq!(
                diagnostic.remediation,
                Remediation::recoverable(
                    "emit a valid entity_id or omit it",
                    ["rebuild the state transition before emitting"],
                )
            );
        }
        let empty_cause = retained_validation_cause(&empty);
        let grammar_cause = retained_validation_cause(&grammar);
        assert_eq!(empty_cause, &EntityId::new("").expect_err("empty id"));
        assert_eq!(
            grammar_cause,
            &EntityId::new(INVALID_ID).expect_err("id with a space")
        );
        assert_ne!(empty_cause, grammar_cause);
        telemetry
            .shutdown_typed()
            .expect("nothing was buffered to export");
    }
}
