//! External workspace composition: real bridge/core → test-local forwarder → observe → OTLP.
//! Collectors use real sockets; the forwarding adapter is deliberately test-local.
#![allow(
    deprecated,
    reason = "the released consumers intentionally use the retained root facade and its legacy error enum"
)]

mod http_collector {
    //! Caller-owned loopback HTTP/1.1 collector for the synchronous HTTP/JSON backend.

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
    /// Per-connection read bound so an idle pooled connection cannot stall shutdown.
    const CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(3);

    /// One HTTP request captured by the collector.
    #[derive(Debug, Clone)]
    pub struct Captured {
        pub method: String,
        pub path: String,
        pub content_type: Option<String>,
        pub body: Vec<u8>,
    }

    /// Loopback OTLP/HTTP collector owned by the test.
    #[derive(Debug)]
    pub struct Collector {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<Captured>>>,
        handle: Option<JoinHandle<()>>,
    }

    impl Collector {
        /// Starts a collector answering every POST with `200` and the given body.
        pub fn start(content_type: &'static str, response_body: &'static [u8]) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
            listener
                .set_nonblocking(true)
                .expect("non-blocking collector listener");
            let address = listener.local_addr().expect("collector address");
            let stop = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let handle = {
                let stop = Arc::clone(&stop);
                let requests = Arc::clone(&requests);
                thread::spawn(move || {
                    accept_loop(&listener, &stop, &requests, content_type, response_body);
                })
            };
            Self {
                address,
                stop,
                requests,
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
            self.requests
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

    fn accept_loop(
        listener: &TcpListener,
        stop: &AtomicBool,
        requests: &Arc<Mutex<Vec<Captured>>>,
        content_type: &'static str,
        response_body: &'static [u8],
    ) {
        let deadline = Instant::now() + COLLECTOR_WATCHDOG;
        let mut connections: Vec<JoinHandle<()>> = Vec::new();
        while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((stream, _)) => {
                    let requests = Arc::clone(requests);
                    connections.push(thread::spawn(move || {
                        serve_connection(stream, &requests, content_type, response_body);
                    }));
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

    fn serve_connection(
        mut stream: TcpStream,
        requests: &Mutex<Vec<Captured>>,
        content_type: &str,
        response_body: &[u8],
    ) {
        if stream.set_nonblocking(false).is_err()
            || stream
                .set_read_timeout(Some(CONNECTION_READ_TIMEOUT))
                .is_err()
            || stream
                .set_write_timeout(Some(CONNECTION_READ_TIMEOUT))
                .is_err()
        {
            return;
        }
        let Some(captured) = read_request(&mut stream) else {
            return;
        };
        // Record before answering so a completed export implies a recorded request.
        requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(captured);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(response_body);
        let _ = stream.flush();
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
        let mut content_length = 0_usize;
        let mut chunked = false;
        for line in lines {
            let (name, value) = line.split_once(':')?;
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "content-type" => content_type = Some(value.to_owned()),
                "content-length" => content_length = value.parse().ok()?,
                "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
                _ => {}
            }
        }
        if chunked {
            while !chunked_body_complete(&body) {
                let read = stream.read(&mut chunk).ok()?;
                if read == 0 {
                    return None;
                }
                body.extend_from_slice(&chunk[..read]);
            }
            body = decode_chunked(&body)?;
        } else {
            while body.len() < content_length {
                let read = stream.read(&mut chunk).ok()?;
                if read == 0 {
                    return None;
                }
                body.extend_from_slice(&chunk[..read]);
            }
            body.truncate(content_length);
        }
        Some(Captured {
            method,
            path,
            content_type,
            body,
        })
    }

    fn chunked_body_complete(raw: &[u8]) -> bool {
        decode_chunked(raw).is_some()
    }

    fn decode_chunked(mut raw: &[u8]) -> Option<Vec<u8>> {
        let mut decoded = Vec::new();
        loop {
            let line_end = find(raw, b"\r\n")?;
            let size_text = std::str::from_utf8(&raw[..line_end]).ok()?;
            let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
            raw = &raw[line_end + 2..];
            if size == 0 {
                return Some(decoded);
            }
            if raw.len() < size + 2 {
                return None;
            }
            decoded.extend_from_slice(&raw[..size]);
            raw = &raw[size + 2..];
        }
    }
}

mod grpc_collector {
    //! Caller-owned loopback OTLP/gRPC collector for the SDK backend.
    //!
    //! These consumers explicitly select gRPC. The collector uses the generated
    //! `LogsService` from the package's own `opentelemetry-proto`; this does not
    //! assert anything about the separate SDK HTTP-binary protocol.

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

    use opentelemetry_proto::tonic::collector::metrics::v1::{
        ExportMetricsServiceRequest, ExportMetricsServiceResponse,
        metrics_service_server::{MetricsService, MetricsServiceServer},
    };
    use opentelemetry_proto::tonic::collector::trace::v1::{
        ExportTraceServiceRequest, ExportTraceServiceResponse,
        trace_service_server::{TraceService, TraceServiceServer},
    };

    #[derive(Debug, Clone, Default)]
    struct Recorder(Arc<Mutex<Exports>>);

    /// Actual decoded exports, grouped by signal service.
    #[derive(Debug, Clone, Default)]
    pub struct Exports {
        pub logs: Vec<Received>,
        pub traces: Vec<ExportTraceServiceRequest>,
        pub metrics: Vec<ExportMetricsServiceRequest>,
    }

    #[tonic::async_trait]
    impl TraceService for Recorder {
        async fn export(
            &self,
            request: Request<ExportTraceServiceRequest>,
        ) -> Result<Response<ExportTraceServiceResponse>, Status> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .traces
                .push(request.into_inner());
            Ok(Response::new(ExportTraceServiceResponse {
                partial_success: None,
            }))
        }
    }
    #[tonic::async_trait]
    impl MetricsService for Recorder {
        async fn export(
            &self,
            request: Request<ExportMetricsServiceRequest>,
        ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .metrics
                .push(request.into_inner());
            Ok(Response::new(ExportMetricsServiceResponse {
                partial_success: None,
            }))
        }
    }

    /// Dispatch three generated OTLP services without enabling tonic's optional
    /// router dependency (the workspace intentionally does not depend on it).
    #[derive(Clone)]
    struct Services(Recorder);
    impl tonic::codegen::Service<tonic::codegen::http::Request<tonic::body::Body>> for Services {
        type Response = tonic::codegen::http::Response<tonic::body::Body>;
        type Error = std::convert::Infallible;
        type Future = tonic::codegen::BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn call(
            &mut self,
            request: tonic::codegen::http::Request<tonic::body::Body>,
        ) -> Self::Future {
            match request.uri().path() {
                "/opentelemetry.proto.collector.trace.v1.TraceService/Export" => {
                    TraceServiceServer::new(self.0.clone()).call(request)
                }
                "/opentelemetry.proto.collector.metrics.v1.MetricsService/Export" => {
                    MetricsServiceServer::new(self.0.clone()).call(request)
                }
                _ => LogsServiceServer::new(self.0.clone()).call(request),
            }
        }
    }

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
                .logs
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
        received: Arc<Mutex<Exports>>,
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
                        Services(recorder),
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
        pub fn finish(mut self) -> Exports {
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

mod support {
    //! Temp-root guard, projector and health assertion shared by every consumer.

    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    use sc_observability_types::v2::{LogProjector, ProjectionError};
    use sc_observability_types::{
        ActionName, Level, LogEvent, Observation, OutcomeLabel, ProcessIdentity, SchemaVersion,
        ServiceName, TargetCategory, Timestamp,
    };
    use serde_json::Map;

    /// Temp directory removed on drop without ever panicking.
    #[derive(Debug)]
    pub struct TempRoot(PathBuf);

    impl TempRoot {
        pub fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            Self(std::env::temp_dir().join(format!(
                "compatible-stack-{name}-{}-{nanos}",
                std::process::id()
            )))
        }

        pub fn path(&self) -> &Path {
            &self.0
        }

        pub fn log_file(&self, service: &str) -> PathBuf {
            self.0
                .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
                .join(format!(
                    "{service}{}",
                    sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
                ))
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub const SERVICE: &str = "compatible-stack";

    pub fn service_name() -> ServiceName {
        ServiceName::new(SERVICE).expect("valid service")
    }

    pub fn log_event(message: &str) -> LogEvent {
        LogEvent {
            version: SchemaVersion::new(
                sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
            )
            .expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: TargetCategory::new("compatible.stack").expect("valid target"),
            action: ActionName::new("stack.observe").expect("valid action"),
            message: Some(message.to_owned()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
            diagnostic: None,
            state_transition: None,
            fields: Map::new(),
        }
    }

    /// Projector usable by both the released and the canonical registration.
    #[derive(Debug)]
    pub struct MessageProjector;

    impl LogProjector<LogEvent> for MessageProjector {
        fn project_logs(
            &self,
            observation: &Observation<LogEvent>,
        ) -> Result<Vec<LogEvent>, ProjectionError> {
            Ok(vec![observation.payload.clone()])
        }
    }

    /// A downstream projector returning a caller-owned diagnostic and source.
    pub struct FailingProjector(
        std::sync::Mutex<Option<Box<sc_observability_types::ErrorContext>>>,
    );

    impl LogProjector<LogEvent> for FailingProjector {
        fn project_logs(
            &self,
            _: &Observation<LogEvent>,
        ) -> Result<Vec<LogEvent>, ProjectionError> {
            Err(ProjectionError::Projection {
                context: self
                    .0
                    .lock()
                    .expect("failure fixture")
                    .take()
                    .expect("one invocation"),
            })
        }
    }

    pub fn assert_failure_identity(
        run: impl FnOnce(std::sync::Arc<FailingProjector>) -> Box<sc_observability_types::ErrorContext>,
    ) {
        use sc_observability_types::{ErrorContext, Remediation};
        use std::error::Error;
        let context = Box::new(
            ErrorContext::new(
                sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE,
                "composition downstream projector failed",
                Remediation::not_recoverable("repair downstream projection"),
            )
            .detail("composition", serde_json::json!("caller-owned"))
            .source(Box::new(std::io::Error::other("composition source cause"))),
        );
        let context_ptr = std::ptr::from_ref(context.as_ref());
        let source_ptr = std::ptr::from_ref(
            context
                .source()
                .expect("source")
                .downcast_ref::<std::io::Error>()
                .expect("typed cause"),
        );
        let diagnostic = context.diagnostic().clone();
        let actual = run(std::sync::Arc::new(FailingProjector(
            std::sync::Mutex::new(Some(context)),
        )));
        assert_eq!(
            std::ptr::from_ref(actual.as_ref()),
            context_ptr,
            "wrapper must move original context"
        );
        assert_eq!(actual.diagnostic(), &diagnostic);
        assert_eq!(
            std::ptr::from_ref(
                actual
                    .source()
                    .expect("preserved source")
                    .downcast_ref::<std::io::Error>()
                    .expect("same source type")
            ),
            source_ptr
        );
    }

    /// Asserts a recorded health report shows one healthy, lossless log exporter.
    pub fn assert_healthy(report: &sc_observability_types::TelemetryHealthReport) {
        assert_eq!(
            report.state,
            sc_observability_types::TelemetryHealthState::Healthy,
            "{report:?}"
        );
        assert_eq!(report.dropped_exports_total, 0, "{report:?}");
        assert!(report.last_error.is_none(), "{report:?}");
        assert!(
            report.exporter_statuses.iter().any(|status| {
                status.state == sc_observability_types::ExporterHealthState::Healthy
            }),
            "{report:?}"
        );
    }
}

mod sdk_backend {
    //! Released `Grpc` and v2 `ExporterBackend::OpenTelemetrySdk` delivery.
    //!
    //! Both consumers select OTLP/gRPC, so the delivery evidence is a decoded
    //! `ExportLogsServiceRequest` received by a real tonic collector.

    use std::sync::Arc;

    use sc_observability_otlp::v2::{
        ExporterBackend, OtelConfig as V2OtelConfig, OtlpEndpoint as V2OtlpEndpoint,
        OtlpProtocol as V2OtlpProtocol, Telemetry as V2Telemetry,
        TelemetryConfigBuilder as V2TelemetryConfigBuilder, TelemetryError as V2TelemetryError,
        TelemetryProjectors as V2TelemetryProjectors,
    };
    use sc_observability_otlp::{
        LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry, TelemetryConfigBuilder,
        TelemetryProjectors,
    };
    use sc_observability_types::{TelemetryError, ToolName};
    use sc_observe::v2::{Observability, ObservabilityConfig};

    use super::grpc_collector::{GrpcCollector, Received};
    use super::support::{SERVICE, TempRoot, assert_healthy, log_event};

    fn caller_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("caller-owned runtime")
    }

    fn assert_sdk_delivery(received: &[Received], message: &str) {
        use opentelemetry_proto::tonic::common::v1::any_value::Value;

        assert!(!received.is_empty(), "collector captured no gRPC export");
        for export in received {
            assert!(
                export
                    .content_type
                    .as_deref()
                    .is_some_and(|value| value.starts_with("application/grpc")),
                "SDK backend must speak gRPC, got {:?}",
                export.content_type
            );
        }
        let resource_logs: Vec<_> = received
            .iter()
            .flat_map(|export| export.request.resource_logs.iter())
            .collect();
        let has_service = resource_logs.iter().any(|resource| {
            resource.resource.as_ref().is_some_and(|resource| {
                resource.attributes.iter().any(|attribute| {
                    attribute.key == "service.name"
                        && matches!(
                            attribute.value.as_ref().and_then(|value| value.value.as_ref()),
                            Some(Value::StringValue(service)) if service == SERVICE
                        )
                })
            })
        });
        assert!(has_service, "service.name resource attribute not delivered");
        assert_eq!(
            resource_logs
                .iter()
                .flat_map(|r| r.scope_logs.iter())
                .flat_map(|s| s.log_records.iter())
                .count(),
            1,
            "exactly the admitted bridge record reaches the collector; policy-negative must not export"
        );
        let has_message = resource_logs
            .iter()
            .flat_map(|resource| resource.scope_logs.iter())
            .flat_map(|scope| scope.log_records.iter())
            .any(|record| {
                matches!(
                    record.body.as_ref().and_then(|body| body.value.as_ref()),
                    Some(Value::StringValue(body)) if body == message
                )
            });
        assert!(
            has_message,
            "log body {message:?} not delivered: {resource_logs:?}"
        );
    }

    #[test]
    fn released_root_consumer_delivers_all_signals_through_sdk_backend() {
        const MESSAGE: &str = "released-sdk-delivery-message";
        if super::bridge::isolated(concat!(
            module_path!(),
            "::released_root_consumer_delivers_all_signals_through_sdk_backend"
        )) {
            return;
        }
        let collector = GrpcCollector::start();
        let root = TempRoot::new("released-sdk");
        let runtime = caller_runtime();

        runtime.block_on(async {
            // Released protocol -> backend mapping: Grpc selects the SDK.
            let transport = OtelConfig {
                enabled: true,
                endpoint: Some(
                    OtlpEndpoint::new_typed(collector.endpoint()).expect("collector endpoint"),
                ),
                protocol: OtlpProtocol::Grpc,
                ..OtelConfig::default()
            };
            let config = TelemetryConfigBuilder::new(super::support::service_name())
                .enable_logs(LogsConfig::default())
                .enable_traces(sc_observability_otlp::TracesConfig::default())
                .enable_metrics(sc_observability_otlp::MetricsConfig::default())
                .with_transport(transport)
                .build_typed()
                .expect("released config");
            let telemetry = Arc::new(Telemetry::new_typed(config).expect("released SDK telemetry"));
            super::support::assert_failure_identity(|projector| {
                let registration = TelemetryProjectors::new(&telemetry)
                    .with_log_projector(projector)
                    .into_registration();
                let (projector, _, _, _) = registration.into_parts();
                let error = projector
                    .expect("registered projector")
                    .project_logs(&sc_observability_types::Observation::new(
                        super::support::service_name(),
                        log_event("rejected-projector"),
                    ))
                    .expect_err("projector cause must propagate");
                let sc_observability_types::v2::ProjectionError::Projection { context } = error
                else {
                    panic!("expected a projection failure");
                };
                context
            });

            let observe_config = ObservabilityConfig::default_for(
                ToolName::new(SERVICE).expect("tool"),
                root.path().to_path_buf(),
            )
            .expect("observe config");
            let observability = Arc::new(
                Observability::builder(observe_config)
                    .with_observability_health_provider(telemetry.clone())
                    .register_projection(
                        TelemetryProjectors::new(&telemetry)
                            .with_log_projector(Arc::new(super::support::MessageProjector))
                            .with_span_projector(Arc::new(super::signals::ReleasedProjector))
                            .with_metric_projector(Arc::new(super::signals::ReleasedProjector))
                            .into_registration(),
                    )
                    .build()
                    .expect("observability"),
            );

            super::bridge::deliver(MESSAGE, observability.clone(), root.path(), true);
            telemetry
                .flush_async_typed()
                .await
                .expect("SDK lifecycle flush delivers to the collector");
            assert_healthy(&telemetry.health());
            assert_healthy(
                &observability
                    .health()
                    .telemetry
                    .expect("attached telemetry health"),
            );
            observability.flush().expect("observe flush");

            telemetry
                .shutdown_async_typed()
                .await
                .expect("SDK lifecycle shutdown");
            assert!(matches!(
                telemetry.emit_log(&log_event("after-shutdown")),
                Err(TelemetryError::Shutdown)
            ));
            observability.shutdown().expect("observe shutdown");
            super::bridge::reject_closed(observability.clone(), root.path());
        });

        let exports = collector.finish();
        assert_sdk_delivery(&exports.logs, MESSAGE);
        super::signals::assert_grpc(&exports, MESSAGE, true);
        let local = std::fs::read_to_string(root.log_file(SERVICE)).expect("local log file");
        assert!(local.contains(MESSAGE), "sc-observe also logged locally");
    }

    #[test]
    fn v2_consumer_delivers_all_signals_through_sdk_backend() {
        const MESSAGE: &str = "v2-sdk-delivery-message";
        if super::bridge::isolated(concat!(
            module_path!(),
            "::v2_consumer_delivers_all_signals_through_sdk_backend"
        )) {
            return;
        }
        let collector = GrpcCollector::start();
        let root = TempRoot::new("v2-sdk");
        let runtime = caller_runtime();

        runtime.block_on(async {
            // v2 selects the backend explicitly together with its protocol.
            let mut transport =
                V2OtelConfig::new(ExporterBackend::OpenTelemetrySdk, V2OtlpProtocol::Grpc);
            transport.enabled = true;
            transport.endpoint =
                Some(V2OtlpEndpoint::new_typed(collector.endpoint()).expect("collector endpoint"));
            let config = V2TelemetryConfigBuilder::new(super::support::service_name())
                .enable_logs(LogsConfig::default())
                .enable_traces(sc_observability_otlp::TracesConfig::default())
                .enable_metrics(sc_observability_otlp::MetricsConfig::default())
                .with_transport(transport)
                .build_typed()
                .expect("v2 config");
            let telemetry = Arc::new(V2Telemetry::new_typed(config).expect("v2 SDK telemetry"));
            super::support::assert_failure_identity(|projector| {
                let registration = V2TelemetryProjectors::new(telemetry.clone())
                    .with_log_projector(projector)
                    .into_registration();
                let (projector, _, _, _) = registration.into_parts();
                let error = projector
                    .expect("registered projector")
                    .project_logs(&sc_observability_types::Observation::new(
                        super::support::service_name(),
                        log_event("rejected-projector"),
                    ))
                    .expect_err("projector cause must propagate");
                error.into_context()
            });

            let observe_config = sc_observe::v2::ObservabilityConfig::default_for(
                ToolName::new(SERVICE).expect("tool"),
                root.path().to_path_buf(),
            )
            .expect("observe config");
            let observability = Arc::new(
                sc_observe::v2::Observability::builder(observe_config)
                    .with_observability_health_provider(telemetry.clone())
                    .register_projection(
                        V2TelemetryProjectors::new(telemetry.clone())
                            .with_log_projector(Arc::new(super::support::MessageProjector))
                            .with_span_projector(Arc::new(super::signals::Projector))
                            .with_metric_projector(Arc::new(super::signals::Projector))
                            .into_registration(),
                    )
                    .build()
                    .expect("observability"),
            );

            super::bridge::deliver(MESSAGE, observability.clone(), root.path(), false);
            // The shared lifecycle barrier is async-only inside the entered runtime.
            assert!(telemetry.flush().is_err());
            telemetry
                .flush_async()
                .await
                .expect("SDK lifecycle flush delivers to the collector");
            assert_healthy(&telemetry.health());
            assert_healthy(
                &observability
                    .health()
                    .telemetry
                    .expect("attached telemetry health"),
            );
            observability.flush().expect("observe flush");

            telemetry
                .shutdown_async()
                .await
                .expect("SDK lifecycle shutdown");
            assert!(matches!(
                telemetry.emit_log(&log_event("after-shutdown")),
                Err(V2TelemetryError::Shutdown { .. })
            ));
            observability.shutdown().expect("observe shutdown");
            super::bridge::reject_closed(observability.clone(), root.path());
        });

        let exports = collector.finish();
        assert_sdk_delivery(&exports.logs, MESSAGE);
        super::signals::assert_grpc(&exports, MESSAGE, false);
        let local = std::fs::read_to_string(root.log_file(SERVICE)).expect("local log file");
        assert!(local.contains(MESSAGE), "sc-observe also logged locally");
    }
}

mod sync_http_backend {
    //! Released `HttpJson` and v2 `ExporterBackend::SyncHttp` delivery.

    use std::sync::Arc;

    use sc_observability_otlp::v2::{
        ExporterBackend, OtelConfig as V2OtelConfig, OtlpEndpoint as V2OtlpEndpoint,
        OtlpProtocol as V2OtlpProtocol, Telemetry as V2Telemetry,
        TelemetryConfigBuilder as V2TelemetryConfigBuilder, TelemetryError as V2TelemetryError,
        TelemetryProjectors as V2TelemetryProjectors,
    };
    use sc_observability_otlp::{
        LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry, TelemetryConfigBuilder,
        TelemetryProjectors,
    };
    use sc_observability_types::{TelemetryError, ToolName};
    use sc_observe::v2::{Observability, ObservabilityConfig};

    use super::http_collector::{Captured, Collector};
    use super::support::{SERVICE, TempRoot, assert_healthy, log_event};

    const JSON: &str = "application/json";

    fn assert_json_delivery(requests: &[Captured], message: &str) {
        assert!(!requests.is_empty(), "collector captured no request");
        for request in requests {
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, "/v1/logs", "only logs are emitted");
            assert!(
                request
                    .content_type
                    .as_deref()
                    .is_some_and(|value| value.starts_with(JSON)),
                "sync-http backend must send JSON, got {:?}",
                request.content_type
            );
        }
        assert_eq!(requests.len(), 1, "only the admitted bridge record exports");
        assert!(!String::from_utf8_lossy(&requests[0].body).contains("denied-must-not-export"));
        let delivered = requests.iter().any(|request| {
            let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
                return false;
            };
            payload.pointer("/resourceLogs/0/scopeLogs/0/logRecords/0/body/stringValue")
                == Some(&serde_json::Value::String(message.to_owned()))
                && payload.pointer("/resourceLogs/0/resource/attributes/0/value/stringValue")
                    == Some(&serde_json::Value::String(SERVICE.to_owned()))
        });
        assert!(
            delivered,
            "JSON log record with message {message:?} and service {SERVICE:?} not captured: {:?}",
            requests
                .iter()
                .map(|request| String::from_utf8_lossy(&request.body).into_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn released_root_consumer_delivers_all_signals_through_sync_http_json_backend() {
        const MESSAGE: &str = "released-sync-http-delivery-message";
        if super::bridge::isolated(concat!(
            module_path!(),
            "::released_root_consumer_delivers_all_signals_through_sync_http_json_backend"
        )) {
            return;
        }
        let collector = Collector::start(JSON, b"{}");
        let root = TempRoot::new("released-sync-http");

        // Released protocol -> backend mapping: HttpJson selects synchronous HTTP/JSON.
        let transport = OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new_typed(collector.endpoint()).expect("collector endpoint"),
            ),
            protocol: OtlpProtocol::HttpJson,
            ..OtelConfig::default()
        };
        let config = TelemetryConfigBuilder::new(super::support::service_name())
            .enable_logs(LogsConfig::default())
            .enable_traces(sc_observability_otlp::TracesConfig::default())
            .enable_metrics(sc_observability_otlp::MetricsConfig::default())
            .with_transport(transport)
            .build_typed()
            .expect("released config");
        let telemetry =
            Arc::new(Telemetry::new_typed(config).expect("released sync-http telemetry"));
        super::support::assert_failure_identity(|projector| {
            let registration = TelemetryProjectors::new(&telemetry)
                .with_log_projector(projector)
                .into_registration();
            let (projector, _, _, _) = registration.into_parts();
            let error = projector
                .expect("registered projector")
                .project_logs(&sc_observability_types::Observation::new(
                    super::support::service_name(),
                    log_event("rejected-projector"),
                ))
                .expect_err("projector cause must propagate");
            let sc_observability_types::v2::ProjectionError::Projection { context } = error else {
                panic!("expected a projection failure");
            };
            context
        });

        let observe_config = ObservabilityConfig::default_for(
            ToolName::new(SERVICE).expect("tool"),
            root.path().to_path_buf(),
        )
        .expect("observe config");
        let observability = Arc::new(
            Observability::builder(observe_config)
                .with_observability_health_provider(telemetry.clone())
                .register_projection(
                    TelemetryProjectors::new(&telemetry)
                        .with_log_projector(Arc::new(super::support::MessageProjector))
                        .with_span_projector(Arc::new(super::signals::ReleasedProjector))
                        .with_metric_projector(Arc::new(super::signals::ReleasedProjector))
                        .into_registration(),
                )
                .build()
                .expect("observability"),
        );

        super::bridge::deliver(MESSAGE, observability.clone(), root.path(), true);
        telemetry
            .flush()
            .expect("sync-http flush delivers to the collector");
        assert_healthy(&telemetry.health());
        assert_healthy(
            &observability
                .health()
                .telemetry
                .expect("attached telemetry health"),
        );
        observability.flush().expect("observe flush");

        telemetry.shutdown().expect("sync-http shutdown");
        assert!(matches!(
            telemetry.emit_log(&log_event("after-shutdown")),
            Err(TelemetryError::Shutdown)
        ));
        assert!(telemetry.flush().is_err());
        observability.shutdown().expect("observe shutdown");
        super::bridge::reject_closed(observability.clone(), root.path());

        let requests = collector.finish();
        let logs: Vec<_> = requests
            .iter()
            .filter(|r| r.path == "/v1/logs")
            .cloned()
            .collect();
        assert_json_delivery(&logs, MESSAGE);
        super::signals::assert_http(&requests, MESSAGE, true);
        let local = std::fs::read_to_string(root.log_file(SERVICE)).expect("local log file");
        assert!(local.contains(MESSAGE), "sc-observe also logged locally");
    }

    #[test]
    fn v2_consumer_delivers_all_signals_through_sync_http_json_backend() {
        const MESSAGE: &str = "v2-sync-http-delivery-message";
        if super::bridge::isolated(concat!(
            module_path!(),
            "::v2_consumer_delivers_all_signals_through_sync_http_json_backend"
        )) {
            return;
        }
        let collector = Collector::start(JSON, b"{}");
        let root = TempRoot::new("v2-sync-http");

        let mut transport = V2OtelConfig::new(ExporterBackend::SyncHttp, V2OtlpProtocol::HttpJson);
        transport.enabled = true;
        transport.endpoint =
            Some(V2OtlpEndpoint::new_typed(collector.endpoint()).expect("collector endpoint"));
        let config = V2TelemetryConfigBuilder::new(super::support::service_name())
            .enable_logs(LogsConfig::default())
            .enable_traces(sc_observability_otlp::TracesConfig::default())
            .enable_metrics(sc_observability_otlp::MetricsConfig::default())
            .with_transport(transport)
            .build_typed()
            .expect("v2 config");
        let telemetry = Arc::new(V2Telemetry::new_typed(config).expect("v2 sync-http telemetry"));
        super::support::assert_failure_identity(|projector| {
            let registration = V2TelemetryProjectors::new(telemetry.clone())
                .with_log_projector(projector)
                .into_registration();
            let (projector, _, _, _) = registration.into_parts();
            let error = projector
                .expect("registered projector")
                .project_logs(&sc_observability_types::Observation::new(
                    super::support::service_name(),
                    log_event("rejected-projector"),
                ))
                .expect_err("projector cause must propagate");
            error.into_context()
        });

        let observe_config = sc_observe::v2::ObservabilityConfig::default_for(
            ToolName::new(SERVICE).expect("tool"),
            root.path().to_path_buf(),
        )
        .expect("observe config");
        let observability = Arc::new(
            sc_observe::v2::Observability::builder(observe_config)
                .with_observability_health_provider(telemetry.clone())
                .register_projection(
                    V2TelemetryProjectors::new(telemetry.clone())
                        .with_log_projector(Arc::new(super::support::MessageProjector))
                        .with_span_projector(Arc::new(super::signals::Projector))
                        .with_metric_projector(Arc::new(super::signals::Projector))
                        .into_registration(),
                )
                .build()
                .expect("observability"),
        );

        super::bridge::deliver(MESSAGE, observability.clone(), root.path(), false);
        telemetry
            .flush()
            .expect("sync-http flush delivers to the collector");
        assert_healthy(&telemetry.health());
        assert_healthy(
            &observability
                .health()
                .telemetry
                .expect("attached telemetry health"),
        );
        observability.flush().expect("observe flush");

        telemetry.shutdown().expect("sync-http shutdown");
        assert!(matches!(
            telemetry.emit_log(&log_event("after-shutdown")),
            Err(V2TelemetryError::Shutdown { .. })
        ));
        assert!(telemetry.flush().is_err());
        observability.shutdown().expect("observe shutdown");
        super::bridge::reject_closed(observability.clone(), root.path());

        let requests = collector.finish();
        let logs: Vec<_> = requests
            .iter()
            .filter(|r| r.path == "/v1/logs")
            .cloned()
            .collect();
        assert_json_delivery(&logs, MESSAGE);
        super::signals::assert_http(&requests, MESSAGE, false);
        let local = std::fs::read_to_string(root.log_file(SERVICE)).expect("local log file");
        assert!(local.contains(MESSAGE), "sc-observe also logged locally");
    }
}

mod bridge;
mod signals;
