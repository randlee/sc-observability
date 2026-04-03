//! Runnable ATM adapter example built on the shared observability crates.

mod constants;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sc_observability::RotationPolicy;
use sc_observability_otlp::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpProtocol, Telemetry, TelemetryConfig,
    TelemetryConfigBuilder, TelemetryProjectors, TracesConfig,
};
use sc_observability_types::{
    ActionName, Diagnostic, DurationMs, ErrorCode, Level, LogEvent, MetricKind, MetricName,
    MetricRecord, LoggingHealthReport, Observation, ObservabilityHealthReport, ProcessIdentity,
    Remediation, ServiceName, SpanEvent, SpanId, SpanRecord, SpanSignal, SpanStarted,
    SpanStatus, StateTransition, TargetCategory, TelemetryHealthReport, TraceContext, TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentContext {
    team: String,
    agent_id: String,
    subagent_id: Option<String>,
    session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum HookEventKind {
    SubagentStart {
        agent_type: String,
        args: Vec<String>,
    },
    ToolUse {
        tool: String,
        args: Vec<String>,
        duration_ms: Option<u64>,
    },
    SubagentEnd {
        outcome: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentInfoEvent {
    context: AgentContext,
    event: HookEventKind,
}

#[derive(Debug, Clone)]
struct AtmHealthProjection {
    logging_state: String,
    routing_state: String,
    telemetry_state: String,
    dropped_logs_total: u64,
    dropped_observations_total: u64,
    dropped_exports_total: u64,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Normal,
    FailOpen,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = parse_mode();
    let service = ServiceName::new("atm").expect("valid service name");
    let log_root = temp_log_root();

    let health = build_observability(service.clone(), log_root.clone(), mode)?;

    println!("ATM adapter example complete");
    println!("mode={mode:?}");
    println!("log_root={}", log_root.display());
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "logging_state": health.logging_state,
            "routing_state": health.routing_state,
            "telemetry_state": health.telemetry_state,
            "dropped_logs_total": health.dropped_logs_total,
            "dropped_observations_total": health.dropped_observations_total,
            "dropped_exports_total": health.dropped_exports_total,
            "last_error": health.last_error,
        }))?
    );

    Ok(())
}

fn build_observability(
    service: ServiceName,
    log_root: PathBuf,
    mode: RunMode,
) -> Result<AtmHealthProjection, Box<dyn std::error::Error>> {
    let observability_config = ObservabilityConfig {
        tool_name: sc_observability_types::ToolName::new("atm").expect("valid tool name"),
        log_root,
        env_prefix: sc_observability_types::EnvPrefix::new("ATM").expect("valid prefix"),
        queue_capacity: 1024,
        rotation: RotationPolicy::default(),
    };

    let telemetry_config = telemetry_config_from_env(service.clone())?;
    let telemetry = Arc::new(Telemetry::new(telemetry_config)?);

    let runtime = Observability::builder(observability_config)
        .with_telemetry_health_provider(telemetry.clone())
        .register_projection(
            TelemetryProjectors::new(telemetry.clone())
                .with_log_projector(Arc::new(AtmLogProjector))
                .with_span_projector(Arc::new(AtmSpanProjector::default()))
                .with_metric_projector(Arc::new(AtmMetricProjector))
                .into_registration(),
        )
        .build()?;

    emit_example_sequence(&runtime, service, mode)?;
    runtime.flush()?;
    telemetry.flush()?;

    match mode {
        RunMode::Normal => {
            telemetry.shutdown()?;
            runtime.shutdown()?;
        }
        RunMode::FailOpen => {
            // OTLP-009: this path intentionally leaves one started span without a
            // matching end so shutdown drops it and records fail-open export loss.
            let _ = telemetry.shutdown();
            runtime.shutdown()?;
        }
    }

    let observability_health = runtime.health();
    let logging_health = observability_health
        .logging
        .clone()
        .ok_or("observability health missing logging snapshot")?;
    let telemetry_health = observability_health
        .telemetry
        .clone()
        .ok_or("observability health missing telemetry snapshot")?;

    Ok(project_health(
        &logging_health,
        &observability_health,
        &telemetry_health,
    ))
}

fn emit_example_sequence(
    runtime: &Observability,
    service: ServiceName,
    mode: RunMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = AgentContext {
        team: "atm-dev".to_string(),
        agent_id: "agent-123".to_string(),
        subagent_id: Some("subagent-7".to_string()),
        session_id: "session-42".to_string(),
    };
    let trace = TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef")?,
        span_id: SpanId::new("0123456789abcdef")?,
        parent_span_id: None,
    };

    let mut start = Observation::new(
        service.clone(),
        AgentInfoEvent {
            context: base.clone(),
            event: HookEventKind::SubagentStart {
                agent_type: "md-file".to_string(),
                args: vec!["docs/agent.md".to_string()],
            },
        },
    );
    start.trace = Some(trace.clone());
    start.identity = ProcessIdentity {
        hostname: Some("localhost".to_string()),
        pid: Some(std::process::id()),
    };
    runtime.emit(start)?;

    let mut tool = Observation::new(
        service.clone(),
        AgentInfoEvent {
            context: base.clone(),
            event: HookEventKind::ToolUse {
                tool: "read_file".to_string(),
                args: vec!["docs/agent.md".to_string()],
                duration_ms: Some(24),
            },
        },
    );
    tool.trace = Some(trace.clone());
    tool.identity = ProcessIdentity {
        hostname: Some("localhost".to_string()),
        pid: Some(std::process::id()),
    };
    runtime.emit(tool)?;

    if mode == RunMode::Normal {
        let mut end = Observation::new(
            service,
            AgentInfoEvent {
                context: base,
                event: HookEventKind::SubagentEnd {
                    outcome: "success".to_string(),
                },
            },
        );
        end.trace = Some(trace);
        end.identity = ProcessIdentity {
            hostname: Some("localhost".to_string()),
            pid: Some(std::process::id()),
        };
        runtime.emit(end)?;
    }

    Ok(())
}

fn telemetry_config_from_env(
    service: ServiceName,
) -> Result<TelemetryConfig, Box<dyn std::error::Error>> {
    let endpoint = env::var("ATM_OTEL_ENDPOINT").ok();
    let protocol = parse_protocol(env::var("ATM_OTEL_PROTOCOL").ok().as_deref())?;
    let auth_header = env::var("ATM_OTEL_AUTH_HEADER").ok();
    let ca_file = env::var("ATM_OTEL_CA_FILE").ok().map(PathBuf::from);
    let insecure_skip_verify = parse_bool_env("ATM_OTEL_INSECURE_SKIP_VERIFY")?.unwrap_or(false);
    let debug_local_export = parse_bool_env("ATM_OTEL_DEBUG_LOCAL_EXPORT")?.unwrap_or(false);

    Ok(TelemetryConfigBuilder::new(service)
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(OtelConfig {
            enabled: endpoint.is_some(),
            endpoint,
            protocol,
            auth_header,
            ca_file,
            insecure_skip_verify,
            timeout_ms: constants::OTLP_TIMEOUT_MS.into(),
            debug_local_export,
            max_retries: constants::OTLP_MAX_RETRIES,
            initial_backoff_ms: constants::OTLP_INITIAL_BACKOFF_MS.into(),
            max_backoff_ms: constants::OTLP_MAX_BACKOFF_MS.into(),
        })
        .with_resource(sc_observability_otlp::ResourceAttributes {
            attributes: [
                ("service.namespace".to_string(), json!("atm")),
                ("service.name".to_string(), json!("atm")),
            ]
            .into_iter()
            .collect(),
        })
        .build())
}

fn parse_protocol(value: Option<&str>) -> Result<OtlpProtocol, Box<dyn std::error::Error>> {
    match value.unwrap_or("http-binary") {
        "http-binary" | "otlp_http" | "http/protobuf" => Ok(OtlpProtocol::HttpBinary),
        "http-json" | "otlp_http_json" | "http/json" => Ok(OtlpProtocol::HttpJson),
        "grpc" | "otlp_grpc" => Ok(OtlpProtocol::Grpc),
        other => Err(format!("unsupported ATM_OTEL_PROTOCOL value: {other}").into()),
    }
}

fn parse_bool_env(name: &str) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => match value.as_str() {
            "1" | "true" | "TRUE" | "yes" | "YES" => Ok(Some(true)),
            "0" | "false" | "FALSE" | "no" | "NO" => Ok(Some(false)),
            _ => Err(format!("unsupported boolean value for {name}: {value}").into()),
        },
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(Box::new(err)),
    }
}

fn project_health(
    logging: &LoggingHealthReport,
    observability: &ObservabilityHealthReport,
    telemetry: &TelemetryHealthReport,
) -> AtmHealthProjection {
    AtmHealthProjection {
        logging_state: format!("{:?}", logging.state),
        routing_state: format!("{:?}", observability.state),
        telemetry_state: format!("{:?}", telemetry.state),
        dropped_logs_total: logging.dropped_events_total,
        dropped_observations_total: observability.dropped_observations_total,
        dropped_exports_total: telemetry.dropped_exports_total,
        last_error: observability
            .last_error
            .as_ref()
            .map(|summary| summary.message.clone())
            .or_else(|| telemetry.last_error.as_ref().map(|summary| summary.message.clone())),
    }
}

fn parse_mode() -> RunMode {
    match env::args().nth(1).as_deref() {
        Some("fail-open") => RunMode::FailOpen,
        _ => RunMode::Normal,
    }
}

fn temp_log_root() -> PathBuf {
    env::temp_dir().join(format!(
        "atm-adapter-example-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("time before unix epoch")
            .as_nanos()
    ))
}

struct AtmLogProjector;

impl sc_observability_types::LogProjector<AgentInfoEvent> for AtmLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentInfoEvent>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::ProjectionError> {
        Ok(vec![LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: observation.timestamp,
            level: Level::Info,
            service: observation.service.clone(),
            target: TargetCategory::new("atm.agent").expect("valid target"),
            action: ActionName::new(action_name(&observation.payload.event)).expect("valid action"),
            message: Some(log_message(&observation.payload.event)),
            identity: observation.identity.clone(),
            trace: observation.trace.clone(),
            request_id: None,
            correlation_id: Some(observation.payload.context.session_id.clone()),
            outcome: outcome(&observation.payload.event),
            diagnostic: None,
            state_transition: state_transition(&observation.payload.event),
            fields: common_fields(&observation.payload),
        }])
    }
}

#[derive(Default)]
struct AtmSpanProjector {
    started: Mutex<HashMap<String, sc_observability_types::Timestamp>>,
}

impl sc_observability_types::SpanProjector<AgentInfoEvent> for AtmSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<AgentInfoEvent>,
    ) -> Result<Vec<SpanSignal>, sc_observability_types::ProjectionError> {
        let Some(trace) = &observation.trace else {
            return Ok(Vec::new());
        };

        let signals = match &observation.payload.event {
            HookEventKind::SubagentStart { .. } => {
                self.started
                    .lock()
                    .expect("started span map poisoned")
                    .insert(trace_key(trace), observation.timestamp);
                vec![SpanSignal::Started(SpanRecord::<SpanStarted>::new(
                    observation.timestamp,
                    observation.service.clone(),
                    ActionName::new("subagent.run").expect("valid action"),
                    trace.clone(),
                    common_fields(&observation.payload),
                ))]
            }
            HookEventKind::ToolUse { tool, duration_ms, .. } => vec![SpanSignal::Event(SpanEvent {
                timestamp: observation.timestamp,
                trace: trace.clone(),
                name: ActionName::new("tool.use").expect("valid action"),
                attributes: Map::from_iter([
                    ("tool".to_string(), Value::from(tool.clone())),
                    (
                        "duration_ms".to_string(),
                        Value::from(duration_ms.unwrap_or_default()),
                    ),
                ]),
                diagnostic: None,
            })],
            HookEventKind::SubagentEnd { outcome } => {
                let start_timestamp = self
                    .started
                    .lock()
                    .expect("started span map poisoned")
                    .remove(&trace_key(trace))
                    .ok_or_else(|| {
                        sc_observability_types::ProjectionError(Box::new(
                            sc_observability_types::ErrorContext::new(
                                ErrorCode::new_static("SC_ATM_ADAPTER_EXAMPLE_MISSING_START"),
                                "subagent end arrived without a matching recorded start",
                                Remediation::recoverable(
                                    "emit subagent.start before subagent.end",
                                    ["preserve the trace context across the span lifecycle"],
                                ),
                            ),
                        ))
                    })?;
                let duration_ms = (observation.timestamp - start_timestamp)
                    .whole_milliseconds()
                    .max(0) as u64;
                vec![SpanSignal::Ended(
                    SpanRecord::<SpanStarted>::new(
                        start_timestamp,
                        observation.service.clone(),
                        ActionName::new("subagent.run").expect("valid action"),
                        trace.clone(),
                        common_fields(&observation.payload),
                    )
                    .with_diagnostic(Diagnostic {
                        timestamp: observation.timestamp,
                        code: ErrorCode::new_static("SC_ATM_ADAPTER_EXAMPLE_OUTCOME"),
                        message: format!("subagent completed with outcome={outcome}"),
                        cause: None,
                        remediation: Remediation::recoverable(
                            "inspect ATM output",
                            ["review structured logs"],
                        ),
                        docs: None,
                        details: Map::new(),
                    })
                    .end(SpanStatus::Ok, DurationMs::from(duration_ms)),
                )]
            }
        };

        Ok(signals)
    }
}

struct AtmMetricProjector;

impl sc_observability_types::MetricProjector<AgentInfoEvent> for AtmMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentInfoEvent>,
    ) -> Result<Vec<MetricRecord>, sc_observability_types::ProjectionError> {
        let mut metrics = Vec::new();

        metrics.push(MetricRecord {
            timestamp: observation.timestamp,
            service: observation.service.clone(),
            name: MetricName::new("atm.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some("1".to_string()),
            attributes: common_fields(&observation.payload),
        });

        if let HookEventKind::ToolUse { duration_ms, .. } = &observation.payload.event {
            metrics.push(MetricRecord {
                timestamp: observation.timestamp,
                service: observation.service.clone(),
                name: MetricName::new("atm.tool_use_duration_ms").expect("valid metric"),
                kind: MetricKind::Histogram,
                value: duration_ms.unwrap_or_default() as f64,
                unit: Some("ms".to_string()),
                attributes: common_fields(&observation.payload),
            });
        }

        Ok(metrics)
    }
}

fn action_name(event: &HookEventKind) -> &'static str {
    match event {
        HookEventKind::SubagentStart { .. } => "subagent.start",
        HookEventKind::ToolUse { .. } => "tool.use",
        HookEventKind::SubagentEnd { .. } => "subagent.end",
    }
}

fn log_message(event: &HookEventKind) -> String {
    match event {
        HookEventKind::SubagentStart { agent_type, .. } => {
            format!("subagent started ({agent_type})")
        }
        HookEventKind::ToolUse { tool, .. } => format!("tool used ({tool})"),
        HookEventKind::SubagentEnd { outcome } => format!("subagent ended ({outcome})"),
    }
}

fn outcome(event: &HookEventKind) -> Option<String> {
    match event {
        HookEventKind::SubagentEnd { outcome } => Some(outcome.clone()),
        _ => None,
    }
}

fn state_transition(event: &HookEventKind) -> Option<StateTransition> {
    match event {
        HookEventKind::SubagentStart { .. } => Some(StateTransition {
            entity_kind: "subagent".to_string(),
            entity_id: Some("subagent-7".to_string()),
            from_state: "idle".to_string(),
            to_state: "running".to_string(),
            reason: None,
            trigger: Some("subagent.start".to_string()),
        }),
        HookEventKind::SubagentEnd { .. } => Some(StateTransition {
            entity_kind: "subagent".to_string(),
            entity_id: Some("subagent-7".to_string()),
            from_state: "running".to_string(),
            to_state: "completed".to_string(),
            reason: None,
            trigger: Some("subagent.end".to_string()),
        }),
        HookEventKind::ToolUse { .. } => None,
    }
}

fn common_fields(payload: &AgentInfoEvent) -> Map<String, Value> {
    Map::from_iter([
        ("team".to_string(), Value::from(payload.context.team.clone())),
        ("agent_id".to_string(), Value::from(payload.context.agent_id.clone())),
        (
            "subagent_id".to_string(),
            payload
                .context
                .subagent_id
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        (
            "session_id".to_string(),
            Value::from(payload.context.session_id.clone()),
        ),
    ])
}

fn trace_key(trace: &TraceContext) -> String {
    format!("{}:{}", trace.trace_id.as_str(), trace.span_id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn service_name() -> ServiceName {
        ServiceName::new("atm").expect("valid service")
    }

    fn base_trace() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span"),
            parent_span_id: None,
        }
    }

    fn base_context() -> AgentContext {
        AgentContext {
            team: "atm-dev".to_string(),
            agent_id: "agent-123".to_string(),
            subagent_id: Some("subagent-7".to_string()),
            session_id: "session-42".to_string(),
        }
    }

    #[test]
    fn subagent_end_uses_recorded_start_timestamp_and_real_duration() {
        let projector = AtmSpanProjector::default();
        let mut start = Observation::new(
            service_name(),
            AgentInfoEvent {
                context: base_context(),
                event: HookEventKind::SubagentStart {
                    agent_type: "md-file".to_string(),
                    args: vec!["docs/agent.md".to_string()],
                },
            },
        );
        start.timestamp = Timestamp::UNIX_EPOCH;
        start.trace = Some(base_trace());

        let mut end = Observation::new(
            service_name(),
            AgentInfoEvent {
                context: base_context(),
                event: HookEventKind::SubagentEnd {
                    outcome: "success".to_string(),
                },
            },
        );
        end.timestamp = Timestamp::UNIX_EPOCH + Duration::milliseconds(250);
        end.trace = Some(base_trace());

        let start_signals = projector.project_spans(&start).expect("start");
        let end_signals = projector.project_spans(&end).expect("end");

        assert!(matches!(&start_signals[0], SpanSignal::Started(_)));
        match &end_signals[0] {
            SpanSignal::Ended(span) => {
                assert_eq!(span.timestamp(), Timestamp::UNIX_EPOCH);
                assert_eq!(span.duration_ms(), DurationMs::from(250));
            }
            other => panic!("expected ended span, got {other:?}"),
        }
    }
}
