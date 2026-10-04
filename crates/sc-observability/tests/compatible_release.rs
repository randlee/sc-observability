//! Package-consumer compatibility contract for the compatible 1.x release.
//!
//! A released 1.x consumer (crate-root `Logger`, `LogError`, ...) and a clean
//! v2 consumer (`sc_observability::v2`) are driven side by side through public
//! APIs only. Both must share the canonical runtime semantics (JSONL output,
//! health, flush, shutdown, query) while each keeps its own error contract.
//!
//! Stopped-typestate note: `Logger<Stopped>` deliberately exposes no
//! admission methods, so "admission after shutdown" is a compile-time
//! impossibility on both facades. The runtime shutdown contract is asserted
//! through the public follow session, which reports `QueryError::Shutdown`.
#![allow(deprecated, reason = "the test drives the released 1.x root facade")]

use std::fs;
use std::path::{Path, PathBuf};

use sc_observability::error_codes::LOGGER_INVALID_EVENT;
use sc_observability::v2::{EventError as V2EventError, Logger as V2Logger};
use sc_observability::{
    ActionName, AdmissionOutcome, JsonlLogReader, Level, LogError, LogEvent, LogFailure, LogQuery,
    Logger, LoggerConfig, LoggingHealthState, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp, TryLogError, TryLogFailure,
};
use sc_observability_types::error_codes::SC_LOG_QUERY_SHUTDOWN;
use sc_observability_types::typed::{ClassifiedError, EventFailure, EventFailureKind};
use sc_observability_types::{DiagnosticInfo, QueryError, StateName, StateTransition};
use serde_json::Value;

const SERVICE: &str = "compatible-release";
const INVALID_ENTITY_ID: &str = "entity invalid";

/// Caller-owned log root removed on success and on panic.
struct OwnedRoot(PathBuf);

impl OwnedRoot {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("sc-obs-compatible-release-")
            .tempdir()
            .expect("create temp root");
        Self(dir.keep())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        // Never panic in drop; NotFound means already clean and any other
        // failure is deliberately ignored.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn service() -> ServiceName {
    ServiceName::new(SERVICE).expect("valid service")
}

fn config(root: &OwnedRoot) -> LoggerConfig {
    LoggerConfig::default_for(service(), root.path().to_path_buf())
}

fn event(action: &str, message: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service(),
        target: TargetCategory::new("app.core").expect("valid target"),
        action: ActionName::new(action).expect("valid action"),
        message: Some(message.to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn entity_event(entity_id: &str) -> LogEvent {
    let mut event = event("transition", "entity transition");
    event.state_transition = Some(StateTransition {
        entity_kind: TargetCategory::new("worker").expect("valid target"),
        entity_id: Some(entity_id.to_string()),
        from_state: StateName::new("idle").expect("valid state"),
        to_state: StateName::new("active").expect("valid state"),
        reason: None,
        trigger: None,
    });
    event
}

fn wrong_service_event() -> LogEvent {
    let mut event = event("wrong-service", "service mismatch");
    event.service = ServiceName::new("other-service").expect("valid service");
    event
}

fn bad_version_event() -> LogEvent {
    let mut event = event("bad-version", "bad version");
    event.version = SchemaVersion::new("v0").expect("version parses");
    event
}

/// Reads the real JSONL file at `path` as parsed records.
fn read_jsonl(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("read jsonl log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("jsonl record parses"))
        .collect()
}

fn actions_and_messages(records: &[Value]) -> Vec<(String, String)> {
    records
        .iter()
        .map(|record| {
            (
                record["action"].as_str().expect("action").to_string(),
                record["message"].as_str().expect("message").to_string(),
            )
        })
        .collect()
}

fn released_invalid_code(error: &LogError) -> sc_observability::ErrorCode {
    match error {
        LogError::InvalidEvent(inner) => inner.diagnostic().code.clone(),
        other => panic!("expected released LogError::InvalidEvent, got {other:?}"),
    }
}

fn released_try_invalid_code(error: &TryLogError) -> sc_observability::ErrorCode {
    match error {
        TryLogError::InvalidEvent(inner) => inner.diagnostic().code.clone(),
        other => panic!("expected released TryLogError::InvalidEvent, got {other:?}"),
    }
}

fn v2_invalid_code(error: &V2EventError) -> sc_observability::ErrorCode {
    match error {
        V2EventError::Validation { .. } => error.diagnostic().code.clone(),
        other => panic!("expected canonical EventError::Validation, got {other:?}"),
    }
}

fn assert_typed_invalid(failure: &EventFailure) {
    assert_eq!(failure.kind(), EventFailureKind::InvalidEvent);
    assert_eq!(failure.diagnostic().code, LOGGER_INVALID_EVENT);
}

#[test]
fn released_and_v2_admit_the_same_event_into_the_same_jsonl_shape() {
    let released_root = OwnedRoot::new();
    let v2_root = OwnedRoot::new();
    let released = Logger::new(config(&released_root)).expect("released logger");
    let v2 = V2Logger::new(config(&v2_root)).expect("v2 logger");

    // Released admission paths.
    released.log(event("blocking", "one")).expect("log");
    released.try_log(event("try", "two")).expect("try_log");
    assert_eq!(
        released
            .try_log_with_outcome(event("outcome", "three"))
            .expect("try_log_with_outcome"),
        AdmissionOutcome::Accepted
    );
    // Canonical admission paths with identical events.
    v2.log(event("blocking", "one")).expect("log");
    v2.try_log(event("try", "two")).expect("try_log");
    assert_eq!(
        v2.try_log_with_outcome(event("outcome", "three"))
            .expect("try_log_with_outcome"),
        AdmissionOutcome::Accepted
    );

    released.flush().expect("released flush");
    v2.flush().expect("v2 flush");

    let released_path = released.health().active_log_path;
    let v2_path = v2.health().active_log_path;
    assert!(released_path.starts_with(released_root.path()));
    assert!(v2_path.starts_with(v2_root.path()));

    let released_records = read_jsonl(&released_path);
    let v2_records = read_jsonl(&v2_path);
    let expected = vec![
        ("blocking".to_string(), "one".to_string()),
        ("try".to_string(), "two".to_string()),
        ("outcome".to_string(), "three".to_string()),
    ];
    assert_eq!(actions_and_messages(&released_records), expected);
    assert_eq!(actions_and_messages(&v2_records), expected);
    assert_eq!(
        released_records, v2_records,
        "both facades persist byte-equivalent records for the same events"
    );

    let _ = released.shutdown();
    let _ = v2.shutdown();
}

#[test]
fn released_keeps_its_error_types_while_v2_returns_canonical_errors_with_same_codes() {
    let released_root = OwnedRoot::new();
    let v2_root = OwnedRoot::new();
    let released = Logger::new(config(&released_root)).expect("released logger");
    let v2 = V2Logger::new(config(&v2_root)).expect("v2 logger");

    // Shared invalid-input conditions: rejected by both with one code.
    for build in [wrong_service_event, bad_version_event] {
        let log_error = released.log(build()).expect_err("released log");
        let try_error = released.try_log(build()).expect_err("released try_log");
        let outcome_error = released
            .try_log_with_outcome(build())
            .expect_err("released try_log_with_outcome");
        let v2_error = v2.log(build()).expect_err("v2 log");
        let v2_try_error = v2.try_log(build()).expect_err("v2 try_log");
        let v2_outcome_error = v2
            .try_log_with_outcome(build())
            .expect_err("v2 try_log_with_outcome");

        assert_eq!(released_invalid_code(&log_error), LOGGER_INVALID_EVENT);
        assert_eq!(released_try_invalid_code(&try_error), LOGGER_INVALID_EVENT);
        assert_eq!(
            released_try_invalid_code(&outcome_error),
            LOGGER_INVALID_EVENT
        );
        assert_eq!(v2_invalid_code(&v2_error), LOGGER_INVALID_EVENT);
        assert_eq!(v2_invalid_code(&v2_try_error), LOGGER_INVALID_EVENT);
        assert_eq!(v2_invalid_code(&v2_outcome_error), LOGGER_INVALID_EVENT);

        // Typed surfaces converge on the same canonical EventFailure.
        match released.log_typed(build()).expect_err("released log_typed") {
            LogFailure::InvalidEvent(failure) => assert_typed_invalid(&failure),
            other => panic!("expected LogFailure::InvalidEvent, got {other:?}"),
        }
        match released
            .try_log_typed(build())
            .expect_err("released try_log_typed")
        {
            TryLogFailure::InvalidEvent(failure) => assert_typed_invalid(&failure),
            other => panic!("expected TryLogFailure::InvalidEvent, got {other:?}"),
        }
        assert_typed_invalid(&v2.log_typed(build()).expect_err("v2 log_typed"));
        assert_typed_invalid(&v2.try_log_typed(build()).expect_err("v2 try_log_typed"));
    }

    // Intentional divergence: released mode performs no entity-id check.
    released
        .log(entity_event(INVALID_ENTITY_ID))
        .expect("released root accepts entity-invalid events");
    released
        .log_typed(entity_event(INVALID_ENTITY_ID))
        .expect("released typed path accepts entity-invalid events");
    let v2_error = v2
        .log(entity_event(INVALID_ENTITY_ID))
        .expect_err("v2 rejects entity-invalid events");
    assert_eq!(v2_invalid_code(&v2_error), LOGGER_INVALID_EVENT);
    assert_typed_invalid(
        &v2.try_log_typed(entity_event(INVALID_ENTITY_ID))
            .expect_err("v2 typed rejection"),
    );

    // A valid entity id is accepted by both.
    released.log(entity_event("worker-1")).expect("released");
    v2.log(entity_event("worker-1")).expect("v2");

    released.flush().expect("released flush");
    v2.flush().expect("v2 flush");

    // Rejected events were never queued; only the accepted ones landed.
    let released_events = read_jsonl(&released.health().active_log_path);
    let v2_events = read_jsonl(&v2.health().active_log_path);
    let entity_ids = |records: &[Value]| -> Vec<String> {
        records
            .iter()
            .map(|record| {
                record["state_transition"]["entity_id"]
                    .as_str()
                    .expect("entity id")
                    .to_string()
            })
            .collect()
    };
    assert_eq!(
        entity_ids(&released_events),
        [INVALID_ENTITY_ID, INVALID_ENTITY_ID, "worker-1"]
    );
    assert_eq!(entity_ids(&v2_events), ["worker-1"]);

    let _ = released.shutdown();
    let _ = v2.shutdown();
}

#[test]
fn health_flush_query_and_shutdown_agree_across_released_and_v2() {
    let released_root = OwnedRoot::new();
    let v2_root = OwnedRoot::new();
    let released = Logger::new(config(&released_root)).expect("released logger");
    let v2 = V2Logger::new(config(&v2_root)).expect("v2 logger");

    // Health before admission.
    let before = (released.health(), v2.health());
    for health in [&before.0, &before.1] {
        assert_eq!(health.state, LoggingHealthState::Healthy);
    }
    assert_eq!(before.0.queue_capacity, before.1.queue_capacity);
    assert_eq!(before.0.writer_state, before.1.writer_state);

    // Follow sessions begin at the current end of each log.
    let mut released_follow = released.follow(LogQuery::default()).expect("follow");
    let mut v2_follow = v2.follow(LogQuery::default()).expect("follow");

    released.log(event("query", "alpha")).expect("log");
    v2.log(event("query", "alpha")).expect("log");

    // Flush: released FlushError, typed failure, and canonical flush all succeed.
    released.flush().expect("released flush");
    released.flush_typed().expect("released flush_typed");
    v2.flush().expect("v2 flush");
    v2.flush_typed().expect("v2 flush_typed");

    // Query through both facades and the standalone reader.
    let released_query = released.query(&LogQuery::default()).expect("query");
    let v2_query = v2.query(&LogQuery::default()).expect("query");
    let reader_query = JsonlLogReader::new(released.health().active_log_path)
        .query(&LogQuery::default())
        .expect("reader query");
    for snapshot in [&released_query, &v2_query, &reader_query] {
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.events[0].action.as_str(), "query");
        assert_eq!(snapshot.events[0].message.as_deref(), Some("alpha"));
    }
    for session in [&mut released_follow, &mut v2_follow] {
        let polled = session.poll().expect("follow poll");
        assert_eq!(polled.events.len(), 1);
        assert_eq!(polled.events[0].message.as_deref(), Some("alpha"));
    }

    // Shutdown: both facades stop through the shared runtime.
    let released_stopped = released.shutdown();
    let v2_stopped = v2.shutdown();
    for (health, path) in [
        (released_stopped.health(), released_root.path()),
        (v2_stopped.health(), v2_root.path()),
    ] {
        assert_eq!(health.state, LoggingHealthState::Unavailable);
        assert!(health.active_log_path.starts_with(path));
    }
    assert_eq!(
        released_stopped.health().writer_state,
        v2_stopped.health().writer_state
    );

    // Lifecycle error: live follow sessions observe the same shutdown error.
    for session in [&mut released_follow, &mut v2_follow] {
        let error = session.poll().expect_err("poll after shutdown");
        assert!(matches!(error, QueryError::Shutdown), "{error:?}");
        assert_eq!(error.code(), SC_LOG_QUERY_SHUTDOWN);
    }

    // Records survive shutdown in both real files.
    for path in [
        released_stopped.health().active_log_path,
        v2_stopped.health().active_log_path,
    ] {
        assert_eq!(
            actions_and_messages(&read_jsonl(&path)),
            [("query".to_string(), "alpha".to_string())]
        );
    }
}
