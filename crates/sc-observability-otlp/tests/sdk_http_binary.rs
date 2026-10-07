//! Public-factory regression for the SDK backend's `HttpBinary` protocol.
//!
//! The released root facade and the canonical `v2` facade both default to the
//! SDK backend with `OtlpProtocol::HttpBinary`. These tests construct each
//! default configuration inside a caller-entered Tokio runtime and deliver to a
//! caller-owned loopback OTLP/HTTP collector that decodes the protobuf bodies,
//! so every assertion is on bytes that crossed a socket. A tonic collector is
//! the `Grpc` control: the same public factory still speaks gRPC there.
#![cfg(feature = "otlp-sdk")]
#![allow(
    deprecated,
    reason = "the released consumer intentionally uses the retained root facade"
)]

mod http_collector {
    //! Caller-owned loopback HTTP/1.1 collector with scripted responses.

    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    /// Upper bound for the whole collector lifetime; a test that needs more has hung.
    const COLLECTOR_WATCHDOG: Duration = Duration::from_secs(20);
    /// Pause between non-blocking accept attempts. Delivery ordering never relies
    /// on it: the collector records a request before it answers, and the
    /// exporter under test only reports completion after the answer.
    const ACCEPT_POLL: Duration = Duration::from_millis(5);
    /// Per-connection I/O bound so an idle pooled connection cannot stall shutdown.
    const CONNECTION_IO_TIMEOUT: Duration = Duration::from_secs(3);

    /// One HTTP request captured by the collector.
    #[derive(Debug, Clone)]
    pub struct Captured {
        pub method: String,
        pub path: String,
        pub content_type: Option<String>,
        pub authorization: Option<String>,
        pub body: Vec<u8>,
    }

    /// One scripted answer: status code and the delay before answering.
    pub type Reply = (u16, Duration);

    #[derive(Debug, Default)]
    struct Shared {
        requests: Mutex<Vec<Captured>>,
        replies: Mutex<VecDeque<Reply>>,
    }

    /// Loopback OTLP/HTTP collector owned by the test.
    #[derive(Debug)]
    pub struct Collector {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        shared: Arc<Shared>,
        handle: Option<JoinHandle<()>>,
    }

    impl Collector {
        /// Starts a collector that answers requests with `replies` in arrival
        /// order and with an immediate `200` once they are used up.
        pub fn start(replies: &[Reply]) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
            listener
                .set_nonblocking(true)
                .expect("non-blocking collector listener");
            let address = listener.local_addr().expect("collector address");
            let stop = Arc::new(AtomicBool::new(false));
            let shared = Arc::new(Shared {
                requests: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.iter().copied().collect()),
            });
            let handle = {
                let stop = Arc::clone(&stop);
                let shared = Arc::clone(&shared);
                thread::spawn(move || accept_loop(&listener, &stop, &shared))
            };
            Self {
                address,
                stop,
                shared,
                handle: Some(handle),
            }
        }

        /// Base endpoint URL of the collector (no signal path).
        pub fn endpoint(&self) -> String {
            format!("http://{}", self.address)
        }

        /// Stops the collector, joins its threads and returns everything it captured.
        pub fn finish(mut self) -> Vec<Captured> {
            self.join();
            self.shared
                .requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn join(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                // A panicking collector thread is reported by the missing capture.
                let _ = handle.join();
            }
        }
    }

    impl Drop for Collector {
        fn drop(&mut self) {
            self.join();
        }
    }

    /// A collector with a causally ordered timeout followed by a successful retry.
    ///
    /// It intentionally serves its two connections on one thread: after reading
    /// the first request it withholds a response until the retry connection is
    /// accepted. This prevents a worker-thread scheduling race from turning the
    /// first response into an immediate connection failure or response.
    #[derive(Debug)]
    pub struct TimeoutRetryCollector {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<Vec<Captured>>>,
    }

    impl TimeoutRetryCollector {
        pub fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind timeout retry collector");
            let address = listener
                .local_addr()
                .expect("timeout retry collector address");
            let stop = Arc::new(AtomicBool::new(false));
            let handle = {
                let stop = Arc::clone(&stop);
                thread::spawn(move || serve_timeout_then_retry(&listener, &stop))
            };
            Self {
                address,
                stop,
                handle: Some(handle),
            }
        }

        pub fn endpoint(&self) -> String {
            format!("http://{}", self.address)
        }

        pub fn finish(mut self) -> Vec<Captured> {
            self.handle
                .take()
                .expect("timeout retry collector is running")
                .join()
                .expect("timeout retry collector completes")
        }

        fn stop(&mut self) {
            if self.handle.is_none() {
                return;
            }
            self.stop.store(true, Ordering::Release);
            // Wake the blocking retry accept if a test exits before a retry.
            let _ = TcpStream::connect(self.address);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl Drop for TimeoutRetryCollector {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn accept_loop(listener: &TcpListener, stop: &AtomicBool, shared: &Arc<Shared>) {
        let deadline = Instant::now() + COLLECTOR_WATCHDOG;
        let mut connections: Vec<JoinHandle<()>> = Vec::new();
        while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((stream, _)) => {
                    let shared = Arc::clone(shared);
                    connections.push(thread::spawn(move || serve_connection(stream, &shared)));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(ACCEPT_POLL);
                }
                Err(_) => break,
            }
        }
        for connection in connections {
            let _ = connection.join();
        }
    }

    fn serve_connection(mut stream: TcpStream, shared: &Shared) {
        if stream.set_nonblocking(false).is_err()
            || stream
                .set_read_timeout(Some(CONNECTION_IO_TIMEOUT))
                .is_err()
            || stream
                .set_write_timeout(Some(CONNECTION_IO_TIMEOUT))
                .is_err()
        {
            return;
        }
        // Serve every request on a kept-alive connection until the client closes it.
        while let Some(captured) = read_request(&mut stream) {
            // Record before answering so a completed export implies a recorded request.
            shared
                .requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(captured);
            let (status, delay) = shared
                .replies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front()
                .unwrap_or((200, Duration::ZERO));
            thread::sleep(delay);
            let head = format!(
                "HTTP/1.1 {status} Scripted\r\nContent-Type: application/x-protobuf\r\nContent-Length: 0\r\n\r\n"
            );
            if stream.write_all(head.as_bytes()).is_err() || stream.flush().is_err() {
                return;
            }
        }
    }

    fn serve_timeout_then_retry(listener: &TcpListener, stop: &AtomicBool) -> Vec<Captured> {
        let (mut first, _) = listener.accept().expect("accept first timeout request");
        configure_stream(&first);
        let first_request = read_request(&mut first).expect("read first timeout request");

        // The first request remains unanswered. Accepting this second connection
        // proves the client classified the first attempt as a timeout and retried.
        let (mut retry, _) = listener.accept().expect("accept retry request");
        if stop.load(Ordering::Acquire) {
            return Vec::new();
        }
        configure_stream(&retry);
        let retry_request = read_request(&mut retry).expect("read retry request");
        write_success(&mut retry);
        drop(first);
        vec![first_request, retry_request]
    }

    fn configure_stream(stream: &TcpStream) {
        stream
            .set_read_timeout(Some(CONNECTION_IO_TIMEOUT))
            .expect("set collector read timeout");
        stream
            .set_write_timeout(Some(CONNECTION_IO_TIMEOUT))
            .expect("set collector write timeout");
    }

    fn write_success(stream: &mut TcpStream) {
        stream
            .write_all(
                b"HTTP/1.1 200 Scripted\r\nContent-Type: application/x-protobuf\r\nContent-Length: 0\r\n\r\n",
            )
            .expect("write retry success");
        stream.flush().expect("flush retry success");
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn read_request(stream: &mut TcpStream) -> Option<Captured> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        let head_end = loop {
            if let Some(position) = find(&buffer, b"\r\n\r\n") {
                break position;
            }
            let read = stream.read(&mut chunk).ok()?;
            if read == 0 {
                return None;
            }
            buffer.extend_from_slice(&chunk[..read]);
        };
        let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
        let mut body = buffer[head_end + 4..].to_vec();
        let mut lines = head.split("\r\n");
        let mut request_line = lines.next()?.split_whitespace();
        let method = request_line.next()?.to_owned();
        let path = request_line.next()?.to_owned();
        let mut content_type = None;
        let mut authorization = None;
        let mut content_length = 0_usize;
        for line in lines {
            let (name, value) = line.split_once(':')?;
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "content-type" => content_type = Some(value.to_owned()),
                "authorization" => authorization = Some(value.to_owned()),
                "content-length" => content_length = value.parse().ok()?,
                _ => {}
            }
        }
        while body.len() < content_length {
            let read = stream.read(&mut chunk).ok()?;
            if read == 0 {
                return None;
            }
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(content_length);
        Some(Captured {
            method,
            path,
            content_type,
            authorization,
            body,
        })
    }
}

mod grpc_collector {
    //! Caller-owned loopback OTLP/gRPC logs collector for the `Grpc` control.

    use std::net::SocketAddr;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::{
        LogsService, LogsServiceServer,
    };
    use opentelemetry_proto::tonic::collector::logs::v1::{
        ExportLogsServiceRequest, ExportLogsServiceResponse,
    };
    use tonic::transport::Server;
    use tonic::transport::server::TcpIncoming;
    use tonic::{Request, Response, Status};

    /// Upper bound for the whole collector lifetime; a test that needs more has hung.
    const COLLECTOR_WATCHDOG: Duration = Duration::from_secs(20);
    /// Bound for the collector thread to report its bound address.
    const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

    /// One decoded gRPC export plus the `content-type` the client sent.
    #[derive(Debug, Clone)]
    pub struct Received {
        pub content_type: Option<String>,
        pub request: ExportLogsServiceRequest,
    }

    #[derive(Debug, Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<Received>>>);

    #[tonic::async_trait]
    impl LogsService for Recorder {
        async fn export(
            &self,
            request: Request<ExportLogsServiceRequest>,
        ) -> Result<Response<ExportLogsServiceResponse>, Status> {
            let content_type = request
                .metadata()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            // Record before answering so a completed export implies a recorded request.
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Received {
                    content_type,
                    request: request.into_inner(),
                });
            Ok(Response::new(ExportLogsServiceResponse {
                partial_success: None,
            }))
        }
    }

    /// Loopback OTLP/gRPC logs collector owned by the test.
    #[derive(Debug)]
    pub struct GrpcCollector {
        address: SocketAddr,
        received: Arc<Mutex<Vec<Received>>>,
        stop: Option<tokio::sync::oneshot::Sender<()>>,
        handle: Option<JoinHandle<()>>,
    }

    impl GrpcCollector {
        pub fn start() -> Self {
            let recorder = Recorder::default();
            let received = Arc::clone(&recorder.0);
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let (ready, address) = mpsc::channel();
            let handle = thread::spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let Ok(incoming) = "127.0.0.1:0"
                        .parse()
                        .map_err(drop)
                        .and_then(|addr| TcpIncoming::bind(addr).map_err(drop))
                    else {
                        return;
                    };
                    let Ok(bound) = incoming.local_addr() else {
                        return;
                    };
                    let server = tokio::spawn(Server::builder().serve_with_incoming_shutdown(
                        LogsServiceServer::new(recorder),
                        incoming,
                        std::future::pending::<()>(),
                    ));
                    if ready.send(bound).is_err() {
                        server.abort();
                        return;
                    }
                    // Stop on request, or when the watchdog elapses. Aborting the
                    // server drops every open connection immediately.
                    let _ = tokio::time::timeout(COLLECTOR_WATCHDOG, stopped).await;
                    server.abort();
                });
            });
            let address = address
                .recv_timeout(STARTUP_TIMEOUT)
                .expect("gRPC collector reports its bound address");
            Self {
                address,
                received,
                stop: Some(stop),
                handle: Some(handle),
            }
        }

        /// Base endpoint URL of the collector.
        pub fn endpoint(&self) -> String {
            format!("http://{}", self.address)
        }

        /// Stops the collector, joins its thread and returns every decoded export.
        pub fn finish(mut self) -> Vec<Received> {
            self.join();
            self.received
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn join(&mut self) {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl Drop for GrpcCollector {
        fn drop(&mut self) {
            self.join();
        }
    }
}

use std::time::Duration;

use http_collector::{Captured, Collector, TimeoutRetryCollector};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use sc_observability_otlp::v2::{
    AuthHeader as V2AuthHeader, OtelConfig as V2OtelConfig, OtlpEndpoint as V2OtlpEndpoint,
    OtlpProtocol as V2OtlpProtocol, Telemetry as V2Telemetry,
    TelemetryConfigBuilder as V2TelemetryConfigBuilder,
};
use sc_observability_otlp::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry,
    TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::v2::{AggregationTemporality, FiniteF64, MetricRecord, MetricValue};
use sc_observability_types::{
    ActionName, DurationMs, Level, LogEvent, MetricName, ProcessIdentity, SchemaVersion,
    ServiceName, SpanId, SpanRecord, SpanSignal, SpanStarted, SpanStatus, TargetCategory,
    TelemetryHealthReport, TelemetryHealthState, Timestamp, TraceContext, TraceId,
};

const SERVICE: &str = "sdk-http-binary";
const LOG_MESSAGE: &str = "http-binary-log";
const SPAN_NAME: &str = "http.binary.span";
const METRIC_NAME: &str = "http.binary.metric";
const PROTOBUF: &str = "application/x-protobuf";

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime")
}

fn service_name() -> ServiceName {
    ServiceName::new(SERVICE).expect("valid service")
}

fn log_event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("sdk.http").expect("valid target"),
        action: ActionName::new("sdk.emit").expect("valid action"),
        message: Some(LOG_MESSAGE.to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn span_signals() -> [SpanSignal; 2] {
    let trace = TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    };
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new(SPAN_NAME).expect("valid span name"),
        trace,
        serde_json::Map::new(),
    );
    let ended = started.clone().end(SpanStatus::Ok, DurationMs::from(10));
    [SpanSignal::Started(started), SpanSignal::Ended(ended)]
}

fn metric() -> MetricRecord {
    MetricRecord::try_new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        MetricName::new(METRIC_NAME).expect("valid metric"),
        MetricValue::Sum {
            value: FiniteF64::new(1.0).expect("finite counter"),
            monotonic: true,
            temporality: AggregationTemporality::Cumulative,
            start_time: Timestamp::UNIX_EPOCH,
        },
    )
    .expect("valid counter")
}

fn canonical_span_signals() -> [sc_observability_types::v2::SpanSignal; 2] {
    use sc_observability_types::v2;
    let trace = v2::TraceContext::new(
        TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        SpanId::new("0123456789abcdef").expect("valid span id"),
        v2::TraceFlags::default(),
    );
    let started = v2::SpanRecord::<v2::SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new(SPAN_NAME).expect("valid span name"),
        trace,
        v2::Attributes::new(),
    );
    let ended = started
        .clone()
        .end(v2::SpanStatus::Ok, DurationMs::from(10));
    [
        v2::SpanSignal::Started(started),
        v2::SpanSignal::Ended(ended),
    ]
}

fn released_config(transport: OtelConfig) -> sc_observability_otlp::TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid released telemetry config")
}

fn v2_default_transport(endpoint: &str) -> V2OtelConfig {
    let mut transport = V2OtelConfig::default();
    transport.enabled = true;
    transport.endpoint = Some(V2OtlpEndpoint::new_typed(endpoint).expect("valid endpoint"));
    transport
}

fn v2_telemetry(transport: V2OtelConfig) -> V2Telemetry {
    let config = V2TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid v2 telemetry config");
    V2Telemetry::new_typed(config).expect("v2 SDK telemetry is constructed on its caller runtime")
}

fn requests_to<'a>(captured: &'a [Captured], path: &str) -> Vec<&'a Captured> {
    captured
        .iter()
        .filter(|request| request.path == path)
        .collect()
}

fn single_request<'a>(captured: &'a [Captured], path: &str) -> &'a Captured {
    let matching = requests_to(captured, path);
    assert_eq!(
        matching.len(),
        1,
        "exactly one POST to {path}: {captured:?}"
    );
    let request = matching[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.content_type.as_deref(), Some(PROTOBUF));
    request
}

/// Decodes all three signals from protobuf bodies and checks the exported content.
fn assert_lossless_three_signals(captured: &[Captured]) {
    let logs =
        ExportLogsServiceRequest::decode(single_request(captured, "/v1/logs").body.as_slice())
            .expect("logs body is an ExportLogsServiceRequest");
    let log_records: Vec<_> = logs
        .resource_logs
        .iter()
        .flat_map(|resource| &resource.scope_logs)
        .flat_map(|scope| &scope.log_records)
        .collect();
    assert_eq!(log_records.len(), 1);
    assert!(format!("{:?}", log_records[0].body).contains(LOG_MESSAGE));

    let traces =
        ExportTraceServiceRequest::decode(single_request(captured, "/v1/traces").body.as_slice())
            .expect("traces body is an ExportTraceServiceRequest");
    let spans: Vec<_> = traces
        .resource_spans
        .iter()
        .flat_map(|resource| &resource.scope_spans)
        .flat_map(|scope| &scope.spans)
        .collect();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SPAN_NAME);

    let metrics = ExportMetricsServiceRequest::decode(
        single_request(captured, "/v1/metrics").body.as_slice(),
    )
    .expect("metrics body is an ExportMetricsServiceRequest");
    let names: Vec<_> = metrics
        .resource_metrics
        .iter()
        .flat_map(|resource| &resource.scope_metrics)
        .flat_map(|scope| &scope.metrics)
        .map(|metric| metric.name.as_str())
        .collect();
    assert_eq!(names, [METRIC_NAME]);
}

fn assert_healthy(health: &TelemetryHealthReport) {
    assert_eq!(health.state, TelemetryHealthState::Healthy, "{health:?}");
    assert_eq!(health.dropped_exports_total, 0, "{health:?}");
    assert!(health.last_error.is_none(), "{health:?}");
}

#[test]
fn released_default_http_binary_exports_lossless_protobuf_for_every_signal() {
    assert_eq!(OtelConfig::default().protocol, OtlpProtocol::HttpBinary);
    let collector = Collector::start(&[]);
    runtime().block_on(async {
        let telemetry = Telemetry::new_typed(released_config(OtelConfig {
            enabled: true,
            endpoint: Some(OtlpEndpoint::new_typed(collector.endpoint()).expect("endpoint")),
            ..OtelConfig::default()
        }))
        .expect("released SDK telemetry");
        telemetry.emit_log(&log_event()).expect("admit log");
        for signal in span_signals() {
            telemetry.emit_span(&signal).expect("admit span");
        }
        telemetry.emit_metric(&metric()).expect("admit metric");

        telemetry
            .flush_async_typed()
            .await
            .expect("HttpBinary export completes");
        assert_healthy(&telemetry.health());
        telemetry
            .shutdown_async_typed()
            .await
            .expect("shutdown completes");
    });
    assert_lossless_three_signals(&collector.finish());
}

#[test]
fn v2_default_http_binary_exports_lossless_protobuf_for_every_signal() {
    assert_eq!(V2OtelConfig::default().protocol, V2OtlpProtocol::HttpBinary);
    let collector = Collector::start(&[]);
    runtime().block_on(async {
        let telemetry = v2_telemetry(v2_default_transport(&collector.endpoint()));
        telemetry.emit_log(&log_event()).expect("admit log");
        for signal in canonical_span_signals() {
            telemetry.emit_span(&signal).expect("admit span");
        }
        telemetry.emit_metric(&metric()).expect("admit metric");

        telemetry
            .flush_async()
            .await
            .expect("HttpBinary export completes");
        assert_healthy(&telemetry.health());
        telemetry
            .shutdown_async()
            .await
            .expect("shutdown completes");
    });
    assert_lossless_three_signals(&collector.finish());
}

#[test]
fn http_binary_sends_the_configured_authorization_header() {
    let collector = Collector::start(&[]);
    runtime().block_on(async {
        let mut transport = v2_default_transport(&collector.endpoint());
        transport.auth_header = Some(V2AuthHeader::new_typed("Bearer sdk-token").expect("header"));
        let telemetry = v2_telemetry(transport);
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry.flush_async().await.expect("export completes");
        assert_healthy(&telemetry.health());
    });
    let captured = collector.finish();
    assert_eq!(
        single_request(&captured, "/v1/logs")
            .authorization
            .as_deref(),
        Some("Bearer sdk-token")
    );
}

#[test]
fn http_binary_endpoint_with_signal_path_is_not_doubled() {
    let collector = Collector::start(&[]);
    runtime().block_on(async {
        let endpoint = format!("{}/v1/logs/", collector.endpoint());
        let telemetry = v2_telemetry(v2_default_transport(&endpoint));
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry.flush_async().await.expect("export completes");
    });
    single_request(&collector.finish(), "/v1/logs");
}

#[test]
fn http_binary_retries_throttling_then_succeeds() {
    let collector = Collector::start(&[(429, Duration::ZERO), (503, Duration::ZERO)]);
    runtime().block_on(async {
        let telemetry = v2_telemetry(v2_default_transport(&collector.endpoint()));
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry
            .flush_async()
            .await
            .expect("retryable statuses are retried to success");
        assert_healthy(&telemetry.health());
    });
    assert_eq!(requests_to(&collector.finish(), "/v1/logs").len(), 3);
}

#[test]
fn http_binary_client_error_is_terminal_without_retry() {
    let collector = Collector::start(&[(400, Duration::ZERO)]);
    runtime().block_on(async {
        let telemetry = v2_telemetry(v2_default_transport(&collector.endpoint()));
        telemetry.emit_log(&log_event()).expect("admit log");
        assert!(telemetry.flush_async().await.is_err());
        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Degraded);
        assert_eq!(health.dropped_exports_total, 1);
        assert_eq!(
            health
                .last_error
                .as_ref()
                .and_then(|error| error.code.as_ref())
                .map(sc_observability_types::ErrorCode::as_str),
            Some("OTLP_EXPORT_TERMINAL")
        );
    });
    assert_eq!(requests_to(&collector.finish(), "/v1/logs").len(), 1);
}

#[test]
fn http_binary_retryable_response_is_retried_without_timeout_race() {
    // A retryable status deterministically exercises the retry path. The next
    // request receives the collector's default immediate success response, so
    // neither attempt depends on a short wall-clock timeout.
    let collector = Collector::start(&[(503, Duration::ZERO)]);
    runtime().block_on(async {
        let telemetry = v2_telemetry(v2_default_transport(&collector.endpoint()));
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry
            .flush_async()
            .await
            .expect("retryable response is retried to success");
        assert_healthy(&telemetry.health());
    });
    assert_eq!(requests_to(&collector.finish(), "/v1/logs").len(), 2);
}

#[test]
fn http_binary_request_timeout_is_retried_within_the_deadline() {
    let collector = TimeoutRetryCollector::start();
    runtime().block_on(async {
        let mut transport = v2_default_transport(&collector.endpoint());
        transport.timeout_ms = Some(DurationMs::from(100));
        let telemetry = v2_telemetry(transport);
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry
            .flush_async()
            .await
            .expect("timed-out request is retried to success");
        assert_healthy(&telemetry.health());
    });

    let requests = collector.finish();
    assert_eq!(requests_to(&requests, "/v1/logs").len(), 2);
}

#[test]
fn grpc_protocol_still_exports_over_grpc() {
    let collector = grpc_collector::GrpcCollector::start();
    runtime().block_on(async {
        let mut transport = v2_default_transport(&collector.endpoint());
        transport.protocol = V2OtlpProtocol::Grpc;
        let telemetry = v2_telemetry(transport);
        telemetry.emit_log(&log_event()).expect("admit log");
        telemetry
            .flush_async()
            .await
            .expect("gRPC export completes");
        assert_healthy(&telemetry.health());
    });
    let received = collector.finish();
    assert_eq!(received.len(), 1);
    assert!(
        received[0]
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.starts_with("application/grpc"))
    );
    assert!(format!("{:?}", received[0].request).contains(LOG_MESSAGE));
}
