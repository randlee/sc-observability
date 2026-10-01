//! Released 1.x `StateTransition::entity_id` acceptance versus canonical admission.
//!
//! The root facade keeps exact 1.4.1 acceptance (`Option<String>`, no entity
//! check). The v2 facade validates the identifier as an `EntityId` at
//! admission, after the service and level checks.
#![expect(
    deprecated,
    reason = "the test exercises the released root facade signatures"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sc_observability::constants::{
    DEFAULT_LOG_DIR_NAME, DEFAULT_LOG_FILE_SUFFIX, MAX_LOG_EVENT_BYTES,
};
use sc_observability::v2::{EventError, Logger as CanonicalLogger};
use sc_observability::{
    AdmissionOutcome, JsonlLogReader, Level, LogEvent, LogQuery, Logger, LoggerConfig, Redactor,
    ServiceName,
};
use sc_observability_types::LevelFilter;
use sc_observability_types::v2::FailureClassification;
use sc_observability_types::{
    ActionName, EntityId, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, Remediation,
    SchemaVersion, StateName, StateTransition, TargetCategory, Timestamp, ValueValidationError,
};

const INVALID_ID: &str = "entity invalid";

struct CountingRedactor(Arc<AtomicUsize>);

impl Redactor for CountingRedactor {
    fn redact(&self, _key: &str, _value: &mut serde_json::Value) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn service_name() -> ServiceName {
    ServiceName::new("released-state-app").expect("valid service name")
}

fn config(root: &tempfile::TempDir) -> LoggerConfig {
    LoggerConfig::default_for(service_name(), root.path().to_path_buf())
}

fn event(entity_id: Option<&str>) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid version"),
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
            entity_kind: TargetCategory::new("worker").expect("valid target"),
            entity_id: entity_id.map(String::from),
            from_state: StateName::new("idle").expect("valid state"),
            to_state: StateName::new("active").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: serde_json::Map::new(),
    }
}

fn root_count(logger: &Logger) -> usize {
    logger.flush().expect("flush");
    logger
        .query(&LogQuery::default())
        .expect("query")
        .events
        .len()
}

fn v2_count(logger: &CanonicalLogger) -> usize {
    logger.flush().expect("flush");
    logger
        .query(&LogQuery::default())
        .expect("query")
        .events
        .len()
}

fn assert_event_validation(error: &EventError) {
    assert!(matches!(error, EventError::Validation { .. }), "{error:?}");
    assert_eq!(
        error.failure_classification(),
        FailureClassification::validation("event")
    );
}

fn retained_validation_cause(error: &EventError) -> &ValueValidationError {
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
fn released_literal_with_string_entity_id_compiles() {
    let transition = StateTransition {
        entity_kind: TargetCategory::new("worker").expect("valid target"),
        entity_id: Some(String::from("worker-1")),
        from_state: StateName::new("idle").expect("valid state"),
        to_state: StateName::new("active").expect("valid state"),
        reason: None,
        trigger: None,
    };
    assert_eq!(transition.entity_id.as_deref(), Some("worker-1"));
}

#[test]
fn root_facade_accepts_invalid_entity_id_on_every_admission_path() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = Logger::new(config(&root)).expect("logger");

    logger.log(event(Some(INVALID_ID))).expect("log");
    assert_eq!(root_count(&logger), 1);
    logger.try_log(event(Some(INVALID_ID))).expect("try_log");
    assert_eq!(root_count(&logger), 2);
    assert_eq!(
        logger
            .try_log_with_outcome(event(Some(INVALID_ID)))
            .expect("try_log_with_outcome"),
        AdmissionOutcome::Accepted
    );
    assert_eq!(root_count(&logger), 3);
    logger.emit(event(Some(INVALID_ID))).expect("emit");
    assert_eq!(root_count(&logger), 4);

    let snapshot = logger.query(&LogQuery::default()).expect("query");
    assert!(snapshot.events.iter().all(|stored| {
        stored
            .state_transition
            .as_ref()
            .and_then(|transition| transition.entity_id.as_deref())
            == Some(INVALID_ID)
    }));
}

#[test]
fn v2_facade_rejects_invalid_entity_id_without_queueing() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = CanonicalLogger::new(config(&root)).expect("logger");
    logger.log(event(Some("worker-1"))).expect("valid id");
    assert_eq!(v2_count(&logger), 1);

    let error = logger.log(event(Some(INVALID_ID))).expect_err("log");
    assert_event_validation(&error);
    let error = logger
        .try_log(event(Some(INVALID_ID)))
        .expect_err("try_log");
    assert_event_validation(&error);
    let error = logger
        .try_log_with_outcome(event(Some(INVALID_ID)))
        .expect_err("try_log_with_outcome");
    assert_event_validation(&error);

    assert_eq!(v2_count(&logger), 1, "rejected events are never queued");
}

#[test]
fn v2_entity_validation_precedes_redaction_and_event_size_validation() {
    let root = tempfile::tempdir().expect("tempdir");
    let redaction_calls = Arc::new(AtomicUsize::new(0));
    let mut logger_config = config(&root);
    logger_config
        .redaction
        .custom_redactors
        .push(Box::new(CountingRedactor(redaction_calls.clone())));
    let logger = CanonicalLogger::new(logger_config).expect("logger");
    let mut oversized = event(Some(INVALID_ID));
    oversized.fields.insert(
        "oversized".into(),
        serde_json::Value::String("x".repeat(MAX_LOG_EVENT_BYTES)),
    );

    let error = logger.log(oversized).expect_err("invalid entity id");
    assert_event_validation(&error);
    assert_eq!(
        error.diagnostic().message,
        "log event state transition entity_id is invalid"
    );
    assert_ne!(
        error.diagnostic().message,
        "log event exceeds the maximum serialized size"
    );
    assert_eq!(
        redaction_calls.load(Ordering::SeqCst),
        0,
        "canonical entity validation rejects before custom redaction"
    );
}

#[test]
fn valid_and_absent_entity_ids_are_admitted_on_root_and_v2() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let root = Logger::new(config(&root_dir)).expect("logger");
    root.log(event(Some("worker-1"))).expect("root valid");
    root.log(event(None)).expect("root absent");
    assert_eq!(root_count(&root), 2);

    let v2_dir = tempfile::tempdir().expect("tempdir");
    let v2 = CanonicalLogger::new(config(&v2_dir)).expect("logger");
    v2.log(event(Some("worker-1"))).expect("v2 valid");
    v2.log(event(None)).expect("v2 absent");
    assert_eq!(v2_count(&v2), 2);
}

#[test]
fn service_mismatch_takes_precedence_over_entity_check() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let root = Logger::new(config(&root_dir)).expect("logger");
    let v2_dir = tempfile::tempdir().expect("tempdir");
    let v2 = CanonicalLogger::new(config(&v2_dir)).expect("logger");

    let mut wrong_service = event(Some(INVALID_ID));
    wrong_service.service = ServiceName::new("other-service").expect("valid service");

    let root_error = root
        .log(wrong_service.clone())
        .expect_err("service mismatch");
    let v2_error = v2.log(wrong_service).expect_err("service mismatch");
    assert_event_validation(&v2_error);
    assert!(
        v2_error
            .diagnostic()
            .message
            .contains("service does not match"),
        "{v2_error:?}"
    );
    assert!(
        root_error.to_string().contains("service does not match"),
        "{root_error}"
    );
}

#[test]
fn below_level_event_with_invalid_entity_id_is_filtered_on_v2() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut config = config(&root);
    config.level = LevelFilter::Info;
    let logger = CanonicalLogger::new(config).expect("logger");

    let mut below_level = event(Some(INVALID_ID));
    below_level.level = Level::Debug;
    assert_eq!(
        logger
            .try_log_with_outcome(below_level.clone())
            .expect("filtered, not rejected"),
        AdmissionOutcome::Filtered
    );
    logger.log(below_level).expect("filtered log is Ok");
}

#[test]
fn stored_invalid_entity_id_still_decodes_on_readback() {
    let root = tempfile::tempdir().expect("tempdir");
    let log_dir = root.path().join(DEFAULT_LOG_DIR_NAME);
    fs::create_dir_all(&log_dir).expect("log dir");
    let path: PathBuf = log_dir.join(format!("released-state-app{DEFAULT_LOG_FILE_SUFFIX}"));
    let line = serde_json::to_string(&event(Some(INVALID_ID))).expect("serialize");
    assert!(line.contains(INVALID_ID));
    fs::write(&path, format!("{line}\n")).expect("write stored line");

    let snapshot = JsonlLogReader::new(path)
        .query(&LogQuery::default())
        .expect("lenient readback");
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(
        snapshot.events[0]
            .state_transition
            .as_ref()
            .and_then(|transition| transition.entity_id.as_deref()),
        Some(INVALID_ID)
    );

    let logger = Logger::new(config(&root)).expect("logger");
    assert_eq!(
        logger
            .query(&LogQuery::default())
            .expect("query readback")
            .events
            .len(),
        1
    );
}

#[test]
fn v2_entity_rejection_retains_the_distinct_validation_cause() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = CanonicalLogger::new(config(&root)).expect("logger");
    let empty = logger.log(event(Some(""))).expect_err("empty entity_id");
    let grammar = logger
        .log(event(Some(INVALID_ID)))
        .expect_err("entity_id outside the identifier grammar");

    for error in [&empty, &grammar] {
        assert_event_validation(error);
        let diagnostic = error.diagnostic();
        assert_eq!(
            diagnostic.code,
            sc_observability::error_codes::LOGGER_INVALID_EVENT
        );
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
    assert_eq!(v2_count(&logger), 0, "rejected events are never queued");

    let root_dir = tempfile::tempdir().expect("tempdir");
    let released = Logger::new(config(&root_dir)).expect("logger");
    released
        .log(event(Some("")))
        .expect("root accepts an empty id");
    released
        .log(event(Some(INVALID_ID)))
        .expect("root accepts an id outside the grammar");
    assert_eq!(root_count(&released), 2);
}
