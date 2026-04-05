use std::sync::Arc;

use sc_observability_otlp::{
    LogsConfig, MetricsConfig, OtelConfig, Telemetry, TelemetryConfigBuilder, TelemetryProjectors,
    TracesConfig,
};
use sc_observability_types::{
    ActionName, Diagnostic, DurationMs, ErrorCode, Level, LogEvent, MetricKind, MetricName,
    MetricRecord, Observation, ObservationFilter, ProcessIdentity, ProjectionError, Remediation,
    ServiceName, SpanEvent, SpanId, SpanProjector, SpanRecord, SpanSignal, SpanStarted,
    StateTransition, TargetCategory, TelemetryHealthState, Timestamp, ToolName, TraceContext,
    TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};

#[derive(Debug, Clone)]
struct AgentPayload {
    action: &'static str,
    emit: bool,
}

struct AllowAll;

impl ObservationFilter<AgentPayload> for AllowAll {
    fn accepts(&self, observation: &Observation<AgentPayload>) -> bool {
        observation.payload.emit
    }
}

struct StaticLogProjector;
struct StaticSpanProjector;
struct StaticMetricProjector;

impl sc_observability_types::LogProjector<AgentPayload> for StaticLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        Ok(vec![log_event(
            observation.service.clone(),
            observation.payload.action,
        )])
    }
}

impl SpanProjector<AgentPayload> for StaticSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        let trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            observation.service.clone(),
            ActionName::new("agent.run").expect("valid action"),
            trace.clone(),
            Default::default(),
        );
        let ended = started
            .clone()
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(10));
        Ok(vec![
            SpanSignal::Started(started),
            SpanSignal::Event(SpanEvent {
                timestamp: Timestamp::UNIX_EPOCH,
                trace: trace.clone(),
                name: ActionName::new("tool.call").expect("valid event name"),
                attributes: Default::default(),
                diagnostic: None,
            }),
            SpanSignal::Ended(ended),
        ])
    }
}

impl sc_observability_types::MetricProjector<AgentPayload> for StaticMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        Ok(vec![MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: observation.service.clone(),
            name: MetricName::new("agent.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some("1".to_string()),
            attributes: Default::default(),
        }])
    }
}

fn telemetry_config() -> sc_observability_otlp::TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(OtelConfig {
            enabled: true,
            endpoint: Some("https://otel.example.internal".to_string()),
            ..OtelConfig::default()
        })
        .build()
}

fn service_name() -> ServiceName {
    ServiceName::new("test-service").expect("valid service")
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    }
}

fn log_event(service: ServiceName, message: &str) -> LogEvent {
    LogEvent {
        version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service,
        target: TargetCategory::new("test.agent").expect("valid target"),
        action: ActionName::new("agent.observe").expect("valid action"),
        message: Some(message.to_string()),
        identity: ProcessIdentity::default(),
        trace: Some(trace_context()),
        request_id: None,
        correlation_id: None,
        outcome: Some("ok".to_string()),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST"),
            message: "projected".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect telemetry"]),
            docs: None,
            details: Default::default(),
        }),
        state_transition: Some(StateTransition {
            entity_kind: "agent".to_string(),
            entity_id: Some("agent-123".to_string()),
            from_state: "idle".to_string(),
            to_state: "running".to_string(),
            reason: None,
            trigger: None,
        }),
        fields: Default::default(),
    }
}

fn observation() -> Observation<AgentPayload> {
    Observation::new(
        service_name(),
        AgentPayload {
            action: "tool_use",
            emit: true,
        },
    )
}

fn temp_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "s4-attach-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

#[test]
fn builder_registration_attaches_logs_spans_and_metrics() {
    let telemetry = Arc::new(Telemetry::new(telemetry_config()).expect("telemetry"));
    let root = temp_root("integration");
    let config = ObservabilityConfig::default_for(
        ToolName::new("test-service").expect("valid tool"),
        root.clone(),
    )
    .expect("config");
    let runtime = Observability::builder(config)
        .with_observability_health_provider(telemetry.clone())
        .register_projection(
            TelemetryProjectors::new(telemetry.clone())
                .with_log_projector(Arc::new(StaticLogProjector))
                .with_span_projector(Arc::new(StaticSpanProjector))
                .with_metric_projector(Arc::new(StaticMetricProjector))
                .with_filter(Arc::new(AllowAll))
                .into_registration(),
        )
        .build()
        .expect("runtime");

    runtime.emit(observation()).expect("emit");
    telemetry.flush().expect("flush");

    let log_path = root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "test-service{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let contents = std::fs::read_to_string(log_path).expect("read projected log file");
    let health = telemetry.health();
    let runtime_health = runtime.health();

    assert!(contents.contains("\"action\":\"agent.observe\""));
    assert_eq!(health.state, TelemetryHealthState::Healthy);
    assert_eq!(health.dropped_exports_total, 0);
    assert_eq!(
        runtime_health
            .telemetry
            .expect("attached telemetry health")
            .state,
        TelemetryHealthState::Healthy
    );
    assert!(
        health
            .exporter_statuses
            .iter()
            .all(|status| status.state == sc_observability_types::ExporterHealthState::Healthy)
    );
}
