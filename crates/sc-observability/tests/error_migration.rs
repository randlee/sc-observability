use std::error::Error;
use std::fs;
use std::io::Write as _;

use sc_observability::constants::{DEFAULT_LOG_DIR_NAME, DEFAULT_LOG_FILE_SUFFIX};
use sc_observability::error_codes;
use sc_observability::*;
use sc_observability_types::{
    DiagnosticInfo, QueryError, QueryHealthState,
    error_codes::{SC_LOG_QUERY_DECODE, SC_LOG_QUERY_SHUTDOWN},
};
use serde_json::json;

fn service_name() -> ServiceName {
    ServiceName::new("error-migration").expect("static service name is valid")
}

fn event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("published schema version is valid"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("app.core").expect("static target is valid"),
        action: ActionName::new("startup").expect("static action is valid"),
        message: Some("boot complete".to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some(OutcomeLabel::new("ok").expect("static outcome is valid")),
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

fn logger(root: &tempfile::TempDir) -> Logger {
    Logger::new_typed(LoggerConfig::default_for(
        service_name(),
        root.path().to_path_buf(),
    ))
    .expect("logger starts")
}

#[test]
#[expect(
    deprecated,
    reason = "the production compatibility facade maps invalid events to EventError"
)]
fn logger_emit_preserves_real_event_validation_diagnostic() {
    let root = tempfile::tempdir().expect("temporary log root");
    let logger = logger(&root);
    let mut invalid_event = event();
    invalid_event.version = SchemaVersion::new("v0").expect("test version parses");

    let error = logger
        .emit(invalid_event)
        .expect_err("Logger::emit maps the production validation failure");

    assert_eq!(error.diagnostic().code, error_codes::LOGGER_INVALID_EVENT);
    assert!(matches!(
        error.diagnostic().remediation,
        Remediation::Recoverable { .. }
    ));
    assert!(
        Error::source(&error).is_some(),
        "EventError preserves the production validation context as its source"
    );

    let _stopped = logger.shutdown();
}

#[test]
fn logger_follow_shutdown_updates_real_query_health() {
    let root = tempfile::tempdir().expect("temporary log root");
    let logger = logger(&root);
    let mut follow = logger.follow(LogQuery::default()).expect("follow session");
    let stopped = logger.shutdown();

    let shutdown = follow
        .poll()
        .expect_err("follow observes the real shutdown path");
    assert!(matches!(shutdown, QueryError::Shutdown));
    assert_eq!(shutdown.code(), SC_LOG_QUERY_SHUTDOWN);
    assert!(matches!(
        shutdown.diagnostic().remediation,
        Remediation::Recoverable { .. }
    ));
    assert!(
        Error::source(&shutdown).is_none(),
        "the source-free shutdown contract must not manufacture a source"
    );

    let health = follow.health();
    assert_eq!(health.state, QueryHealthState::Unavailable);
    assert_eq!(
        health.last_error.expect("shutdown health summary").code,
        Some(SC_LOG_QUERY_SHUTDOWN)
    );
    assert_eq!(
        stopped.health().query.expect("stopped query health").state,
        QueryHealthState::Unavailable
    );
}

#[test]
fn logger_query_decode_preserves_source_and_records_health_summary() {
    let root = tempfile::tempdir().expect("temporary log root");
    let logger = logger(&root);
    logger.log_typed(event()).expect("admit event");
    logger.flush_typed().expect("flush event");

    let active_path = root
        .path()
        .join(DEFAULT_LOG_DIR_NAME)
        .join(format!("error-migration{DEFAULT_LOG_FILE_SUFFIX}"));
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(active_path)
        .expect("open active log");
    writeln!(file, "{{not-json").expect("append malformed JSONL");

    let decode = logger
        .query(&LogQuery::default())
        .expect_err("malformed persisted data fails through Logger::query");
    let QueryError::Decode(context) = decode else {
        panic!("expected the production query decode variant");
    };
    assert_eq!(context.diagnostic().code, SC_LOG_QUERY_DECODE);
    assert!(matches!(
        context.diagnostic().remediation,
        Remediation::Recoverable { .. }
    ));
    Error::source(context.as_ref())
        .and_then(|source| source.downcast_ref::<serde_json::Error>())
        .expect("the real decode context preserves its serde_json source");

    let health = logger.health().query.expect("query health");
    assert_eq!(health.state, QueryHealthState::Degraded);
    assert_eq!(
        health.last_error.expect("decode health summary").code,
        Some(SC_LOG_QUERY_DECODE),
        "health records the stable code; DiagnosticSummary has no source or remediation fields"
    );

    let _stopped = logger.shutdown();
}
