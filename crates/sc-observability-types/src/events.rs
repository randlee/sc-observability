use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    ActionName, Diagnostic, Level, ProcessIdentity, ServiceName, StateTransition, TargetCategory,
    Timestamp, TraceContext, constants,
};

/// Marker trait for consumer-owned observation payloads.
pub trait Observable: Send + Sync + 'static {}

impl<T> Observable for T where T: Send + Sync + 'static {}

/// Shared envelope around a typed observation payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation<T>
where
    T: Observable,
{
    /// Envelope schema version.
    pub version: String,
    /// UTC observation timestamp.
    pub timestamp: Timestamp,
    /// Service that emitted the observation.
    pub service: ServiceName,
    /// Process identity attached to the observation.
    pub identity: ProcessIdentity,
    /// Optional trace context for correlation.
    pub trace: Option<TraceContext>,
    /// Caller-owned typed payload.
    pub payload: T,
}

impl<T> Observation<T>
where
    T: Observable,
{
    /// Creates a new observation envelope using the current UTC timestamp.
    pub fn new(service: ServiceName, payload: T) -> Self {
        Self {
            version: constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::now_utc(),
            service,
            identity: ProcessIdentity::default(),
            trace: None,
            payload,
        }
    }
}

/// Structured log record emitted by the logging and routing layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    /// Log schema version.
    pub version: String,
    /// UTC event timestamp.
    pub timestamp: Timestamp,
    /// Event severity.
    pub level: Level,
    /// Service that emitted the event.
    pub service: ServiceName,
    /// Stable target/category label.
    pub target: TargetCategory,
    /// Stable action label.
    pub action: ActionName,
    /// Optional human-readable message.
    pub message: Option<String>,
    /// Process identity attached to the event.
    pub identity: ProcessIdentity,
    /// Optional trace context for correlation.
    pub trace: Option<TraceContext>,
    /// Optional request identifier.
    pub request_id: Option<String>,
    /// Optional correlation identifier.
    pub correlation_id: Option<String>,
    /// Optional stable outcome label.
    pub outcome: Option<String>,
    /// Optional structured diagnostic payload.
    pub diagnostic: Option<Diagnostic>,
    /// Optional state transition payload.
    pub state_transition: Option<StateTransition>,
    /// Arbitrary structured event fields.
    pub fields: Map<String, Value>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::{SpanId, TraceId};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct FixturePayload {
        name: String,
        count: u32,
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn target_category() -> TargetCategory {
        TargetCategory::new("routing.core").expect("valid target category")
    }

    fn action_name() -> ActionName {
        ActionName::new("observation.received").expect("valid action name")
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: Some(SpanId::new("fedcba9876543210").expect("valid parent span id")),
        }
    }

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: crate::error_codes::DIAGNOSTIC_INVALID,
            message: "diagnostic invalid".to_string(),
            cause: Some("invalid example".to_string()),
            remediation: crate::Remediation::recoverable(
                "fix the input",
                ["rerun the command", "review the docs"],
            ),
            docs: Some("https://example.test/docs".to_string()),
            details: Map::from_iter([("key".to_string(), json!("value"))]),
        }
    }

    #[test]
    fn observation_round_trips_through_serde() {
        let mut observation = Observation::new(
            service_name(),
            FixturePayload {
                name: "agent-info".to_string(),
                count: 2,
            },
        );
        observation.identity = ProcessIdentity {
            hostname: Some("host-1".to_string()),
            pid: Some(42),
        };
        observation.trace = Some(trace_context());

        let encoded = serde_json::to_string(&observation).expect("serialize observation");
        let decoded: Observation<FixturePayload> =
            serde_json::from_str(&encoded).expect("deserialize observation");
        assert_eq!(decoded, observation);
    }

    #[test]
    fn log_event_round_trips_through_serde() {
        let event = LogEvent {
            version: constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: target_category(),
            action: action_name(),
            message: Some("observation accepted".to_string()),
            identity: ProcessIdentity {
                hostname: Some("host-1".to_string()),
                pid: Some(7),
            },
            trace: Some(trace_context()),
            request_id: Some("req-1".to_string()),
            correlation_id: Some("corr-1".to_string()),
            outcome: Some("success".to_string()),
            diagnostic: Some(diagnostic()),
            state_transition: Some(StateTransition {
                entity_kind: "subagent".to_string(),
                entity_id: Some("agent-1".to_string()),
                from_state: "started".to_string(),
                to_state: "running".to_string(),
                reason: Some("hook received".to_string()),
                trigger: Some("subagent-start".to_string()),
            }),
            fields: Map::from_iter([("attempt".to_string(), json!(1))]),
        };

        let encoded = serde_json::to_string(&event).expect("serialize log event");
        let decoded: LogEvent = serde_json::from_str(&encoded).expect("deserialize log event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn observation_new_sets_defaults() {
        let observation = Observation::new(
            service_name(),
            FixturePayload {
                name: "payload".to_string(),
                count: 1,
            },
        );

        assert_eq!(observation.version, constants::OBSERVATION_ENVELOPE_VERSION);
        assert_eq!(observation.identity, ProcessIdentity::default());
        assert!(observation.trace.is_none());
    }
}
