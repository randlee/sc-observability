#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use std::net::TcpListener;
#[cfg(feature = "otlp-sdk")]
use std::sync::Arc;
#[cfg(feature = "otlp-sdk")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "sync-http")]
use std::time::{Duration, Instant};

#[cfg(feature = "sync-http")]
use std::io::{ErrorKind, Read};

#[cfg(feature = "otlp-sdk")]
use sc_observability_otlp::v2::AuthHeader as V2AuthHeader;
use sc_observability_otlp::v2::{
    OtelConfig as V2OtelConfig, OtlpEndpoint as V2OtlpEndpoint, Telemetry as V2Telemetry,
    TelemetryConfig as V2TelemetryConfig, TelemetryConfigBuilder as V2TelemetryConfigBuilder,
};
use sc_observability_otlp::{LogsConfig, MetricsConfig, TracesConfig};
#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use sc_observability_types::TelemetryHealthState;
#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use sc_observability_types::v2::{
    AggregationTemporality as CanonicalAggregationTemporality,
    AttributeValue as CanonicalAttributeValue, Attributes as CanonicalAttributes, FiniteF64,
    HistogramPoint, MetricRecord as CanonicalMetricRecord, MetricValue,
    SpanEvent as CanonicalSpanEvent, SpanKind as CanonicalSpanKind, SpanLink,
    SpanRecord as CanonicalSpanRecord, SpanSignal as CanonicalSpanSignal,
    SpanStarted as CanonicalSpanStarted, SpanStatus as CanonicalSpanStatus,
    TraceContext as CanonicalTraceContext, TraceFlags,
};
#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricName, OutcomeLabel, ProcessIdentity,
    Remediation, SchemaVersion, SpanId, StateTransition, TargetCategory, Timestamp, TraceContext,
    TraceId,
};
use sc_observability_types::{DurationMs, ServiceName};
#[cfg(feature = "sync-http")]
use serde_json::Value;
#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
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

fn enabled_telemetry_config() -> V2TelemetryConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.timeout_ms = Some(DurationMs::from(2_000));
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

#[cfg(feature = "otlp-sdk")]
fn enabled_sdk_grpc_config(address: std::net::SocketAddr) -> V2TelemetryConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.timeout_ms = Some(DurationMs::from(2_000));
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
    transport.timeout_ms = Some(DurationMs::from(2_000));
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

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: Some(SpanId::new("fedcba9876543210").expect("valid parent span id")),
    }
}

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
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

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
fn canonical_timestamp(seconds: i64) -> Timestamp {
    serde_json::from_str(&format!("\"1970-01-01T00:00:{seconds:02}Z\""))
        .expect("valid canonical timestamp")
}

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
fn canonical_trace_context() -> CanonicalTraceContext {
    CanonicalTraceContext::new(
        TraceId::new("1234567890abcdef1234567890abcdef").expect("valid trace id"),
        SpanId::new("1234567890abcdef").expect("valid span id"),
        TraceFlags::new(0x01),
    )
    .with_parent(SpanId::new("abcdef0123456789").expect("valid parent span id"))
}

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
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

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
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

#[cfg(feature = "sync-http")]
const SYNC_HTTP_COLLECTOR_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "sync-http")]
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let deadline = Instant::now() + SYNC_HTTP_COLLECTOR_TIMEOUT;
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

#[cfg(feature = "sync-http")]
fn accept_sync_http_export(listener: &TcpListener) -> std::io::Result<std::net::TcpStream> {
    const POLL_INTERVAL: Duration = Duration::from_millis(10);
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + SYNC_HTTP_COLLECTOR_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(SYNC_HTTP_COLLECTOR_TIMEOUT))?;
                stream.set_write_timeout(Some(SYNC_HTTP_COLLECTOR_TIMEOUT))?;
                return Ok(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(std::io::Error::new(
                    ErrorKind::TimedOut,
                    "timed out waiting for the expected sync-http export",
                ));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(feature = "sync-http")]
fn decoded_sync_http_log(request: &str) -> Value {
    decoded_sync_http_payload(request)["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0].clone()
}

#[cfg(feature = "sync-http")]
fn decoded_sync_http_payload(request: &str) -> Value {
    let (_, body) = request
        .split_once("\r\n\r\n")
        .expect("HTTP request has a payload delimiter");
    serde_json::from_str::<Value>(body).expect("collector payload is OTLP JSON")
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
        let export_error = error
            .export_cause()
            .expect("the typed flush failure retains its typed export cause");
        assert_eq!(
            export_error.code(),
            sc_observability_types::error_codes::otlp::OTLP_EXPORT_TERMINAL,
            "the retained export cause classifies the SDK terminal export failure with the stable error code"
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

#[cfg(feature = "sync-http")]
#[test]
#[ignore = "requires a harness-owned pinned desktop viewer; set D9_VIEWER_SYNC_HTTP_ADDRESS"]
fn public_sync_http_factory_exports_three_signals_to_the_pinned_desktop_viewer() {
    let address: std::net::SocketAddr = std::env::var("D9_VIEWER_SYNC_HTTP_ADDRESS")
        .expect("D9_VIEWER_SYNC_HTTP_ADDRESS is set to the isolated viewer address")
        .parse()
        .expect("D9_VIEWER_SYNC_HTTP_ADDRESS is a socket address");
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.backend = sc_observability_otlp::v2::ExporterBackend::SyncHttp;
    transport.protocol = sc_observability_otlp::v2::OtlpProtocol::HttpJson;
    transport.endpoint = Some(
        V2OtlpEndpoint::new_typed(format!("http://{address}")).expect("valid viewer OTLP endpoint"),
    );
    let config = V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid canonical sync-http viewer config");
    let telemetry = V2Telemetry::new_typed(config).expect("canonical sync-http factory constructs");
    telemetry
        .emit_log(&log_event(service_name(), "d9-viewer-sync-http-factory"))
        .expect("sync-http factory admits viewer log");
    for signal in canonical_completed_span_signals() {
        telemetry
            .emit_span(&signal)
            .expect("sync-http factory admits canonical viewer span signal");
    }
    for metric in canonical_metrics() {
        telemetry
            .emit_metric(&metric)
            .expect("sync-http factory admits canonical viewer metric");
    }
    telemetry
        .flush_typed()
        .expect("sync-http worker barrier delivers all viewer signals");
    telemetry
        .shutdown_typed()
        .expect("sync-http viewer delivery shuts down after its worker barrier");
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
fn sdk_hermetic_configs_bound_transport_lifecycle_requests() {
    let address = "127.0.0.1:1".parse().expect("loopback address");
    let configs = [
        enabled_telemetry_config(),
        enabled_sdk_grpc_config(address),
        enabled_sdk_grpc_config_with_auth(address, "Bearer test-credential"),
    ];

    for config in configs {
        assert_eq!(
            u64::from(config.transport.timeout_ms.expect("SDK test timeout")),
            2_000,
            "SDK hermetic lifecycle tests must not inherit the unbounded transport default"
        );
    }
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

/// Bounds the entire public-factory scenario, including exporter construction and cleanup.
/// The parent owns and reaps the child even when a regression strands a worker thread.
#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
fn public_factory_scenario_child(name: &str) -> bool {
    const CHILD_CASE: &str = "SC_OTLP_PUBLIC_FACTORY_CASE";
    const WATCHDOG: std::time::Duration = std::time::Duration::from_secs(30);
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        return true;
    }
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", name, "--nocapture"])
        .env(CHILD_CASE, name)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn isolated public factory scenario");
    let deadline = std::time::Instant::now() + WATCHDOG;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut output = String::new();
                std::io::Read::read_to_string(
                    &mut child.stdout.take().expect("captured child output"),
                    &mut output,
                )
                .expect("read completed child output");
                assert!(
                    status.success(),
                    "public factory scenario {name}: {status}\n{output}"
                );
                assert!(
                    output.contains("test result: ok. 1 passed; 0 failed;"),
                    "exactly one scenario must execute, not a zero-test success: {output}"
                );
                return false;
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                // Watchdog polling only; no functional assertion depends on this delay.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("public factory scenario {name} exceeded watchdog: {result:?}");
            }
        }
    }
}

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
#[cfg(feature = "otlp-sdk")]
fn public_flush_export_cause(
    failure: &sc_observability_types::v2::FlushError,
) -> &sc_observability_types::v2::ExportError {
    let sc_observability_types::v2::FlushError::Drain { context: _ } = failure else {
        panic!("expected a drain failure from the unavailable collector");
    };
    failure.export_cause().expect("native typed exporter cause")
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_recovers_after_collector_unavailability() {
    if !public_factory_scenario_child("public_sdk_factory_recovers_after_collector_unavailability")
    {
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve collector endpoint");
        let address = reservation.local_addr().expect("endpoint");
        drop(reservation);
        let mut config = enabled_sdk_grpc_config(address);
        config.transport.timeout_ms = Some(2_000_u64.into());
        let telemetry = V2Telemetry::new_typed(config).expect("public SDK factory");
        telemetry.emit_log(&log_event(service_name(), "unavailable SDK")).expect("admission");
        let failure = telemetry.flush_async_typed().await.expect_err("unavailable gRPC collector fails export");
        assert!(matches!(public_flush_export_cause(&failure), sc_observability_types::v2::ExportError::Transport { .. }));
        assert_eq!(telemetry.health().state, TelemetryHealthState::Degraded);
        assert_eq!(telemetry.health().dropped_exports_total, 1);
        assert!(telemetry.health().last_error.is_some());

        let listener = tokio::net::TcpListener::bind(address).await.expect("recover same endpoint");
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let collector = tokio::spawn(async move {
            tonic::transport::Server::builder().serve_with_incoming(
                LogsServiceServer::new(CapturingLogsService { sender }),
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            ).await.expect("serve recovered collector");
        });
        telemetry.emit_log(&log_event(service_name(), "recovered SDK")).expect("admission after recovery");
        telemetry.flush_async_typed().await.expect("same factory delivers after recovery");
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
            .await.expect("recovery payload watchdog").expect("recovery payload");
        let log = &request.resource_logs[0].scope_logs[0].log_records[0];
        assert_eq!(log.severity_number, 9);
        assert!(matches!(log.body.as_ref().and_then(|body| body.value.as_ref()),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(value)) if value == "recovered SDK"));
        assert_eq!(telemetry.health().state, TelemetryHealthState::Healthy);
        assert_eq!(telemetry.health().dropped_exports_total, 1);
        telemetry.shutdown_async_typed().await.expect("shutdown recovered SDK");
        collector.abort();
        let _ = collector.await;
    });
}

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
fn invalid_event() -> LogEvent {
    let mut event = log_event(service_name(), "invalid entity must never reach collector");
    event
        .state_transition
        .as_mut()
        .expect("transition fixture")
        .entity_id = Some(String::new());
    event
}

#[cfg(feature = "sync-http")]
#[test]
fn public_sync_http_factory_rejects_invalid_model_before_collector_contact() {
    if !public_factory_scenario_child(
        "public_sync_http_factory_rejects_invalid_model_before_collector_contact",
    ) {
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("collector contact witness");
    listener.set_nonblocking(true).expect("nonblocking witness");
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.backend = sc_observability_otlp::v2::ExporterBackend::SyncHttp;
    transport.protocol = sc_observability_otlp::v2::OtlpProtocol::HttpJson;
    transport.endpoint = Some(
        V2OtlpEndpoint::new_typed(format!("http://{}", listener.local_addr().unwrap())).unwrap(),
    );
    let config = V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("canonical sync-http config");
    let telemetry = V2Telemetry::new_typed(config).expect("canonical public sync-http factory");
    let error = telemetry
        .emit_log(&invalid_event())
        .expect_err("invalid canonical model");
    assert!(matches!(
        error,
        sc_observability_types::v2::TelemetryError::Event(
            sc_observability_types::v2::EventError::Validation { .. }
        )
    ));
    telemetry.flush_typed().expect("nothing was admitted");
    telemetry.shutdown_typed().expect("empty shutdown");
    assert_eq!(
        telemetry.health().dropped_exports_total,
        0,
        "rejected before exporter admission"
    );
    assert_eq!(
        listener.accept().expect_err("no collector contact").kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn public_sdk_factory_rejects_invalid_model_before_collector_contact() {
    if !public_factory_scenario_child(
        "public_sdk_factory_rejects_invalid_model_before_collector_contact",
    ) {
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    runtime.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").expect("collector contact witness");
        listener.set_nonblocking(true).expect("nonblocking witness");
        let telemetry =
            V2Telemetry::new_typed(enabled_sdk_grpc_config(listener.local_addr().unwrap()))
                .expect("public SDK factory");
        let error = telemetry
            .emit_log(&invalid_event())
            .expect_err("invalid canonical model");
        assert!(matches!(
            error,
            sc_observability_types::v2::TelemetryError::Event(
                sc_observability_types::v2::EventError::Validation { .. }
            )
        ));
        telemetry
            .flush_async_typed()
            .await
            .expect("nothing was admitted");
        telemetry
            .shutdown_async_typed()
            .await
            .expect("empty shutdown");
        assert_eq!(telemetry.health().dropped_exports_total, 0);
        assert_eq!(
            listener.accept().expect_err("no collector contact").kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
}

#[cfg(feature = "sync-http")]
#[test]
fn public_sync_http_factory_reports_stalled_collector_timeout() {
    if !public_factory_scenario_child("public_sync_http_factory_reports_stalled_collector_timeout")
    {
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("stalled collector");
    let address = listener.local_addr().expect("collector address");
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let collector = std::thread::spawn(move || {
        let mut stream = accept_sync_http_export(&listener).expect("accept public factory request");
        let request = read_http_request(&mut stream);
        assert_eq!(
            decoded_sync_http_log(&request)["body"]["stringValue"],
            "stalled collector"
        );
        observed_tx.send(()).expect("request observed");
        let _ = release_rx.recv_timeout(Duration::from_secs(10));
        drop(stream);
    });
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.backend = sc_observability_otlp::v2::ExporterBackend::SyncHttp;
    transport.protocol = sc_observability_otlp::v2::OtlpProtocol::HttpJson;
    transport.endpoint =
        Some(V2OtlpEndpoint::new_typed(format!("http://{address}")).expect("endpoint"));
    transport.timeout_ms = Some(500_u64.into());
    let retry = transport
        .sync_http_retry
        .get_or_insert_with(Default::default);
    retry.max_retries = Some(10);
    retry.initial_backoff_ms = Some(1_u64.into());
    retry.max_backoff_ms = Some(1_u64.into());
    retry.retry_after_cap_ms = Some(100_u64.into());
    retry.retry_jitter_percent = Some(0);
    retry.retry_sequence_timeout_ms = Some(500_u64.into());
    let config = V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("stalled collector config");
    let telemetry = V2Telemetry::new_typed(config).expect("public sync-http factory");
    telemetry
        .emit_log(&log_event(service_name(), "stalled collector"))
        .expect("admit stalled export");
    let operation = std::thread::spawn(move || {
        let result = telemetry.flush_typed();
        (telemetry, result)
    });
    observed_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("actual request reached stalled collector");
    let (telemetry, result) = operation
        .join()
        .expect("flush operation exits under child watchdog");
    let _ = release_tx.send(());
    collector.join().expect("collector reaped");
    let error = result.expect_err("request exhausted retry deadline");
    assert!(matches!(
        error.export_cause(),
        Some(sc_observability_types::v2::ExportError::RetryDeadlineExhausted { .. })
    ));
    assert_eq!(telemetry.health().state, TelemetryHealthState::Degraded);
    assert_eq!(telemetry.health().dropped_exports_total, 1);
    telemetry.shutdown_typed().expect("shutdown after timeout");
}
