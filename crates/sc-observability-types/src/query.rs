use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    ActionName, CorrelationId, Diagnostic, DiagnosticInfo, ErrorCode, ErrorContext, Level,
    LogEvent, Remediation, ServiceName, TargetCategory, Timestamp, error_codes, sealed,
};

/// Deterministic result ordering for historical query and follow polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogOrder {
    #[default]
    /// Return records from oldest to newest.
    OldestFirst,
    /// Return records from newest to oldest.
    NewestFirst,
}

/// One exact-match field filter in a historical/follow log query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogFieldMatch {
    /// Structured field name to compare.
    pub field: String,
    /// Exact JSON value to match.
    pub value: Value,
}

impl LogFieldMatch {
    /// Creates an exact-value field match.
    pub fn equals(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            value,
        }
    }
}

/// Shared historical/follow query contract used by the logging reader/runtime layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogQuery {
    /// Optional service filter.
    pub service: Option<ServiceName>,
    /// Allowed severity levels; empty means any level.
    pub levels: Vec<Level>,
    /// Optional target/category filter.
    pub target: Option<TargetCategory>,
    /// Optional action filter.
    pub action: Option<ActionName>,
    /// Optional request identifier filter.
    pub request_id: Option<CorrelationId>,
    /// Optional correlation identifier filter.
    pub correlation_id: Option<CorrelationId>,
    /// Optional inclusive lower timestamp bound.
    pub since: Option<Timestamp>,
    /// Optional inclusive upper timestamp bound.
    pub until: Option<Timestamp>,
    /// Exact-match field predicates.
    pub field_matches: Vec<LogFieldMatch>,
    /// Optional maximum number of returned events.
    pub limit: Option<usize>,
    /// Result ordering.
    pub order: LogOrder,
}

impl LogQuery {
    /// Validates the frozen shared query semantics before runtime execution.
    pub fn validate(&self) -> Result<(), QueryError> {
        if self.limit == Some(0) {
            return Err(QueryError::invalid_query(
                "query limit must be greater than zero when provided",
            ));
        }

        if matches!((self.since, self.until), (Some(since), Some(until)) if since > until) {
            return Err(QueryError::invalid_query(
                "query since timestamp must be less than or equal to until",
            ));
        }

        if self
            .field_matches
            .iter()
            .any(|field_match| field_match.field.trim().is_empty())
        {
            return Err(QueryError::invalid_query(
                "query field match names must not be empty",
            ));
        }

        Ok(())
    }
}

/// Stable synchronous result contract returned by query/follow polling surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogSnapshot {
    /// Matching events returned by the query or poll.
    pub events: Vec<LogEvent>,
    /// Whether the result set was truncated by the configured limit.
    pub truncated: bool,
}

/// Stable shared error contract for historical query and follow operations.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
pub enum QueryError {
    #[error("{0}")]
    /// The query contract was invalid before execution.
    InvalidQuery(#[source] Box<ErrorContext>),
    #[error("{0}")]
    /// I/O failed while reading log data.
    Io(#[source] Box<ErrorContext>),
    #[error("{0}")]
    /// A JSONL record failed to decode.
    Decode(#[source] Box<ErrorContext>),
    #[error("{0}")]
    /// Query or follow is unavailable in the current runtime state.
    Unavailable(#[source] Box<ErrorContext>),
    #[error("query runtime shut down")]
    /// The query runtime was shut down.
    Shutdown,
}

impl QueryError {
    /// Returns the stable machine-readable error code for this variant.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidQuery(_) => error_codes::SC_LOG_QUERY_INVALID_QUERY,
            Self::Io(_) => error_codes::SC_LOG_QUERY_IO,
            Self::Decode(_) => error_codes::SC_LOG_QUERY_DECODE,
            Self::Unavailable(_) => error_codes::SC_LOG_QUERY_UNAVAILABLE,
            Self::Shutdown => error_codes::SC_LOG_QUERY_SHUTDOWN,
        }
    }

    /// Returns the attached diagnostic for the error.
    pub fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::InvalidQuery(context)
            | Self::Io(context)
            | Self::Decode(context)
            | Self::Unavailable(context) => context.diagnostic(),
            Self::Shutdown => shutdown_diagnostic(),
        }
    }

    /// Builds an invalid-query error using the stable shared code.
    pub fn invalid_query(message: impl Into<String>) -> Self {
        Self::InvalidQuery(Box::new(ErrorContext::new(
            error_codes::SC_LOG_QUERY_INVALID_QUERY,
            message,
            Remediation::recoverable("correct the query parameters", ["retry the query"]),
        )))
    }
}

impl sealed::Sealed for QueryError {}

impl DiagnosticInfo for QueryError {
    fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic()
    }
}

fn shutdown_diagnostic() -> &'static Diagnostic {
    static DIAGNOSTIC: LazyLock<Diagnostic> = LazyLock::new(|| Diagnostic {
        timestamp: Timestamp::UNIX_EPOCH,
        code: error_codes::SC_LOG_QUERY_SHUTDOWN,
        message: "query runtime shut down".to_string(),
        cause: None,
        remediation: Remediation::recoverable("restart the logger", ["retry"]),
        docs: None,
        details: serde_json::Map::new(),
    });

    &DIAGNOSTIC
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};
    use time::Duration;

    use super::*;
    use crate::{DiagnosticSummary, ProcessIdentity, TraceContext};

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn target_category() -> TargetCategory {
        TargetCategory::new("logging.query").expect("valid target category")
    }

    fn action_name() -> ActionName {
        ActionName::new("query.executed").expect("valid action name")
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: crate::TraceId::new("0123456789abcdef0123456789abcdef")
                .expect("valid trace id"),
            span_id: crate::SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: None,
        }
    }

    fn diagnostic(code: ErrorCode, message: &str) -> Diagnostic {
        Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code,
            message: message.to_string(),
            cause: Some("root cause".to_string()),
            remediation: Remediation::recoverable("fix input", ["retry"]),
            docs: Some("https://example.test/query".to_string()),
            details: Map::from_iter([("line".to_string(), json!(12))]),
        }
    }

    fn log_event() -> LogEvent {
        LogEvent {
            version: crate::SchemaVersion::new("v1").expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: target_category(),
            action: action_name(),
            message: Some("query event".to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace_context()),
            request_id: Some(CorrelationId::new("req-1").expect("valid request id")),
            correlation_id: Some(CorrelationId::new("corr-1").expect("valid correlation id")),
            outcome: Some(crate::OutcomeLabel::new("success").expect("valid outcome")),
            diagnostic: None,
            state_transition: None,
            fields: Map::from_iter([("status".to_string(), json!("ok"))]),
        }
    }

    #[test]
    fn log_query_round_trips_through_serde() {
        let query = LogQuery {
            service: Some(service_name()),
            levels: vec![Level::Info, Level::Warn],
            target: Some(target_category()),
            action: Some(action_name()),
            request_id: Some(CorrelationId::new("req-1").expect("valid request id")),
            correlation_id: Some(CorrelationId::new("corr-1").expect("valid correlation id")),
            since: Some(Timestamp::UNIX_EPOCH),
            until: Some(Timestamp::UNIX_EPOCH + Duration::minutes(5)),
            field_matches: vec![LogFieldMatch::equals("status", json!("ok"))],
            limit: Some(25),
            order: LogOrder::NewestFirst,
        };

        let encoded = serde_json::to_value(&query).expect("serialize query");
        let decoded: LogQuery = serde_json::from_value(encoded).expect("deserialize query");
        assert_eq!(decoded, query);
        decoded.validate().expect("valid query");
    }

    #[test]
    fn log_query_validation_rejects_invalid_ranges_and_limits() {
        let invalid_limit = LogQuery {
            limit: Some(0),
            ..LogQuery::default()
        };
        let invalid_range = LogQuery {
            since: Some(Timestamp::UNIX_EPOCH + Duration::hours(1)),
            until: Some(Timestamp::UNIX_EPOCH),
            ..LogQuery::default()
        };
        let invalid_field = LogQuery {
            field_matches: vec![LogFieldMatch::equals("", json!("ok"))],
            ..LogQuery::default()
        };

        assert_eq!(
            invalid_limit.validate().expect_err("invalid limit").code(),
            error_codes::SC_LOG_QUERY_INVALID_QUERY
        );
        assert_eq!(
            invalid_range.validate().expect_err("invalid range").code(),
            error_codes::SC_LOG_QUERY_INVALID_QUERY
        );
        assert_eq!(
            invalid_field
                .validate()
                .expect_err("invalid field name")
                .code(),
            error_codes::SC_LOG_QUERY_INVALID_QUERY
        );
    }

    #[test]
    fn log_snapshot_round_trips_through_serde() {
        let snapshot = LogSnapshot {
            events: vec![log_event()],
            truncated: true,
        };

        let encoded = serde_json::to_value(&snapshot).expect("serialize snapshot");
        let decoded: LogSnapshot = serde_json::from_value(encoded).expect("deserialize snapshot");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn query_error_variants_round_trip_and_match_stable_codes() {
        let variants = [
            QueryError::InvalidQuery(Box::new(ErrorContext::new(
                error_codes::SC_LOG_QUERY_INVALID_QUERY,
                "invalid query",
                Remediation::recoverable("fix the query", ["retry"]),
            ))),
            QueryError::Io(Box::new(ErrorContext::new(
                error_codes::SC_LOG_QUERY_IO,
                "i/o failure",
                Remediation::recoverable("check the log path", ["retry"]),
            ))),
            QueryError::Decode(Box::new(ErrorContext::new(
                error_codes::SC_LOG_QUERY_DECODE,
                "decode failure",
                Remediation::recoverable("repair the malformed record", ["retry"]),
            ))),
            QueryError::Unavailable(Box::new(ErrorContext::new(
                error_codes::SC_LOG_QUERY_UNAVAILABLE,
                "query unavailable",
                Remediation::recoverable("wait for logging to recover", ["retry"]),
            ))),
            QueryError::Shutdown,
        ];

        for error in variants {
            let encoded = serde_json::to_value(&error).expect("serialize error");
            let decoded: QueryError = serde_json::from_value(encoded).expect("deserialize error");
            assert_eq!(decoded, error);
            assert_eq!(decoded.diagnostic().code, decoded.code());
        }
    }

    #[test]
    fn query_shutdown_uses_static_diagnostic() {
        let error = QueryError::Shutdown;
        assert_eq!(error.code(), error_codes::SC_LOG_QUERY_SHUTDOWN);
        assert_eq!(error.diagnostic().message, "query runtime shut down");
    }

    #[test]
    fn query_health_report_round_trips_through_serde() {
        let report = crate::QueryHealthReport {
            state: crate::QueryHealthState::Degraded,
            last_error: Some(DiagnosticSummary::from(&diagnostic(
                error_codes::SC_LOG_QUERY_DECODE,
                "decode failure",
            ))),
        };

        let encoded = serde_json::to_value(&report).expect("serialize report");
        let decoded: crate::QueryHealthReport =
            serde_json::from_value(encoded).expect("deserialize report");
        assert_eq!(decoded, report);
    }
}
