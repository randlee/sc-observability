//! External Tokio-hosted exercise of the D.7 SDK fixture seam.
//!
//! This consumer lives outside `sc-observability-otlp`. It proves the
//! non-default fixture can drive real signal projection, bounded admission,
//! ordered asynchronous completion, and host-runtime teardown without
//! activating D.18's production `Telemetry` factory.

#![cfg(feature = "sdk-fixture")]

use sc_observability_otlp::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, SdkFixture,
    TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::otlp::{
    OtlpCompleteSpan, OtlpInstrumentationScope, OtlpLogRecord, OtlpRecord, OtlpResource,
};
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, Attributes, FiniteF64, HistogramPoint, MetricRecord,
    MetricValue, SpanEvent, SpanKind, SpanRecord, TraceContext as V2TraceContext, TraceFlags,
};
use sc_observability_types::{
    ActionName, DurationMs, Level, LogEvent, MetricName, ProcessIdentity, SchemaVersion,
    ServiceName, SpanId, SpanStatus, TargetCategory, Timestamp, TraceContext as LogTraceContext,
    TraceId,
};
use std::io::Read;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn fixture_config(queue_byte_capacity: usize) -> TelemetryConfig {
    fixture_config_for_endpoint(queue_byte_capacity, "http://127.0.0.1:9")
}

fn fixture_config_for_endpoint(queue_byte_capacity: usize, endpoint: &str) -> TelemetryConfig {
    let mut transport = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    transport.enabled = true;
    transport.endpoint =
        Some(OtlpEndpoint::new_typed(endpoint.to_owned()).expect("fixture endpoint"));
    transport.queue_capacity = Some(8);
    transport.queue_byte_capacity = Some(queue_byte_capacity);
    transport.timeout_ms = Some(DurationMs::from(10));
    transport.lifecycle_flush_timeout_ms = Some(DurationMs::from(100));
    transport.lifecycle_shutdown_timeout_ms = Some(DurationMs::from(100));

    TelemetryConfigBuilder::new(ServiceName::new("otlp-sdk-fixture").expect("service"))
        .with_transport(transport)
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .build_typed()
        .expect("valid fixture config")
}

fn resource() -> OtlpResource {
    OtlpResource {
        attributes: Attributes::from([(
            "service.instance.id".to_owned(),
            AttributeValue::String("fixture-1".to_owned()),
        )]),
        schema_url: Some("https://example.test/resource".to_owned()),
    }
}

fn scope() -> OtlpInstrumentationScope {
    OtlpInstrumentationScope {
        name: "external-fixture".to_owned(),
        version: Some("2.0".to_owned()),
        schema_url: Some("https://example.test/scope".to_owned()),
        attributes: Attributes::default(),
    }
}

fn log_trace_context() -> LogTraceContext {
    LogTraceContext {
        trace_id: TraceId::new("1234567890abcdef1234567890abcdef").expect("trace id"),
        span_id: SpanId::new("1234567890abcdef").expect("span id"),
        parent_span_id: None,
    }
}

fn span_trace_context() -> V2TraceContext {
    V2TraceContext::new(
        TraceId::new("1234567890abcdef1234567890abcdef").expect("trace id"),
        SpanId::new("1234567890abcdef").expect("span id"),
        TraceFlags::new(0x01),
    )
}

fn log_record() -> OtlpRecord<OtlpLogRecord> {
    OtlpRecord {
        resource: resource(),
        scope: scope(),
        record: OtlpLogRecord {
            event: LogEvent {
                version: SchemaVersion::new("v1").expect("schema version"),
                timestamp: Timestamp::UNIX_EPOCH,
                level: Level::Info,
                service: ServiceName::new("otlp-sdk-fixture").expect("service"),
                target: TargetCategory::new("fixture.sdk").expect("target"),
                action: ActionName::new("fixture.export").expect("action"),
                message: Some("real external SDK fixture".to_owned()),
                identity: ProcessIdentity::default(),
                trace: Some(log_trace_context()),
                request_id: None,
                correlation_id: None,
                outcome: None,
                diagnostic: None,
                state_transition: None,
                fields: serde_json::Map::new(),
            },
            trace_flags: TraceFlags::new(0x01),
            attributes: Attributes::from([(
                "fixture.kind".to_owned(),
                AttributeValue::String("log".to_owned()),
            )]),
        },
    }
}

fn span_record() -> OtlpRecord<OtlpCompleteSpan> {
    let trace = span_trace_context();
    OtlpRecord {
        resource: resource(),
        scope: scope(),
        record: OtlpCompleteSpan {
            record: SpanRecord::new(
                Timestamp::UNIX_EPOCH,
                ServiceName::new("otlp-sdk-fixture").expect("service"),
                ActionName::new("fixture.span").expect("action"),
                trace.clone(),
                Attributes::default(),
            )
            .with_kind(SpanKind::Server)
            .end(SpanStatus::Ok, DurationMs::from(1)),
            events: vec![SpanEvent {
                timestamp: Timestamp::UNIX_EPOCH,
                trace,
                name: ActionName::new("fixture.event").expect("action"),
                attributes: Attributes::default(),
                diagnostic: None,
            }],
        },
    }
}

fn metric_record() -> OtlpRecord<MetricRecord> {
    OtlpRecord {
        resource: resource(),
        scope: scope(),
        record: MetricRecord::try_new(
            Timestamp::UNIX_EPOCH,
            ServiceName::new("otlp-sdk-fixture").expect("service"),
            MetricName::new("fixture.duration").expect("metric name"),
            MetricValue::Histogram {
                point: HistogramPoint::try_new(
                    vec![FiniteF64::new(1.0).expect("finite bound")],
                    vec![1, 2],
                    3,
                    FiniteF64::new(4.0).expect("finite sum"),
                )
                .expect("histogram"),
                temporality: AggregationTemporality::Cumulative,
                start_time: Timestamp::UNIX_EPOCH,
            },
        )
        .expect("metric"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn external_fixture_drives_all_signals_and_host_lifecycle() {
    let fixture = SdkFixture::new(&fixture_config(8 * 1_024)).expect("fixture adapter");

    fixture.export_logs(&[log_record()]).expect("schedule log");
    fixture
        .export_spans(&[span_record()])
        .expect("schedule span");
    fixture
        .export_metrics(&[metric_record()])
        .expect("schedule metric");

    // The closed loopback endpoint supplies a deterministic terminal transport
    // result. Flush/shutdown still wait through the same caller-runtime core.
    fixture.flush().await.expect("ordered async flush");
    fixture.shutdown().await.expect("host runtime teardown");
}

#[tokio::test(flavor = "current_thread")]
async fn external_fixture_enforces_record_and_byte_pressure() {
    let fixture = SdkFixture::new(&fixture_config(1_024)).expect("fixture adapter");

    let error = fixture
        .export_logs(&[log_record(), log_record()])
        .expect_err("two fixture records exceed the one-record byte budget");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::QueueFull { .. }
    ));
    fixture
        .shutdown()
        .await
        .expect("teardown after pressure refusal");
}

#[tokio::test(flavor = "current_thread")]
async fn external_fixture_exercises_request_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind deadline listener");
    let address = listener.local_addr().expect("deadline listener address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deadline request");
        let mut request_prefix = [0_u8; 1];
        stream
            .read_exact(&mut request_prefix)
            .expect("read the started deadline request");
        accepted_tx.send(()).expect("signal request acceptance");
        release_rx.recv().expect("release held deadline request");
    });

    let endpoint = format!("http://{address}");
    let fixture = SdkFixture::new(&fixture_config_for_endpoint(8 * 1_024, &endpoint))
        .expect("fixture adapter");
    fixture
        .export_logs(&[log_record()])
        .expect("schedule deadline request");
    tokio::time::timeout(Duration::from_secs(1), accepted_rx)
        .await
        .expect("SDK did not open the deadline request in time")
        .expect("deadline server dropped its acceptance signal");
    tokio::task::yield_now().await;

    let started = Instant::now();
    tokio::time::timeout(Duration::from_secs(1), fixture.flush())
        .await
        .expect("request deadline must complete before the test bound")
        .expect("held request must complete through the configured deadline");
    assert!(
        started.elapsed() >= Duration::from_millis(5),
        "flush completed before the configured 10ms request deadline"
    );
    release_tx.send(()).expect("release deadline request");
    fixture
        .shutdown()
        .await
        .expect("shutdown after the bounded request deadline");
    server.join().expect("join deadline server");
}
