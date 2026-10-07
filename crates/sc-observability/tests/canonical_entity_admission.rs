//! Canonical v2 admission coverage for `StateTransition::entity_id`.

use std::error::Error;

use sc_observability::v2::{EventError, Logger, LoggerConfig};
use sc_observability::{
    AdmissionOutcome, Level, LogEvent, LogQuery, ProcessIdentity, SchemaVersion, ServiceName,
    TargetCategory, Timestamp,
};
use sc_observability_types::{
    ActionName, EntityId, LevelFilter, StateName, StateTransition, ValueValidationError,
};

const INVALID_ENTITY_ID: &str = "entity invalid";

fn service_name() -> ServiceName {
    ServiceName::new("canonical-entity-admission").expect("valid service name")
}

fn event(entity_id: Option<&str>) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("app.core").expect("valid target"),
        action: ActionName::new("transition").expect("valid action"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: Some(StateTransition {
            entity_kind: TargetCategory::new("worker").expect("valid entity kind"),
            entity_id: entity_id.map(str::to_owned),
            from_state: StateName::new("idle").expect("valid state"),
            to_state: StateName::new("active").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: serde_json::Map::new(),
    }
}

fn event_count(logger: &Logger) -> usize {
    logger.flush().expect("flush");
    logger
        .query(&LogQuery::default())
        .expect("query")
        .events
        .len()
}

fn assert_validation(error: &EventError) {
    assert!(matches!(error, EventError::Validation { .. }), "{error:?}");
}

fn retained_validation_cause(error: &EventError) -> &ValueValidationError {
    let mut current: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(link) = current {
        if let Some(cause) = link.downcast_ref::<ValueValidationError>() {
            return cause;
        }
        current = link.source();
    }
    panic!("entity rejection must retain its ValueValidationError: {error:?}");
}

#[test]
fn canonical_entity_id_admission_rejects_invalid_values_without_queueing() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = Logger::new(LoggerConfig::default_for(
        service_name(),
        root.path().into(),
    ))
    .expect("canonical logger");

    logger
        .log(event(Some("worker-1")))
        .expect("valid entity id");
    logger.log(event(None)).expect("absent entity id");
    assert_eq!(event_count(&logger), 2);

    for result in [
        logger.log(event(Some(INVALID_ENTITY_ID))),
        logger.try_log(event(Some(INVALID_ENTITY_ID))),
        logger
            .try_log_with_outcome(event(Some(INVALID_ENTITY_ID)))
            .map(drop),
    ] {
        assert_validation(&result.expect_err("invalid entity id"));
    }
    assert_eq!(event_count(&logger), 2, "rejected events are never queued");
}

#[test]
fn canonical_entity_id_admission_respects_service_and_level_ordering() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut config = LoggerConfig::default_for(service_name(), root.path().into());
    config.level = LevelFilter::Info;
    let logger = Logger::new(config).expect("canonical logger");

    let mut wrong_service = event(Some(INVALID_ENTITY_ID));
    wrong_service.service = ServiceName::new("other-service").expect("valid service");
    let error = logger.log(wrong_service).expect_err("service mismatch");
    assert_validation(&error);
    assert!(
        error
            .diagnostic()
            .message
            .contains("service does not match")
    );

    let mut below_level = event(Some(INVALID_ENTITY_ID));
    below_level.level = Level::Debug;
    assert_eq!(
        logger
            .try_log_with_outcome(below_level.clone())
            .expect("filtered before entity validation"),
        AdmissionOutcome::Filtered
    );
    logger.log(below_level).expect("filtered log is successful");
}

#[test]
fn canonical_entity_id_rejection_retains_its_validation_cause() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = Logger::new(LoggerConfig::default_for(
        service_name(),
        root.path().into(),
    ))
    .expect("canonical logger");
    let empty = logger.log(event(Some(""))).expect_err("empty entity id");
    let malformed = logger
        .log(event(Some(INVALID_ENTITY_ID)))
        .expect_err("entity id outside the grammar");

    for error in [&empty, &malformed] {
        assert_validation(error);
        assert_eq!(
            error.diagnostic().message,
            "log event state transition entity_id is invalid"
        );
    }
    assert_eq!(
        retained_validation_cause(&empty),
        &EntityId::new("").expect_err("empty entity id")
    );
    assert_eq!(
        retained_validation_cause(&malformed),
        &EntityId::new(INVALID_ENTITY_ID).expect_err("malformed entity id")
    );
    assert_eq!(event_count(&logger), 0, "rejected events are never queued");
}
