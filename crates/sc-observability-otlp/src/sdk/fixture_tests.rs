//! Crate-local SDK fixture tests for canonical adapter projection and lifecycle.

use crate::sdk_backend::fixture::SdkFixture;
use crate::v2::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
    TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::error_codes::otlp::OTLP_EXPORT_TERMINAL;
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
use std::io;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEADLINE_TEST_SERVER_IO_TIMEOUT: Duration = Duration::from_secs(2);
const DEADLINE_TEST_SERVER_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const DEADLINE_TEST_UNWIND_MIN_WAIT: Duration = Duration::from_secs(1);
const DEADLINE_TEST_UNWIND_MAX_WAIT: Duration = Duration::from_secs(5);

struct DeadlineTestServer {
    release_tx: Option<mpsc::Sender<()>>,
    join_handle: Option<JoinHandle<io::Result<()>>>,
}

impl DeadlineTestServer {
    fn spawn(listener: TcpListener, accepted_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        let (release_tx, release_rx) = mpsc::channel();
        let join_handle = thread::spawn(move || {
            listener.set_nonblocking(true)?;
            let accept_deadline = Instant::now() + DEADLINE_TEST_SERVER_IO_TIMEOUT;
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            && Instant::now() < accept_deadline =>
                    {
                        thread::sleep(DEADLINE_TEST_SERVER_ACCEPT_POLL_INTERVAL);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "deadline test server accept timed out",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            };
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(DEADLINE_TEST_SERVER_IO_TIMEOUT))?;
            let mut request_prefix = [0_u8; 1];
            stream.read_exact(&mut request_prefix)?;
            accepted_tx.send(()).map_err(|()| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "deadline test dropped acceptance receiver",
                )
            })?;
            release_rx
                .recv_timeout(DEADLINE_TEST_SERVER_IO_TIMEOUT)
                .map_err(|error| {
                    let kind = match error {
                        mpsc::RecvTimeoutError::Timeout => io::ErrorKind::TimedOut,
                        mpsc::RecvTimeoutError::Disconnected => io::ErrorKind::BrokenPipe,
                    };
                    io::Error::new(kind, "deadline test server release was not received")
                })?;
            Ok(())
        });

        Self {
            release_tx: Some(release_tx),
            join_handle: Some(join_handle),
        }
    }

    fn finish(mut self) -> io::Result<()> {
        self.release();
        let join_handle = self
            .join_handle
            .take()
            .expect("deadline test server thread is present");
        join_handle
            .join()
            .map_err(|_| io::Error::other("deadline test server thread panicked"))?
    }

    fn release(&mut self) {
        if let Some(release_tx) = self.release_tx.take() {
            let _ = release_tx.send(());
        }
    }
}

impl Drop for DeadlineTestServer {
    fn drop(&mut self) {
        self.release();
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

#[test]
fn deadline_test_server_drop_joins_on_unwind_after_bounded_read() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind deadline listener");
    let address = listener.local_addr().expect("deadline listener address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let client = TcpStream::connect(address).expect("connect deadline client");
    let server = DeadlineTestServer::spawn(listener, accepted_tx);

    let started = Instant::now();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _server = server;
        panic!("simulate a failed deadline-fixture assertion");
    }));
    drop(client);
    drop(accepted_rx);

    assert!(unwind.is_err(), "the simulated assertion must unwind");
    assert!(
        started.elapsed() >= DEADLINE_TEST_UNWIND_MIN_WAIT,
        "unwinding should wait for the server's bounded read timeout"
    );
    assert!(
        started.elapsed() < DEADLINE_TEST_UNWIND_MAX_WAIT,
        "unwinding must join the read-blocked helper promptly"
    );
}

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

    TelemetryConfigBuilder::new(ServiceName::new("otlp-sdk-test").expect("service"))
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
                service: ServiceName::new("otlp-sdk-test").expect("service"),
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
                ServiceName::new("otlp-sdk-test").expect("service"),
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
            ServiceName::new("otlp-sdk-test").expect("service"),
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

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn external_fixture_drives_all_signals_and_host_lifecycle() {
    // This deliberately relies on the endpoint validator accepting a port above
    // 65535 and reqwest rejecting it synchronously during request construction.
    // Tightening validation must fail this fixture loudly at construction.
    let mut config = fixture_config_for_endpoint(8 * 1_024, "http://127.0.0.1:99999");
    config.transport.protocol = OtlpProtocol::HttpBinary;
    let fixture = SdkFixture::new(&config).expect("fixture adapter");
    let started = tokio::time::Instant::now();

    fixture.export_logs(&[log_record()]).expect("schedule log");
    fixture
        .export_spans(&[span_record()])
        .expect("schedule span");
    fixture
        .export_metrics(&[metric_record()])
        .expect("schedule metric");

    // Request construction supplies a deterministic terminal transport failure.
    // Flush reports that window through the caller-runtime admission
    // core; the following empty shutdown window must not repeat its failure.
    let flush = fixture
        .flush()
        .await
        .expect_err("ordered async flush reports the admitted RPC failure");
    assert_eq!(flush.diagnostic().code, OTLP_EXPORT_TERMINAL);
    assert_eq!(
        tokio::time::Instant::now(),
        started,
        "request construction fails without advancing any deadline"
    );
    fixture
        .shutdown()
        .await
        .expect("shutdown starts a clean window after flush reported the RPC failure");
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
async fn external_fixture_exercises_request_deadline_terminal_outcome() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind deadline listener");
    let address = listener.local_addr().expect("deadline listener address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = DeadlineTestServer::spawn(listener, accepted_tx);

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
    let flush_result = tokio::time::timeout(Duration::from_secs(1), fixture.flush()).await;

    // Release and join before inspecting the assertion result. The guard also
    // does this on unwinding paths, with bounded I/O in the helper thread.
    server
        .finish()
        .expect("deadline server should complete cleanly");
    let flush = flush_result
        .expect("request deadline must complete before the test bound")
        .expect_err("held request must preserve its typed terminal outcome");
    assert_eq!(flush.diagnostic().code, OTLP_EXPORT_TERMINAL);

    fixture
        .shutdown()
        .await
        .expect("shutdown starts a clean window after flush reported the request deadline");
}
