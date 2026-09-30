//! Projection admission follows the facade that built the runtime.
//!
//! A root-built runtime keeps exact 1.4.1 acceptance of a projected
//! `StateTransition::entity_id`; a v2-built runtime validates it at canonical
//! admission and records a projection failure. Flush-failure identity per
//! facade is covered by the in-crate tests, where a failing sink can be
//! injected into the logger.
#![allow(
    deprecated,
    reason = "the released projector trait returns the retained root ProjectionError"
)]

use std::path::PathBuf;
use std::sync::Arc;

use sc_observability_types::ProjectionError;
use sc_observability_types::{
    ActionName, Level, LogEvent, LogProjector, Observation, OutcomeLabel, ProcessIdentity,
    ProjectionRegistration, SchemaVersion, ServiceName, StateName, StateTransition, TargetCategory,
    Timestamp, ToolName,
};
use sc_observe::{Observability, ObservabilityConfig};
use serde_json::Map;

const INVALID_ID: &str = "entity invalid";

#[derive(Debug, Clone)]
struct Payload;

struct EntityProjector(&'static str);

impl LogProjector<Payload> for EntityProjector {
    fn project_logs(
        &self,
        observation: &Observation<Payload>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        Ok(vec![LogEvent {
            version: SchemaVersion::new(
                sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
            )
            .expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: observation.service.clone(),
            target: TargetCategory::new("observe.routing").expect("valid target"),
            action: ActionName::new("observation.received").expect("valid action"),
            message: None,
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
            diagnostic: None,
            state_transition: Some(StateTransition {
                entity_kind: TargetCategory::new("agent").expect("valid target"),
                entity_id: Some(self.0.to_string()),
                from_state: StateName::new("idle").expect("valid state"),
                to_state: StateName::new("running").expect("valid state"),
                reason: None,
                trigger: None,
            }),
            fields: Map::default(),
        }])
    }
}

fn tool_name() -> ToolName {
    ToolName::new("obs-app").expect("valid tool name")
}

fn observation() -> Observation<Payload> {
    Observation::new(ServiceName::new("obs-app").expect("valid service"), Payload)
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "sc-observe-released-admission-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ))
}

fn registration(id: &'static str) -> ProjectionRegistration<Payload> {
    ProjectionRegistration::new().with_log_projector(Arc::new(EntityProjector(id)))
}

fn root_runtime(name: &str, id: &'static str) -> Observability {
    let config =
        ObservabilityConfig::default_for_typed(tool_name(), temp_path(name)).expect("root config");
    Observability::builder(config)
        .register_projection(registration(id))
        .build_typed()
        .expect("root runtime")
}

fn v2_runtime(name: &str, id: &'static str) -> sc_observe::v2::Observability {
    let config = sc_observe::v2::ObservabilityConfig::default_for(tool_name(), temp_path(name))
        .expect("v2 config");
    sc_observe::v2::Observability::builder(config)
        .register_projection(registration(id).into())
        .build()
        .expect("v2 runtime")
}

#[test]
fn root_built_runtime_accepts_invalid_projected_entity_id() {
    let runtime = root_runtime("root-invalid", INVALID_ID);
    runtime.emit(observation()).expect("emit");
    let health = runtime.health();
    assert_eq!(health.projection_failures_total, 0);
    assert!(health.last_error.is_none(), "{:?}", health.last_error);
    let logging = health.logging.expect("logging health");
    assert_eq!(logging.flush_errors_total, 0);
}

#[test]
fn v2_built_runtime_counts_one_projection_failure_with_validation_code() {
    let runtime = v2_runtime("v2-invalid", INVALID_ID);
    runtime.emit(observation()).expect("emit still matches");
    let health = runtime.health();
    assert_eq!(health.projection_failures_total, 1);
    let summary = health.last_error.expect("last error recorded");
    assert_eq!(
        summary.code.as_ref(),
        Some(&sc_observability::error_codes::LOGGER_INVALID_EVENT)
    );
}

#[test]
fn valid_projected_entity_ids_pass_on_both_facades() {
    let root = root_runtime("root-valid", "agent-1");
    root.emit(observation()).expect("root emit");
    assert_eq!(root.health().projection_failures_total, 0);
    assert!(root.health().last_error.is_none());

    let v2 = v2_runtime("v2-valid", "agent-1");
    v2.emit(observation()).expect("v2 emit");
    assert_eq!(v2.health().projection_failures_total, 0);
    assert!(v2.health().last_error.is_none());
}
