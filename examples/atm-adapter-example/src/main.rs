//! Unpublished proving artifact for the ATM adapter pattern.
//!
//! This crate is intentionally minimal. It exists to prove that ATM-shaped
//! payloads can be modeled outside the shared repo while depending only on the
//! standalone observability crates.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sc_observability::RotationPolicy;
use sc_observability_otlp::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpProtocol, TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::{
    EnvPrefix, Observation, ProcessIdentity, ServiceName, ToolName, TraceContext, TraceId, SpanId,
};
use sc_observe::ObservabilityConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AtmHookObservation {
    event_type: String,
    agent_id: String,
    team: String,
}

fn main() {
    let payload = AtmHookObservation {
        event_type: "tool_use".to_string(),
        agent_id: "agent-123".to_string(),
        team: "atm-dev".to_string(),
    };

    let _observation = Observation {
        version: "v1".to_string(),
        timestamp: time::OffsetDateTime::now_utc(),
        service: ServiceName::new("atm").expect("service name"),
        identity: ProcessIdentity {
            hostname: Some("host-a".to_string()),
            pid: Some(4242),
        },
        trace: Some(TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("span id"),
            parent_span_id: None,
        }),
        payload,
    };

    let _observability_config = ObservabilityConfig {
        tool_name: ToolName::new("atm").expect("tool name"),
        log_root: std::path::PathBuf::from("/var/log/atm"),
        env_prefix: EnvPrefix::new("ATM").expect("env prefix"),
        queue_capacity: 1024,
        rotation: RotationPolicy::default(),
    };

    let _telemetry_config = TelemetryConfigBuilder::new(ServiceName::new("atm").expect("service"))
        .with_transport(OtelConfig {
            enabled: true,
            endpoint: Some("https://otel.example.internal".to_string()),
            protocol: OtlpProtocol::HttpBinary,
            ..OtelConfig::default()
        })
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_resource(sc_observability_otlp::ResourceAttributes {
            attributes: [("service.namespace".to_string(), json!("atm"))]
                .into_iter()
                .collect(),
        })
        .build();
}
