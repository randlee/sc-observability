#![allow(
    deprecated,
    reason = "OTLP integration compatibility fixtures exercise retained projector boundaries"
)]

use std::net::TcpListener;
use std::sync::Arc;
#[cfg(feature = "otlp-sdk")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "legacy-http-json")]
use std::time::{Duration, Instant};

#[cfg(feature = "legacy-http-json")]
use std::io::{ErrorKind, Read, Write};

#[cfg(feature = "legacy-http-json")]
use sc_observability_otlp::AuthHeader;
#[cfg(feature = "legacy-http-json")]
use sc_observability_otlp::OtlpProtocol;
#[cfg(feature = "otlp-sdk")]
use sc_observability_otlp::v2::AuthHeader as V2AuthHeader;
use sc_observability_otlp::v2::{
    OtelConfig as V2OtelConfig, OtlpEndpoint as V2OtlpEndpoint, Telemetry as V2Telemetry,
    TelemetryConfig as V2TelemetryConfig, TelemetryConfigBuilder as V2TelemetryConfigBuilder,
};
use sc_observability_otlp::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, Telemetry, TelemetryConfigBuilder,
    TelemetryProjectors, TracesConfig,
};
use sc_observability_types::ProjectionError;
use sc_observability_types::typed::{
    ProjectionFailure, TypedLogProjector, TypedMetricProjector, TypedSpanProjector,
    legacy_log_projector, legacy_metric_projector, legacy_span_projector,
};
#[cfg(feature = "otlp-sdk")]
use sc_observability_types::v2::{
    AggregationTemporality as CanonicalAggregationTemporality,
    AttributeValue as CanonicalAttributeValue, Attributes as CanonicalAttributes, FiniteF64,
    HistogramPoint, MetricRecord as CanonicalMetricRecord, MetricValue,
    SpanEvent as CanonicalSpanEvent, SpanKind as CanonicalSpanKind, SpanLink,
    SpanRecord as CanonicalSpanRecord, SpanSignal as CanonicalSpanSignal,
    SpanStarted as CanonicalSpanStarted, SpanStatus as CanonicalSpanStatus,
    TraceContext as CanonicalTraceContext, TraceFlags,
};
use sc_observability_types::{
    ActionName, Diagnostic, DiagnosticInfo, DurationMs, ErrorCode, Level, LogEvent, LogProjector,
    MetricKind, MetricName, MetricProjector, MetricRecord, MetricUnit, Observation,
    ObservationFilter, OutcomeLabel, ProcessIdentity, Remediation, SchemaVersion, ServiceName,
    SpanEvent, SpanId, SpanProjector, SpanRecord, SpanSignal, SpanStarted, StateTransition,
    TargetCategory, TelemetryHealthState, Timestamp, ToolName, TraceContext, TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};
#[cfg(feature = "legacy-http-json")]
use serde_json::Value;
use serde_json::{Map, json};

#[cfg(feature = "otlp-sdk")]
use opentelemetry_proto::tonic::collector::{
    logs::v1::{
        ExportLogsServiceRequest, ExportLogsServiceResponse,
        logs_service_server::{LogsService, LogsServiceServer},
    },
    metrics::v1::{
        ExportMetricsServiceRequest, ExportMetricsServiceResponse,
        metrics_service_server::{MetricsService, MetricsServiceServer},
    },
    trace::v1::{
        ExportTraceServiceRequest, ExportTraceServiceResponse,
        trace_service_server::{TraceService, TraceServiceServer},
    },
};

#[cfg(feature = "otlp-sdk")]
struct CapturingLogsService {
    sender: tokio::sync::mpsc::Sender<ExportLogsServiceRequest>,
}

#[cfg(feature = "otlp-sdk")]
fn assert_sdk_default_resource_and_scope(
    resource: &opentelemetry_proto::tonic::resource::v1::Resource,
    scope: &opentelemetry_proto::tonic::common::v1::InstrumentationScope,
    resource_schema_url: &str,
    scope_schema_url: &str,
) {
    assert_eq!(
        resource.attributes.len(),
        1,
        "default resource is not widened"
    );
    assert_eq!(resource.attributes[0].key, "service.name");
    assert!(matches!(
        resource.attributes[0]
            .value
            .as_ref()
            .and_then(|value| value.value.as_ref()),
        Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(value))
            if value == service_name().as_str()
    ));
    assert_eq!(
        resource_schema_url, "",
        "default resource has no schema URL"
    );
    assert_eq!(scope.name, "", "default instrumentation scope is retained");
    assert_eq!(scope.version, "", "default scope has no version");
    assert!(
        scope.attributes.is_empty(),
        "default scope has no attributes"
    );
    assert_eq!(scope_schema_url, "", "default scope has no schema URL");
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl LogsService for CapturingLogsService {
    async fn export(
        &self,
        request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        self.sender
            .send(request.into_inner())
            .await
            .map_err(|_| tonic::Status::unavailable("collector receiver closed"))?;
        Ok(tonic::Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

#[cfg(feature = "otlp-sdk")]
struct CapturingTraceService {
    sender: tokio::sync::mpsc::Sender<ExportTraceServiceRequest>,
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl TraceService for CapturingTraceService {
    async fn export(
        &self,
        request: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        self.sender
            .send(request.into_inner())
            .await
            .map_err(|_| tonic::Status::unavailable("collector receiver closed"))?;
        Ok(tonic::Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

#[cfg(feature = "otlp-sdk")]
struct CapturingMetricsService {
    sender: tokio::sync::mpsc::Sender<ExportMetricsServiceRequest>,
}

#[cfg(feature = "otlp-sdk")]
struct RejectingLogsService {
    sender: tokio::sync::mpsc::Sender<tonic::metadata::MetadataMap>,
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl LogsService for RejectingLogsService {
    async fn export(
        &self,
        request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        self.sender
            .send(request.metadata().clone())
            .await
            .map_err(|_| tonic::Status::unavailable("collector receiver closed"))?;
        Err(tonic::Status::unauthenticated(
            "collector rejected credential",
        ))
    }
}

#[cfg(feature = "otlp-sdk")]
struct UnavailableLogsService {
    attempts: tokio::sync::mpsc::Sender<()>,
}

#[cfg(feature = "otlp-sdk")]
struct RecoveringLogsService {
    attempts: Arc<AtomicUsize>,
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl LogsService for RecoveringLogsService {
    async fn export(
        &self,
        _request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(tonic::Status::internal("collector rejects the first log"))
        } else {
            Ok(tonic::Response::new(ExportLogsServiceResponse {
                partial_success: None,
            }))
        }
    }
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl LogsService for UnavailableLogsService {
    async fn export(
        &self,
        _request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        self.attempts
            .send(())
            .await
            .map_err(|_| tonic::Status::unavailable("collector receiver closed"))?;
        Err(tonic::Status::unavailable(
            "collector is temporarily unavailable",
        ))
    }
}

#[cfg(feature = "otlp-sdk")]
#[tonic::async_trait]
impl MetricsService for CapturingMetricsService {
    async fn export(
        &self,
        request: tonic::Request<ExportMetricsServiceRequest>,
    ) -> Result<tonic::Response<ExportMetricsServiceResponse>, tonic::Status> {
        self.sender
            .send(request.into_inner())
            .await
            .map_err(|_| tonic::Status::unavailable("collector receiver closed"))?;
        Ok(tonic::Response::new(ExportMetricsServiceResponse {
            partial_success: None,
        }))
    }
}

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
struct TypedStaticLogProjector;
struct TypedStaticSpanProjector;
struct TypedStaticMetricProjector;

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
            Map::default(),
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
                attributes: Map::default(),
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
            unit: Some(MetricUnit::new("1").expect("valid metric unit")),
            attributes: Map::default(),
        }])
    }
}

impl TypedLogProjector<AgentPayload> for TypedStaticLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::typed::ProjectionFailure> {
        StaticLogProjector
            .project_logs(observation)
            .map_err(|error| ProjectionFailure::from_context(error.0))
    }
}

impl TypedSpanProjector<AgentPayload> for TypedStaticSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<SpanSignal>, sc_observability_types::typed::ProjectionFailure> {
        StaticSpanProjector
            .project_spans(observation)
            .map_err(|error| ProjectionFailure::from_context(error.0))
    }
}

impl TypedMetricProjector<AgentPayload> for TypedStaticMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentPayload>,
    ) -> Result<Vec<MetricRecord>, sc_observability_types::typed::ProjectionFailure> {
        StaticMetricProjector
            .project_metrics(observation)
            .map_err(|error| ProjectionFailure::from_context(error.0))
    }
}

fn disabled_telemetry_config() -> sc_observability_otlp::TelemetryConfig {
    // Local projection fixture; exporter routing uses the private injection seam.
    let transport = OtelConfig {
        enabled: false,
        ..OtelConfig::default()
    };
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build()
        .expect("valid telemetry config")
}

fn disabled_telemetry_config_with_endpoint(
    address: std::net::SocketAddr,
) -> sc_observability_otlp::TelemetryConfig {
    let transport = OtelConfig {
        enabled: false,
        endpoint: Some(
            OtlpEndpoint::new_typed(format!("http://{address}"))
                .expect("valid loopback OTLP endpoint"),
        ),
        ..OtelConfig::default()
    };
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid disabled telemetry config")
}

fn enabled_telemetry_config() -> V2TelemetryConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.endpoint = Some(
        V2OtlpEndpoint::new_typed("https://otel.example.internal").expect("valid OTLP endpoint"),
    );
    V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid enabled telemetry config")
}

#[cfg(feature = "legacy-http-json")]
fn enabled_legacy_http_json_config(
    address: std::net::SocketAddr,
) -> sc_observability_otlp::TelemetryConfig {
    let transport = OtelConfig {
        enabled: true,
        endpoint: Some(
            OtlpEndpoint::new_typed(format!("http://{address}"))
                .expect("valid loopback OTLP endpoint"),
        ),
        protocol: OtlpProtocol::HttpJson,
        ..OtelConfig::default()
    };
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid legacy HTTP/JSON config")
}

#[cfg(feature = "legacy-http-json")]
fn enabled_legacy_http_json_config_with_auth(
    address: std::net::SocketAddr,
    credential: &str,
) -> sc_observability_otlp::TelemetryConfig {
    let transport = OtelConfig {
        enabled: true,
        endpoint: Some(
            OtlpEndpoint::new_typed(format!("http://{address}"))
                .expect("valid loopback OTLP endpoint"),
        ),
        protocol: OtlpProtocol::HttpJson,
        auth_header: Some(AuthHeader::new_typed(credential).expect("valid test credential")),
        ..OtelConfig::default()
    };
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid authenticated legacy HTTP/JSON config")
}

#[cfg(feature = "legacy-http-json")]
fn enabled_legacy_http_json_retry_config(
    address: std::net::SocketAddr,
    max_retries: u32,
) -> sc_observability_otlp::TelemetryConfig {
    let transport = OtelConfig {
        enabled: true,
        endpoint: Some(
            OtlpEndpoint::new_typed(format!("http://{address}"))
                .expect("valid loopback OTLP endpoint"),
        ),
        protocol: OtlpProtocol::HttpJson,
        max_retries,
        initial_backoff_ms: DurationMs::from(1),
        max_backoff_ms: DurationMs::from(1),
        ..OtelConfig::default()
    };
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid legacy retry config")
}

#[cfg(feature = "otlp-sdk")]
fn enabled_sdk_grpc_config(address: std::net::SocketAddr) -> V2TelemetryConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.protocol = sc_observability_otlp::v2::OtlpProtocol::Grpc;
    transport.endpoint = Some(
        V2OtlpEndpoint::new_typed(format!("http://{address}"))
            .expect("valid loopback gRPC OTLP endpoint"),
    );
    V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid explicit-gRPC SDK config")
}

#[cfg(feature = "otlp-sdk")]
fn enabled_sdk_grpc_config_with_auth(
    address: std::net::SocketAddr,
    credential: &str,
) -> V2TelemetryConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.protocol = sc_observability_otlp::v2::OtlpProtocol::Grpc;
    transport.endpoint = Some(
        V2OtlpEndpoint::new_typed(format!("http://{address}"))
            .expect("valid loopback gRPC OTLP endpoint"),
    );
    transport.auth_header = Some(V2AuthHeader::new_typed(credential).expect("valid credential"));
    V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid authenticated SDK config")
}

fn service_name() -> ServiceName {
    ServiceName::new("test-service").expect("valid service")
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: Some(SpanId::new("fedcba9876543210").expect("valid parent span id")),
    }
}

fn log_event(service: ServiceName, message: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version"),
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
        outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST"),
            message: "projected".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect telemetry"]),
            docs: None,
            details: Map::default(),
        }),
        state_transition: Some(StateTransition {
            entity_kind: TargetCategory::new("agent").expect("valid target"),
            entity_id: Some(String::from("agent-123")),
            from_state: sc_observability_types::StateName::new("idle").expect("valid state"),
            to_state: sc_observability_types::StateName::new("running").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
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

#[cfg(feature = "legacy-http-json")]
fn completed_span_signals() -> [SpanSignal; 3] {
    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace.clone(),
        Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(10));
    [
        SpanSignal::Started(started),
        SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace,
            name: ActionName::new("tool.call").expect("valid event name"),
            attributes: Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
            diagnostic: None,
        }),
        SpanSignal::Ended(ended),
    ]
}

#[cfg(feature = "legacy-http-json")]
fn counter_metric() -> MetricRecord {
    MetricRecord {
        timestamp: Timestamp::UNIX_EPOCH,
        service: service_name(),
        name: MetricName::new("agent.events_total").expect("valid metric"),
        kind: MetricKind::Counter,
        value: 7.0,
        unit: Some(MetricUnit::new("1").expect("valid metric unit")),
        attributes: Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
    }
}

#[cfg(feature = "legacy-http-json")]
fn gauge_metric() -> MetricRecord {
    MetricRecord {
        timestamp: Timestamp::UNIX_EPOCH,
        service: service_name(),
        name: MetricName::new("agent.queue_depth").expect("valid metric"),
        kind: MetricKind::Gauge,
        value: 3.0,
        unit: Some(MetricUnit::new("1").expect("valid metric unit")),
        attributes: Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
    }
}

#[cfg(feature = "otlp-sdk")]
fn canonical_timestamp(seconds: i64) -> Timestamp {
    serde_json::from_str(&format!("\"1970-01-01T00:00:{seconds:02}Z\""))
        .expect("valid canonical timestamp")
}

#[cfg(feature = "otlp-sdk")]
fn canonical_trace_context() -> CanonicalTraceContext {
    CanonicalTraceContext::new(
        TraceId::new("1234567890abcdef1234567890abcdef").expect("valid trace id"),
        SpanId::new("1234567890abcdef").expect("valid span id"),
        TraceFlags::new(0x01),
    )
    .with_parent(SpanId::new("abcdef0123456789").expect("valid parent span id"))
}

#[cfg(feature = "otlp-sdk")]
fn canonical_completed_span_signals() -> [CanonicalSpanSignal; 3] {
    let trace = canonical_trace_context();
    let link = SpanLink::new(
        TraceId::new("fedcba9876543210fedcba9876543210").expect("valid linked trace id"),
        SpanId::new("fedcba9876543210").expect("valid linked span id"),
        TraceFlags::new(0x03),
        CanonicalAttributes::from([(
            "link.reason".to_owned(),
            CanonicalAttributeValue::String("follows".to_owned()),
        )]),
    );
    let started = CanonicalSpanRecord::<CanonicalSpanStarted>::new(
        canonical_timestamp(1),
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace.clone(),
        CanonicalAttributes::from([(
            "corpus.phase".to_owned(),
            CanonicalAttributeValue::String("decoded".to_owned()),
        )]),
    )
    .with_kind(CanonicalSpanKind::Client)
    .with_links(vec![link]);
    let ended = started
        .clone()
        .end(CanonicalSpanStatus::Error, DurationMs::from(10));
    [
        CanonicalSpanSignal::Started(started),
        CanonicalSpanSignal::Event(CanonicalSpanEvent {
            timestamp: canonical_timestamp(1),
            trace,
            name: ActionName::new("tool.call").expect("valid event name"),
            attributes: CanonicalAttributes::new(),
            diagnostic: None,
        }),
        CanonicalSpanSignal::Ended(ended),
    ]
}

#[cfg(feature = "otlp-sdk")]
fn canonical_metrics() -> Vec<CanonicalMetricRecord> {
    let metric = |name: &str, value| {
        CanonicalMetricRecord::try_new(
            canonical_timestamp(2),
            service_name(),
            MetricName::new(name).expect("valid metric name"),
            value,
        )
        .expect("valid canonical metric")
    };
    let histogram = |name: &str, bounds: Vec<f64>, buckets: Vec<u64>, count, sum| {
        metric(
            name,
            MetricValue::Histogram {
                point: HistogramPoint::try_new(
                    bounds
                        .into_iter()
                        .map(|bound| FiniteF64::new(bound).expect("finite bound"))
                        .collect(),
                    buckets,
                    count,
                    FiniteF64::new(sum).expect("finite sum"),
                )
                .expect("valid histogram point"),
                temporality: CanonicalAggregationTemporality::Delta,
                start_time: canonical_timestamp(1),
            },
        )
    };
    vec![
        metric(
            "agent.canonical.events_total",
            MetricValue::Sum {
                value: FiniteF64::new(7.0).expect("finite counter"),
                monotonic: true,
                temporality: CanonicalAggregationTemporality::Delta,
                start_time: canonical_timestamp(1),
            },
        ),
        metric(
            "agent.canonical.queue_depth",
            MetricValue::Gauge(FiniteF64::new(3.0).expect("finite gauge")),
        ),
        histogram("agent.canonical.histogram.zero", vec![], vec![3], 3, 4.5),
        histogram(
            "agent.canonical.histogram.one",
            vec![5.0],
            vec![1, 2],
            3,
            20.0,
        ),
        histogram(
            "agent.canonical.histogram.many",
            vec![1.0, 10.0, 100.0],
            vec![1, 2, 3, 4],
            10,
            555.0,
        ),
    ]
}

#[cfg(feature = "legacy-http-json")]
fn scalar_histogram_metric() -> MetricRecord {
    MetricRecord {
        timestamp: Timestamp::UNIX_EPOCH,
        service: service_name(),
        name: MetricName::new("agent.latency").expect("valid metric"),
        kind: MetricKind::Histogram,
        value: 3.0,
        unit: Some(MetricUnit::new("ms").expect("valid metric unit")),
        attributes: Map::from_iter([("corpus.phase".to_owned(), json!("decoded"))]),
    }
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
fn disabled_public_factory_never_contacts_an_available_collector() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    listener
        .set_nonblocking(true)
        .expect("make collector nonblocking");
    let telemetry = Telemetry::new_typed(disabled_telemetry_config_with_endpoint(
        listener.local_addr().expect("collector address"),
    ))
    .expect("disabled public factory constructs");

    telemetry
        .emit_log(&log_event(service_name(), "disabled"))
        .expect("disabled telemetry accepts local events");
    telemetry.flush_typed().expect("disabled flush succeeds");
    telemetry
        .shutdown_typed()
        .expect("disabled shutdown succeeds");

    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    assert_eq!(telemetry.health().state, TelemetryHealthState::Unavailable);
}

#[cfg(feature = "legacy-http-json")]
const LEGACY_COLLECTOR_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "legacy-http-json")]
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let deadline = Instant::now() + LEGACY_COLLECTOR_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out reading the complete collector request"
        );
        stream
            .set_read_timeout(Some(remaining))
            .expect("apply collector read deadline");
        let read = stream.read(&mut buffer).expect("read collector request");
        assert_ne!(read, 0, "collector request closed before its complete body");
        request.extend_from_slice(&buffer[..read]);
        assert!(
            request.len() <= MAX_REQUEST_BYTES,
            "collector request exceeded the bounded test limit"
        );

        let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let body_start = headers_end + 4;
        let headers = std::str::from_utf8(&request[..headers_end])
            .expect("HTTP/JSON collector headers are UTF-8");
        let content_length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim().parse::<usize>().expect("valid Content-Length"))
            .expect("HTTP/JSON collector request has Content-Length");
        if request.len() >= body_start + content_length {
            assert_eq!(
                request.len(),
                body_start + content_length,
                "one request is expected per collector connection"
            );
            break;
        }
    }
    String::from_utf8(request).expect("HTTP/JSON collector request is UTF-8")
}

#[cfg(feature = "legacy-http-json")]
fn accept_legacy_export(listener: &TcpListener) -> std::io::Result<std::net::TcpStream> {
    const POLL_INTERVAL: Duration = Duration::from_millis(10);
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + LEGACY_COLLECTOR_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(LEGACY_COLLECTOR_TIMEOUT))?;
                stream.set_write_timeout(Some(LEGACY_COLLECTOR_TIMEOUT))?;
                return Ok(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(std::io::Error::new(
                    ErrorKind::TimedOut,
                    "timed out waiting for the expected legacy export",
                ));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(feature = "legacy-http-json")]
fn assert_no_legacy_export(listener: &TcpListener) {
    const POLL_INTERVAL: Duration = Duration::from_millis(10);
    const QUIET_WINDOW: Duration = Duration::from_millis(250);
    listener
        .set_nonblocking(true)
        .expect("configure collector as nonblocking");
    let deadline = Instant::now() + QUIET_WINDOW;
    loop {
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => break,
            Ok((stream, _)) => panic!("unexpected legacy export connection: {stream:?}"),
            Err(error) => panic!("inspect collector connection: {error}"),
        }
    }
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn legacy_collector_reassembles_a_fragmented_request_larger_than_one_read() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let body = format!(r#"{{"payload":"{}"}}"#, "x".repeat(8 * 1024));
    let expected = format!(
        "POST /v1/logs HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let collector = std::thread::spawn(move || {
        let mut stream = accept_legacy_export(&listener).expect("accept fragmented request");
        let request = read_http_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("acknowledge fragmented request");
        request
    });

    let mut client = std::net::TcpStream::connect(address).expect("connect fragmented client");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("bound fragmented client response");
    let header_end = expected.find("\r\n\r\n").expect("request has headers") + 4;
    client
        .write_all(&expected.as_bytes()[..header_end - 3])
        .expect("write first header fragment");
    std::thread::sleep(Duration::from_millis(20));
    client
        .write_all(&expected.as_bytes()[header_end - 3..header_end + 2048])
        .expect("write second header and body fragment");
    std::thread::sleep(Duration::from_millis(20));
    client
        .write_all(&expected.as_bytes()[header_end + 2048..])
        .expect("write remaining body fragment");
    let mut response = Vec::new();
    client
        .read_to_end(&mut response)
        .expect("read complete collector response");
    assert!(
        std::str::from_utf8(&response)
            .expect("HTTP response is UTF-8")
            .starts_with("HTTP/1.1 200 OK")
    );
    assert_eq!(collector.join().expect("collector exits"), expected);
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn legacy_collector_fails_boundedly_when_the_expected_export_is_omitted() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let started = Instant::now();
    let error = accept_legacy_export(&listener).expect_err("missing export times out");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the two-second collector timeout is bounded by the watchdog"
    );
}

#[cfg(feature = "legacy-http-json")]
fn decoded_legacy_log(request: &str) -> Value {
    decoded_legacy_payload(request)["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0].clone()
}

#[cfg(feature = "legacy-http-json")]
fn decoded_legacy_payload(request: &str) -> Value {
    let (_, body) = request
        .split_once("\r\n\r\n")
        .expect("HTTP request has a payload delimiter");
    serde_json::from_str::<Value>(body).expect("collector payload is OTLP JSON")
}

#[cfg(feature = "legacy-http-json")]
fn assert_legacy_default_resource_and_scope(resource_group: &Value, scope_field: &str) {
    let resource_attributes = resource_group["resource"]["attributes"]
        .as_array()
        .expect("resource attributes");
    assert_eq!(
        resource_attributes.len(),
        1,
        "default resource is not widened"
    );
    assert_eq!(resource_attributes[0]["key"], "service.name");
    assert_eq!(
        resource_attributes[0]["value"]["stringValue"],
        service_name().as_str()
    );
    assert!(
        resource_group["resource"]["schemaUrl"].is_null(),
        "default resource has no schema URL"
    );

    let scope = &resource_group[scope_field][0]["scope"];
    assert_eq!(
        scope["name"], "",
        "default instrumentation scope is retained"
    );
    assert!(scope["version"].is_null(), "default scope has no version");
    assert!(
        scope["schemaUrl"].is_null(),
        "default scope has no schema URL"
    );
    assert_eq!(
        scope["attributes"],
        json!([]),
        "default scope has no attributes"
    );
}

#[cfg(feature = "legacy-http-json")]
fn assert_decoded_log(level: &Value, body: &Value, attributes: &[Value]) {
    assert_eq!(level, &json!(9), "Info maps to OTLP severity number 9");
    assert_eq!(
        body["stringValue"], "tool_use",
        "public log body is retained"
    );
    assert!(
        attributes.iter().any(|attribute| {
            attribute["key"] == "corpus.phase" && attribute["value"]["stringValue"] == "decoded"
        }),
        "public log attributes reach the collector"
    );
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_http_json_factory_exports_a_decoded_log_to_a_hermetic_collector() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let collector = std::thread::spawn(move || {
        let mut stream = accept_legacy_export(&listener).expect("accept legacy export");
        let request = read_http_request(&mut stream);
        assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("acknowledge legacy export");
        request
    });

    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config(address))
        .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "tool_use"))
        .expect("public legacy facade admits the log");
    telemetry
        .flush_typed()
        .expect("legacy worker barrier completes the export");
    telemetry
        .shutdown_typed()
        .expect("legacy shutdown completes after its worker barrier");

    let request = collector.join().expect("collector exits");
    let payload = decoded_legacy_payload(&request);
    assert_legacy_default_resource_and_scope(&payload["resourceLogs"][0], "scopeLogs");
    let log = decoded_legacy_log(&request);
    assert_decoded_log(
        &log["severityNumber"],
        &log["body"],
        log["attributes"].as_array().expect("attributes"),
    );
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_factory_redacts_rejected_authorization_from_diagnostics_and_health() {
    const CREDENTIAL: &str = "authorization: Bearer d9-legacy-secret";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let collector = std::thread::spawn(move || {
        let mut stream = accept_legacy_export(&listener).expect("accept legacy export");
        let request = read_http_request(&mut stream);
        assert!(
            request.contains(CREDENTIAL),
            "the public factory supplies the configured credential to the collector"
        );
        stream
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("reject credential");
    });

    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config_with_auth(
        address, CREDENTIAL,
    ))
    .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "rejected credential"))
        .expect("legacy factory admits log before export");
    let error = telemetry
        .flush_typed()
        .expect_err("collector credential rejection reaches the lifecycle barrier");
    let diagnostic = format!("{error:?}");
    assert!(
        !diagnostic.contains("d9-legacy-secret"),
        "public flush diagnostics redact credentials"
    );
    let health = format!("{:?}", telemetry.health());
    assert!(
        !health.contains("d9-legacy-secret"),
        "public health diagnostics redact credentials"
    );
    collector.join().expect("collector exits");
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_factory_reports_retry_exhaustion_after_the_configured_attempts() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let collector = std::thread::spawn(move || {
        for _ in 0..2 {
            let mut stream = accept_legacy_export(&listener).expect("accept retry attempt");
            let request = read_http_request(&mut stream);
            assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("reject retry attempt");
        }
    });

    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_retry_config(address, 1))
        .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "retry exhaustion"))
        .expect("legacy factory admits log before export");
    let error = telemetry
        .flush_typed()
        .expect_err("retryable collector failures exhaust the configured attempt budget");
    assert!(
        format!("{error:?}").contains("attempts were exhausted"),
        "the lifecycle reports retry exhaustion rather than a successful flush"
    );
    assert_eq!(telemetry.health().dropped_exports_total, 1);
    collector.join().expect("collector exits");
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_factory_recovers_a_partial_log_export_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let collector = std::thread::spawn(move || {
        for status in [
            b"500 Internal Server Error".as_slice(),
            b"200 OK".as_slice(),
        ] {
            let mut stream = accept_legacy_export(&listener).expect("accept export");
            let request = read_http_request(&mut stream);
            assert!(
                request.starts_with("POST /v1/logs HTTP/1.1"),
                "recovery scenario exercises only the log exporter, got request: {request}"
            );
            stream
                .write_all(b"HTTP/1.1 ")
                .and_then(|()| stream.write_all(status))
                .and_then(|()| {
                    stream.write_all(b"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                })
                .expect("write collector response");
        }
    });

    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_retry_config(address, 0))
        .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "partial legacy failure"))
        .expect("legacy factory admits failing log");
    telemetry
        .flush_typed()
        .expect_err("non-retried log failure reaches the lifecycle barrier");
    let failed = telemetry.health();
    assert_eq!(failed.state, TelemetryHealthState::Degraded);
    assert_eq!(
        failed.exporter_statuses[0].state,
        sc_observability_otlp::ExporterHealthState::Degraded
    );
    assert_eq!(
        failed.exporter_statuses[2].state,
        sc_observability_otlp::ExporterHealthState::Healthy,
        "the unaffected metric exporter remains healthy during the log failure"
    );

    telemetry
        .emit_log(&log_event(service_name(), "partial legacy recovery"))
        .expect("legacy factory admits recovery log");
    telemetry
        .flush_typed()
        .expect("next log export recovers the affected exporter");
    assert_eq!(
        telemetry.health().dropped_exports_total,
        1,
        "the successful recovery does not erase the recorded failed export"
    );
    telemetry
        .shutdown_typed()
        .expect("legacy recovery scenario shuts down cleanly");
    collector.join().expect("collector exits");
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_http_json_factory_exports_decoded_trace_counter_and_gauge() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let address = listener.local_addr().expect("collector address");
    let collector = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..3 {
            let mut stream = accept_legacy_export(&listener).expect("accept legacy export");
            let request = read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("acknowledge legacy export");
            requests.push(request);
        }
        requests
    });

    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config(address))
        .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "tool_use"))
        .expect("admit log");
    for signal in completed_span_signals() {
        telemetry.emit_span(&signal).expect("admit complete span");
    }
    telemetry
        .emit_metric(&counter_metric())
        .expect("admit counter");
    telemetry.emit_metric(&gauge_metric()).expect("admit gauge");
    telemetry
        .flush_typed()
        .expect("legacy worker barrier completes every export");
    telemetry
        .shutdown_typed()
        .expect("legacy shutdown completes after every export");
    telemetry
        .shutdown_typed()
        .expect("legacy public-factory shutdown remains idempotent after a completed export");

    let requests = collector.join().expect("collector exits");
    let logs = requests
        .iter()
        .find(|request| request.starts_with("POST /v1/logs HTTP/1.1"))
        .expect("logs request");
    let traces = requests
        .iter()
        .find(|request| request.starts_with("POST /v1/traces HTTP/1.1"))
        .expect("traces request");
    let metrics = requests
        .iter()
        .find(|request| request.starts_with("POST /v1/metrics HTTP/1.1"))
        .expect("metrics request");
    let log_payload = decoded_legacy_payload(logs);
    assert_legacy_default_resource_and_scope(&log_payload["resourceLogs"][0], "scopeLogs");
    let log = decoded_legacy_log(logs);
    assert_decoded_log(
        &log["severityNumber"],
        &log["body"],
        log["attributes"].as_array().expect("attributes"),
    );

    let (_, trace_body) = traces.split_once("\r\n\r\n").expect("trace payload");
    let trace_payload = serde_json::from_str::<Value>(trace_body).expect("OTLP trace JSON");
    assert_legacy_default_resource_and_scope(&trace_payload["resourceSpans"][0], "scopeSpans");
    let trace = trace_payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0].clone();
    assert_eq!(trace["name"], "agent.run");
    assert_eq!(trace["parentSpanId"], "fedcba9876543210");
    assert_eq!(trace["kind"], 1, "legacy facade maps to internal kind");
    assert_eq!(
        trace["flags"], 0,
        "the released legacy trace defaults its decoded trace flags to zero"
    );
    assert_eq!(trace["events"][0]["name"], "tool.call");
    assert_eq!(trace["status"]["code"], "STATUS_CODE_OK", "span is OK");
    assert_eq!(trace["endTimeUnixNano"], "10000000");

    let (_, metric_body) = metrics.split_once("\r\n\r\n").expect("metric payload");
    let metric_payload = serde_json::from_str::<Value>(metric_body).expect("OTLP metric JSON");
    assert_legacy_default_resource_and_scope(&metric_payload["resourceMetrics"][0], "scopeMetrics");
    let exported = metric_payload["resourceMetrics"][0]["scopeMetrics"][0]["metrics"].clone();
    assert_eq!(exported[0]["name"], "agent.events_total");
    assert!(exported[0]["sum"].is_object());
    assert_eq!(exported[1]["name"], "agent.queue_depth");
    assert!(exported[1]["gauge"].is_object());
}

#[cfg(feature = "legacy-http-json")]
#[test]
#[ignore = "requires a harness-owned pinned desktop viewer; set D9_VIEWER_LEGACY_ADDRESS"]
fn public_legacy_factory_exports_three_signals_to_the_pinned_desktop_viewer() {
    let address = std::env::var("D9_VIEWER_LEGACY_ADDRESS")
        .expect("D9_VIEWER_LEGACY_ADDRESS is set to the isolated viewer address")
        .parse()
        .expect("D9_VIEWER_LEGACY_ADDRESS is a socket address");
    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config(address))
        .expect("public legacy factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "d9-viewer-legacy-factory"))
        .expect("legacy factory admits viewer log");
    for signal in completed_span_signals() {
        telemetry
            .emit_span(&signal)
            .expect("legacy factory admits viewer span signal");
    }
    telemetry
        .emit_metric(&counter_metric())
        .expect("legacy factory admits viewer counter");
    telemetry
        .emit_metric(&gauge_metric())
        .expect("legacy factory admits viewer gauge");
    telemetry
        .flush_typed()
        .expect("legacy worker barrier delivers all viewer signals");
    telemetry
        .shutdown_typed()
        .expect("legacy viewer delivery shuts down after its worker barrier");
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_factory_accepts_scalar_histogram_without_wire_or_degraded_health() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config(
        listener.local_addr().expect("collector address"),
    ))
    .expect("public legacy factory constructs");
    telemetry
        .emit_metric(&scalar_histogram_metric())
        .expect("released scalar metric is accepted at admission");
    telemetry
        .flush_typed()
        .expect("released scalar metric needs no projection or transport");

    let health = telemetry.health();
    assert_eq!(
        health.dropped_exports_total, 0,
        "accepted released scalar histograms do not increment dropped exports"
    );
    assert_eq!(
        health.state,
        TelemetryHealthState::Healthy,
        "no exporter is contacted for a released scalar histogram"
    );
    assert!(
        health.last_error.is_none(),
        "accepted released scalar histogram retains no exporter error: {health:?}"
    );
    assert_no_legacy_export(&listener);
    telemetry
        .shutdown_typed()
        .expect("released scalar histogram shutdown remains clean");
}

#[cfg(feature = "legacy-http-json")]
#[test]
fn public_legacy_factory_shutdown_is_idempotent() {
    let telemetry = Telemetry::new_typed(enabled_legacy_http_json_config(
        "127.0.0.1:1".parse().expect("loopback address"),
    ))
    .expect("public legacy factory constructs");

    telemetry
        .shutdown_typed()
        .expect("first synchronous compatibility shutdown completes");
    telemetry
        .shutdown_typed()
        .expect("second synchronous compatibility shutdown is idempotent");
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_explicit_grpc_factory_exports_a_decoded_log_to_a_hermetic_collector() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC collector");
        let address = listener.local_addr().expect("collector address");
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .serve_with_incoming(
                    LogsServiceServer::new(CapturingLogsService { sender }),
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                )
                .await
                .expect("gRPC collector serves until aborted");
        });

        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(address))
            .expect("public SDK factory constructs with explicit gRPC");
        telemetry
            .emit_log(&log_event(service_name(), "tool_use"))
            .expect("public SDK facade admits the log");
        telemetry
            .flush_async_typed()
            .await
            .expect("awaited SDK lifecycle delivers the log");

        let request = tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
            .await
            .expect("collector receives SDK export before deadline")
            .expect("collector channel remains open");
        let resource_log = &request.resource_logs[0];
        assert_sdk_default_resource_and_scope(
            resource_log.resource.as_ref().expect("resource"),
            resource_log.scope_logs[0].scope.as_ref().expect("scope"),
            &resource_log.schema_url,
            &resource_log.scope_logs[0].schema_url,
        );
        let log = &request.resource_logs[0].scope_logs[0].log_records[0];
        let body = match log.body.as_ref().and_then(|body| body.value.as_ref()) {
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(value)) => value,
            other => panic!("expected string log body, got {other:?}"),
        };
        assert_eq!(log.severity_number, 9, "Info maps to OTLP severity number 9");
        assert_eq!(body, "tool_use", "public log body is retained");
        assert!(log.attributes.iter().any(|attribute| {
            attribute.key == "corpus.phase"
                && matches!(
                    attribute.value.as_ref().and_then(|value| value.value.as_ref()),
                    Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(value))
                        if value == "decoded"
                )
        }), "public log attributes reach the collector");

        telemetry
            .shutdown_async_typed()
            .await
            .expect("awaited SDK shutdown is ordered after the export");
        collector.abort();
        let _ = collector.await;
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_redacts_rejected_authorization_from_diagnostics_and_health() {
    const CREDENTIAL: &str = "Bearer d9-sdk-secret";
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC collector");
        let address = listener.local_addr().expect("collector address");
        let (sender, mut metadata) = tokio::sync::mpsc::channel(1);
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .serve_with_incoming(
                    LogsServiceServer::new(RejectingLogsService { sender }),
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                )
                .await
                .expect("gRPC collector serves until aborted");
        });

        let telemetry =
            V2Telemetry::new_typed(enabled_sdk_grpc_config_with_auth(address, CREDENTIAL))
                .expect("public SDK factory constructs with credential");
        telemetry
            .emit_log(&log_event(service_name(), "rejected credential"))
            .expect("SDK factory admits log before export");
        let error = telemetry
            .flush_async_typed()
            .await
            .expect_err("collector credential rejection reaches awaited lifecycle barrier");
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), metadata.recv())
            .await
            .expect("collector receives SDK export before deadline")
            .expect("collector metadata channel remains open");
        assert_eq!(
            received
                .get("authorization")
                .expect("configured authorization metadata")
                .to_str()
                .expect("ASCII authorization metadata"),
            CREDENTIAL,
            "the public factory supplies the configured credential to the collector"
        );
        let diagnostic = format!("{error:?}");
        assert!(
            !diagnostic.contains("d9-sdk-secret"),
            "public flush diagnostics redact credentials"
        );
        let health = format!("{:?}", telemetry.health());
        assert!(
            !health.contains("d9-sdk-secret"),
            "public health diagnostics redact credentials"
        );

        collector.abort();
        let _ = collector.await;
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_reports_retry_exhaustion_after_the_default_attempts() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC collector");
        let address = listener.local_addr().expect("collector address");
        let (attempts, mut received_attempts) = tokio::sync::mpsc::channel(4);
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .serve_with_incoming(
                    LogsServiceServer::new(UnavailableLogsService { attempts }),
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                )
                .await
                .expect("gRPC collector serves until aborted");
        });

        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(address))
            .expect("public SDK factory constructs");
        telemetry
            .emit_log(&log_event(service_name(), "retry exhaustion"))
            .expect("SDK factory admits log before export");
        let error = telemetry
            .flush_async_typed()
            .await
            .expect_err("retryable collector failures exhaust the SDK retry budget");
        assert!(
            format!("{error:?}").contains("OTLP log export failed"),
            "the lifecycle reports the terminal export failure"
        );
        for _ in 0..4 {
            tokio::time::timeout(std::time::Duration::from_secs(2), received_attempts.recv())
                .await
                .expect("collector receives each SDK retry before deadline")
                .expect("retry-attempt channel remains open");
        }
        assert_eq!(telemetry.health().dropped_exports_total, 1);

        collector.abort();
        let _ = collector.await;
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the one public-factory scenario intentionally keeps collector setup, lifecycle, and decoded signal assertions together"
)]
fn public_sdk_explicit_grpc_factory_exports_decoded_trace_counter_and_gauge() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC collector");
        let address = listener.local_addr().expect("collector address");
        let (log_sender, mut logs) = tokio::sync::mpsc::channel(1);
        let (trace_sender, mut traces) = tokio::sync::mpsc::channel(1);
        let (metric_sender, mut metrics) = tokio::sync::mpsc::channel(1);
        let (shutdown_sender, shutdown) = tokio::sync::oneshot::channel();
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(LogsServiceServer::new(CapturingLogsService {
                    sender: log_sender,
                }))
                .add_service(TraceServiceServer::new(CapturingTraceService {
                    sender: trace_sender,
                }))
                .add_service(MetricsServiceServer::new(CapturingMetricsService {
                    sender: metric_sender,
                }))
                .serve_with_incoming_shutdown(
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown.await;
                    },
                )
                .await
                .expect("gRPC collector stops cleanly");
        });

        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(address))
            .expect("public SDK factory constructs with explicit gRPC");
        telemetry
            .emit_log(&log_event(service_name(), "tool_use"))
            .expect("admit log");
        for signal in canonical_completed_span_signals() {
            telemetry.emit_span(&signal).expect("admit complete span");
        }
        for metric in canonical_metrics() {
            telemetry
                .emit_metric(&metric)
                .expect("admit canonical metric");
        }
        telemetry
            .flush_async_typed()
            .await
            .expect("awaited SDK lifecycle delivers all signals");

        let log_request = tokio::time::timeout(std::time::Duration::from_secs(2), logs.recv())
            .await
            .expect("collector receives logs")
            .expect("logs channel open");
        let resource_log = &log_request.resource_logs[0];
        assert_sdk_default_resource_and_scope(
            resource_log.resource.as_ref().expect("resource"),
            resource_log.scope_logs[0].scope.as_ref().expect("scope"),
            &resource_log.schema_url,
            &resource_log.scope_logs[0].schema_url,
        );

        let trace_request = tokio::time::timeout(std::time::Duration::from_secs(2), traces.recv())
            .await
            .expect("collector receives traces")
            .expect("traces channel open");
        let resource_span = &trace_request.resource_spans[0];
        assert_sdk_default_resource_and_scope(
            resource_span.resource.as_ref().expect("resource"),
            resource_span.scope_spans[0].scope.as_ref().expect("scope"),
            &resource_span.schema_url,
            &resource_span.scope_spans[0].schema_url,
        );
        let span = &trace_request.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(span.name, "agent.run");
        assert_eq!(
            span.parent_span_id,
            vec![0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89]
        );
        assert_eq!(span.kind, 3, "canonical client span kind reaches gRPC");
        assert_eq!(span.flags, 1, "canonical trace flags reach gRPC");
        assert_eq!(span.events[0].name, "tool.call");
        assert_eq!(
            span.status.as_ref().expect("status").code,
            2,
            "span is error"
        );
        assert_eq!(span.start_time_unix_nano, 1_000_000_000);
        assert_eq!(span.end_time_unix_nano, 1_010_000_000);
        assert_eq!(span.links.len(), 1, "canonical link reaches gRPC");
        assert_eq!(
            span.links[0].flags, 3,
            "canonical linked-trace flags reach gRPC"
        );

        let metric_request =
            tokio::time::timeout(std::time::Duration::from_secs(2), metrics.recv())
                .await
                .expect("collector receives metrics")
                .expect("metrics channel open");
        let resource_metric = &metric_request.resource_metrics[0];
        assert_sdk_default_resource_and_scope(
            resource_metric.resource.as_ref().expect("resource"),
            resource_metric.scope_metrics[0]
                .scope
                .as_ref()
                .expect("scope"),
            &resource_metric.schema_url,
            &resource_metric.scope_metrics[0].schema_url,
        );
        let exported = &metric_request.resource_metrics[0].scope_metrics[0].metrics;
        assert_eq!(exported.len(), 5);
        assert_eq!(exported[0].name, "agent.canonical.events_total");
        assert!(matches!(
            &exported[0].data,
            Some(opentelemetry_proto::tonic::metrics::v1::metric::Data::Sum(
                _
            ))
        ));
        assert_eq!(exported[1].name, "agent.canonical.queue_depth");
        assert!(matches!(
            &exported[1].data,
            Some(opentelemetry_proto::tonic::metrics::v1::metric::Data::Gauge(_))
        ));
        for (name, bounds, counts, count, sum) in [
            ("agent.canonical.histogram.zero", vec![], vec![3], 3, 4.5),
            (
                "agent.canonical.histogram.one",
                vec![5.0],
                vec![1, 2],
                3,
                20.0,
            ),
            (
                "agent.canonical.histogram.many",
                vec![1.0, 10.0, 100.0],
                vec![1, 2, 3, 4],
                10,
                555.0,
            ),
        ] {
            let metric = exported
                .iter()
                .find(|metric| metric.name == name)
                .unwrap_or_else(|| panic!("{name} reaches the gRPC collector"));
            let Some(opentelemetry_proto::tonic::metrics::v1::metric::Data::Histogram(histogram)) =
                &metric.data
            else {
                panic!("{name} is a histogram: {metric:?}");
            };
            let point = &histogram.data_points[0];
            assert_eq!(point.explicit_bounds, bounds, "{name}");
            assert_eq!(point.bucket_counts, counts, "{name}");
            assert_eq!(point.count, count, "{name}");
            assert_eq!(point.sum, Some(sum), "{name}");
        }

        telemetry
            .shutdown_async_typed()
            .await
            .expect("awaited SDK shutdown is ordered after every export");
        telemetry
            .shutdown_async_typed()
            .await
            .expect("SDK public-factory shutdown remains idempotent after a completed export");
        let _ = shutdown_sender.send(());
        collector.await.expect("collector task exits");
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
#[ignore = "requires a harness-owned pinned desktop viewer; set D9_VIEWER_SDK_ADDRESS"]
fn public_sdk_factory_exports_three_signals_to_the_pinned_desktop_viewer() {
    let address = std::env::var("D9_VIEWER_SDK_ADDRESS")
        .expect("D9_VIEWER_SDK_ADDRESS is set to the isolated viewer address")
        .parse()
        .expect("D9_VIEWER_SDK_ADDRESS is a socket address");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(address))
            .expect("public SDK factory constructs");
        telemetry
            .emit_log(&log_event(service_name(), "d9-viewer-sdk-factory"))
            .expect("SDK factory admits viewer log");
        for signal in canonical_completed_span_signals() {
            telemetry
                .emit_span(&signal)
                .expect("SDK factory admits viewer span signal");
        }
        for metric in canonical_metrics() {
            telemetry
                .emit_metric(&metric)
                .expect("SDK factory admits viewer canonical metric");
        }
        telemetry
            .flush_async_typed()
            .await
            .expect("awaited SDK lifecycle delivers all viewer signals");
        telemetry
            .shutdown_async_typed()
            .await
            .expect("SDK viewer delivery shuts down after its awaited lifecycle barrier");
    });
}

#[test]
fn builder_registration_attaches_logs_spans_and_metrics() {
    let telemetry = Arc::new(Telemetry::new(disabled_telemetry_config()).expect("telemetry"));
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
    assert_eq!(health.state, TelemetryHealthState::Disabled);
    assert_eq!(health.dropped_exports_total, 0);
    assert_eq!(
        runtime_health
            .telemetry
            .expect("attached telemetry health")
            .state,
        TelemetryHealthState::Disabled
    );
}

#[test]
fn typed_projector_inputs_forward_through_retained_registration() {
    let telemetry = Arc::new(Telemetry::new(disabled_telemetry_config()).expect("typed telemetry"));
    let root = temp_root("typed-integration");
    let config = ObservabilityConfig::default_for(
        ToolName::new("test-service").expect("valid tool"),
        root.clone(),
    )
    .expect("config");
    let runtime = Observability::builder(config)
        .with_observability_health_provider(telemetry.clone())
        .register_projection(
            TelemetryProjectors::new(telemetry.clone())
                .with_log_projector(legacy_log_projector(Arc::new(TypedStaticLogProjector)))
                .with_span_projector(legacy_span_projector(Arc::new(TypedStaticSpanProjector)))
                .with_metric_projector(legacy_metric_projector(Arc::new(
                    TypedStaticMetricProjector,
                )))
                .with_filter(Arc::new(AllowAll))
                .into_registration(),
        )
        .build()
        .expect("runtime");

    runtime.emit(observation()).expect("emit");
    telemetry.flush().expect("typed flush");

    let log_path = root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "test-service{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    assert!(
        std::fs::read_to_string(log_path)
            .expect("read projected log file")
            .contains("\"action\":\"agent.observe\"")
    );
    assert_eq!(telemetry.health().state, TelemetryHealthState::Disabled);
}

#[test]
fn enabled_configuration_rejects_unavailable_backend() {
    let Err(error) = V2Telemetry::new(enabled_telemetry_config()) else {
        panic!("enabled configuration must not receive a fallback exporter");
    };

    #[cfg(not(feature = "otlp-sdk"))]
    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND
    );
    #[cfg(feature = "otlp-sdk")]
    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED
    );
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_recovers_a_partial_log_export_failure() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gRPC collector");
        let address = listener.local_addr().expect("collector address");
        let attempts = Arc::new(AtomicUsize::new(0));
        let recovering_attempts = Arc::clone(&attempts);
        let (metric_sender, mut metrics) = tokio::sync::mpsc::channel(1);
        let (shutdown_sender, shutdown) = tokio::sync::oneshot::channel();
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(LogsServiceServer::new(RecoveringLogsService {
                    attempts: recovering_attempts,
                }))
                .add_service(MetricsServiceServer::new(CapturingMetricsService {
                    sender: metric_sender,
                }))
                .serve_with_incoming_shutdown(
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown.await;
                    },
                )
                .await
                .expect("gRPC collector stops cleanly");
        });

        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(address))
            .expect("public SDK factory constructs");
        let canonical_gauge = canonical_metrics()
            .into_iter()
            .nth(1)
            .expect("canonical gauge fixture");
        telemetry
            .emit_metric(&canonical_gauge)
            .expect("SDK factory admits healthy sibling metric");
        telemetry
            .flush_async_typed()
            .await
            .expect("metric export succeeds before the partial failure");
        let _metric = tokio::time::timeout(std::time::Duration::from_secs(2), metrics.recv())
            .await
            .expect("collector receives healthy metric")
            .expect("metric collector channel remains open");

        telemetry
            .emit_log(&log_event(service_name(), "partial SDK failure"))
            .expect("SDK factory admits failing log");
        telemetry
            .flush_async_typed()
            .await
            .expect_err("terminal log failure reaches the awaited lifecycle barrier");
        let failed = telemetry.health();
        assert_eq!(failed.state, TelemetryHealthState::Degraded);
        assert_eq!(
            failed.exporter_statuses[0].state,
            sc_observability_otlp::ExporterHealthState::Degraded
        );
        assert_eq!(
            failed.exporter_statuses[2].state,
            sc_observability_otlp::ExporterHealthState::Healthy,
            "the independent metric exporter remains healthy during the log failure"
        );

        telemetry
            .emit_log(&log_event(service_name(), "partial SDK recovery"))
            .expect("SDK factory admits recovery log");
        telemetry
            .flush_async_typed()
            .await
            .expect("next log export recovers the affected exporter");
        assert_eq!(
            telemetry.health().dropped_exports_total,
            1,
            "the successful recovery does not erase the recorded failed export"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        telemetry
            .shutdown_async_typed()
            .await
            .expect("SDK recovery scenario shuts down cleanly");
        let _ = shutdown_sender.send(());
        collector.await.expect("collector task exits");
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn enabled_sdk_telemetry_awaits_shared_lifecycle_and_closes_admission() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let telemetry = V2Telemetry::new(enabled_telemetry_config())
            .expect("enabled SDK telemetry is constructed on its caller runtime");

        assert!(telemetry.flush_typed().is_err());
        assert!(telemetry.shutdown_typed().is_err());

        telemetry
            .flush_async_typed()
            .await
            .expect("empty shared SDK lifecycle barrier completes");
        telemetry
            .shutdown_async_typed()
            .await
            .expect("shared SDK lifecycle shutdown completes");

        assert!(matches!(
            telemetry.emit_log(&log_event(service_name(), "after-shutdown")),
            Err(sc_observability_types::v2::TelemetryError::Shutdown { .. })
        ));
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_shutdown_is_idempotent() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let telemetry = V2Telemetry::new_typed(enabled_sdk_grpc_config(
            "127.0.0.1:1".parse().expect("loopback address"),
        ))
        .expect("public SDK factory constructs");

        telemetry
            .shutdown_async_typed()
            .await
            .expect("first awaited SDK shutdown completes");
        telemetry
            .shutdown_async_typed()
            .await
            .expect("second awaited SDK shutdown is idempotent");
    });
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn admitted_sdk_export_failure_reaches_public_health_once() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let mut config = enabled_telemetry_config();
        config.transport.endpoint = Some(
            V2OtlpEndpoint::new_typed("http://127.0.0.1:1").expect("valid unavailable endpoint"),
        );
        config.transport.timeout_ms = Some(DurationMs::from(1));
        let telemetry = V2Telemetry::new_typed(config).expect("SDK telemetry construction");

        telemetry
            .emit_log(&log_event(service_name(), "export failure"))
            .expect("buffer log before lifecycle barrier");
        assert!(telemetry.flush_async_typed().await.is_err());

        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Degraded);
        assert_eq!(health.dropped_exports_total, 1);
        assert!(health.last_error.is_some());
        assert_eq!(
            health.exporter_statuses[0].state,
            sc_observability_otlp::ExporterHealthState::Degraded
        );
    });
}
