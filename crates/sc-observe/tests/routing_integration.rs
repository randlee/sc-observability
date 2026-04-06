use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricKind, MetricName, MetricUnit,
    Observation, ObservationSubscriber, OutcomeLabel, ProcessIdentity, ProjectionRegistration,
    Remediation, SchemaVersion, ServiceName, SpanId, SpanProjector, SpanRecord, SpanSignal,
    SpanStarted, SubscriberRegistration, TargetCategory, Timestamp, TraceContext, TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};
use serde_json::Map;

#[derive(Debug, Clone)]
struct AgentEvent {
    kind: &'static str,
}

struct RecordingSubscriber {
    id: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl ObservationSubscriber<AgentEvent> for RecordingSubscriber {
    fn observe(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<(), sc_observability_types::SubscriberError> {
        self.calls.lock().expect("calls poisoned").push(self.id);
        Ok(())
    }
}

struct RecordingLogProjector {
    calls: Arc<Mutex<Vec<&'static str>>>,
    id: &'static str,
}

impl sc_observability_types::LogProjector<AgentEvent> for RecordingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::ProjectionError> {
        self.calls.lock().expect("calls poisoned").push(self.id);
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
            message: Some(observation.payload.kind.to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace_context()),
            request_id: None,
            correlation_id: None,
            outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
            diagnostic: Some(Diagnostic {
                timestamp: Timestamp::UNIX_EPOCH,
                code: ErrorCode::new_static("SC_TEST"),
                message: "projected".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect log output"]),
                docs: None,
                details: Map::default(),
            }),
            state_transition: None,
            fields: Map::default(),
        }])
    }
}

struct RecordingSpanProjector {
    count: Arc<AtomicU64>,
}

impl SpanProjector<AgentEvent> for RecordingSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<SpanSignal>, sc_observability_types::ProjectionError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            observation.service.clone(),
            ActionName::new("span.started").expect("valid action"),
            trace_context(),
            Map::default(),
        ))])
    }
}

struct RecordingMetricProjector {
    count: Arc<AtomicU64>,
}

impl sc_observability_types::MetricProjector<AgentEvent> for RecordingMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<sc_observability_types::MetricRecord>, sc_observability_types::ProjectionError>
    {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![sc_observability_types::MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: observation.service.clone(),
            name: MetricName::new("obs.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some(MetricUnit::new("1").expect("valid metric unit")),
            attributes: Map::default(),
        }])
    }
}

fn tool_name() -> sc_observability_types::ToolName {
    sc_observability_types::ToolName::new("obs-app").expect("valid tool name")
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    }
}

fn observation() -> Observation<AgentEvent> {
    let mut observation = Observation::new(
        ServiceName::new("obs-app").expect("valid service"),
        AgentEvent { kind: "received" },
    );
    observation.identity = ProcessIdentity::default();
    observation
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sc-observe-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ))
}

#[test]
fn one_observation_can_fan_out_to_subscribers_logs_spans_and_metrics() {
    let subscriber_calls = Arc::new(Mutex::new(Vec::new()));
    let log_calls = Arc::new(Mutex::new(Vec::new()));
    let span_count = Arc::new(AtomicU64::new(0));
    let metric_count = Arc::new(AtomicU64::new(0));
    let root = temp_path("fanout");
    let config = ObservabilityConfig::default_for(tool_name(), root.clone()).expect("config");
    let runtime = Observability::builder(config)
        .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
            id: "subscriber",
            calls: subscriber_calls.clone(),
        })))
        .register_projection(
            ProjectionRegistration::new()
                .with_log_projector(Arc::new(RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                }))
                .with_span_projector(Arc::new(RecordingSpanProjector {
                    count: span_count.clone(),
                }))
                .with_metric_projector(Arc::new(RecordingMetricProjector {
                    count: metric_count.clone(),
                })),
        )
        .build()
        .expect("runtime");

    runtime.emit(observation()).expect("emit");

    let log_path = root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "obs-app{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let contents = std::fs::read_to_string(log_path).expect("read projected log file");

    assert_eq!(
        *subscriber_calls.lock().expect("subscriber calls poisoned"),
        vec!["subscriber"]
    );
    assert_eq!(*log_calls.lock().expect("log calls poisoned"), vec!["log"]);
    assert_eq!(span_count.load(Ordering::SeqCst), 1);
    assert_eq!(metric_count.load(Ordering::SeqCst), 1);
    assert!(contents.contains("\"action\":\"observation.received\""));
}
