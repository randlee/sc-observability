//! Direct public canonical ingress through both production backends.
//!
//! `v2::Telemetry::emit_span` and `emit_metric` receive canonical models and
//! deliver them through the default SDK backend (OTLP/HTTP protobuf) and the
//! synchronous HTTP JSON backend to a caller-owned loopback collector. Each test
//! decodes the bytes that crossed the socket and checks every canonical span
//! field and histogram bucket.
#![cfg(all(feature = "otlp-sdk", feature = "sync-http"))]

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
            let head = format!("HTTP/1.1 {status} Scripted\r\nContent-Length: 0\r\n\r\n");
            if stream.write_all(head.as_bytes()).is_err() || stream.flush().is_err() {
                return;
            }
        }
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

use http_collector::{Captured, Collector};
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value::Value as AnyValue;
use opentelemetry_proto::tonic::metrics::v1::metric::Data;
use prost::Message;
use sc_observability_otlp::v2::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry,
    TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, Attributes, FiniteF64, HistogramPoint, MetricRecord,
    MetricValue, SpanEvent, SpanKind, SpanLink, SpanRecord, SpanSignal, SpanStarted, SpanStatus,
    TraceContext, TraceFlags,
};
use sc_observability_types::{
    ActionName, DurationMs, MetricName, ServiceName, SpanId, TelemetryHealthState, Timestamp,
    TraceId,
};
use serde_json::Value;

const SERVICE: &str = "canonical-ingress";
const TRACE_ID: &str = "0123456789abcdef0123456789abcdef";
const SPAN_ID: &str = "0123456789abcdef";
const PARENT_ID: &str = "1111111111111111";
const LINK_TRACE_ID: &str = "fedcba9876543210fedcba9876543210";
const LINK_SPAN_ID: &str = "fedcba9876543210";
const SPAN_NAME: &str = "canonical.span";
const EVENT_NAME: &str = "canonical.event";
const FLAGS: u8 = 0x01;
const LINK_FLAGS: u8 = 0x03;
const START_NANOS: u64 = 1_000_000_000;
const DURATION_MS: u64 = 25;
const PROTOBUF: &str = "application/x-protobuf";
const JSON: &str = "application/json";

/// Expected histogram: name, explicit bounds, bucket counts, count, sum.
///
/// The canonical model always carries one more bucket than bounds, so the
/// cases cover zero, one and many explicit bounds plus an empty distribution.
type ExpectedHistogram = (&'static str, Vec<f64>, Vec<u64>, u64, f64);

fn expected_histograms() -> Vec<ExpectedHistogram> {
    vec![
        ("histogram.empty", vec![], vec![0], 0, 0.0),
        ("histogram.zero_bounds", vec![], vec![3], 3, 4.5),
        ("histogram.one_bound", vec![5.0], vec![1, 2], 3, 20.0),
        (
            "histogram.many_bounds",
            vec![1.0, 10.0, 100.0],
            vec![1, 2, 3, 4],
            10,
            555.0,
        ),
    ]
}

fn service_name() -> ServiceName {
    ServiceName::new(SERVICE).expect("valid service")
}

fn timestamp(seconds: i64) -> Timestamp {
    let text = format!("\"1970-01-01T00:00:{seconds:02}Z\"");
    serde_json::from_str(&text).expect("valid timestamp")
}

fn trace() -> TraceContext {
    TraceContext::new(
        TraceId::new(TRACE_ID).expect("valid trace id"),
        SpanId::new(SPAN_ID).expect("valid span id"),
        TraceFlags::new(FLAGS),
    )
    .with_parent(SpanId::new(PARENT_ID).expect("valid parent id"))
}

fn span_signals() -> [SpanSignal; 3] {
    let link = SpanLink::new(
        TraceId::new(LINK_TRACE_ID).expect("valid linked trace"),
        SpanId::new(LINK_SPAN_ID).expect("valid linked span"),
        TraceFlags::new(LINK_FLAGS),
        Attributes::from([(
            "link.reason".to_owned(),
            AttributeValue::String("follows".to_owned()),
        )]),
    );
    let started = SpanRecord::<SpanStarted>::new(
        timestamp(1),
        service_name(),
        ActionName::new(SPAN_NAME).expect("valid span name"),
        trace(),
        Attributes::from([("span.attr".to_owned(), AttributeValue::Int(7))]),
    )
    .with_kind(SpanKind::Client)
    .with_links(vec![link]);
    let event = SpanEvent {
        timestamp: timestamp(1),
        trace: trace(),
        name: ActionName::new(EVENT_NAME).expect("valid event"),
        attributes: Attributes::new(),
        diagnostic: None,
    };
    let ended = started
        .clone()
        .end(SpanStatus::Error, DurationMs::from(DURATION_MS));
    [
        SpanSignal::Started(started),
        SpanSignal::Event(event),
        SpanSignal::Ended(ended),
    ]
}

fn histogram_metrics() -> Vec<MetricRecord> {
    expected_histograms()
        .into_iter()
        .map(|(name, bounds, counts, count, sum)| {
            let point = HistogramPoint::try_new(
                bounds
                    .into_iter()
                    .map(|bound| FiniteF64::new(bound).expect("finite bound"))
                    .collect(),
                counts,
                count,
                FiniteF64::new(sum).expect("finite sum"),
            )
            .expect("valid histogram point");
            MetricRecord::try_new(
                timestamp(2),
                service_name(),
                MetricName::new(name).expect("valid metric"),
                MetricValue::Histogram {
                    point,
                    temporality: AggregationTemporality::Delta,
                    start_time: timestamp(1),
                },
            )
            .expect("valid histogram metric")
        })
        .collect()
}

fn config(transport: OtelConfig) -> sc_observability_otlp::v2::TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid canonical telemetry config")
}

fn transport(endpoint: &str, backend: ExporterBackend, protocol: OtlpProtocol) -> OtelConfig {
    let mut transport = OtelConfig::default();
    transport.enabled = true;
    transport.backend = backend;
    transport.protocol = protocol;
    transport.endpoint = Some(OtlpEndpoint::new_typed(endpoint).expect("valid endpoint"));
    transport
}

fn emit_canonical(telemetry: &Telemetry) {
    for signal in span_signals() {
        telemetry.emit_span(&signal).expect("admit canonical span");
    }
    for metric in histogram_metrics() {
        telemetry
            .emit_metric(&metric)
            .expect("admit canonical histogram");
    }
}

fn assert_healthy(telemetry: &Telemetry) {
    let health = telemetry.health();
    assert_eq!(health.state, TelemetryHealthState::Healthy, "{health:?}");
    assert_eq!(health.dropped_exports_total, 0, "{health:?}");
    assert!(health.last_error.is_none(), "{health:?}");
}

fn single_body<'a>(captured: &'a [Captured], path: &str, content_type: &str) -> &'a [u8] {
    let matching: Vec<_> = captured
        .iter()
        .filter(|request| request.path == path)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exactly one POST to {path}: {captured:?}"
    );
    assert_eq!(matching[0].method, "POST");
    assert_eq!(matching[0].content_type.as_deref(), Some(content_type));
    assert_eq!(matching[0].authorization, None);
    &matching[0].body
}

fn request_bodies<'a>(captured: &'a [Captured], path: &str, content_type: &str) -> Vec<&'a [u8]> {
    captured
        .iter()
        .filter(|request| request.path == path)
        .map(|request| {
            assert_eq!(request.method, "POST");
            assert_eq!(request.content_type.as_deref(), Some(content_type));
            assert_eq!(request.authorization, None);
            request.body.as_slice()
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

#[test]
fn canonical_ingress_exports_every_field_over_the_sdk_backend() {
    let collector = Collector::start(&[]);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime")
        .block_on(async {
            let telemetry = Telemetry::new_typed(config(transport(
                &collector.endpoint(),
                ExporterBackend::OpenTelemetrySdk,
                OtlpProtocol::HttpBinary,
            )))
            .expect("SDK telemetry");
            emit_canonical(&telemetry);
            telemetry.flush_async().await.expect("SDK export completes");
            assert_healthy(&telemetry);
            telemetry
                .shutdown_async()
                .await
                .expect("shutdown completes");
        });
    let captured = collector.finish();

    let traces = ExportTraceServiceRequest::decode(single_body(&captured, "/v1/traces", PROTOBUF))
        .expect("traces body decodes");
    let spans: Vec<_> = traces
        .resource_spans
        .iter()
        .flat_map(|resource| &resource.scope_spans)
        .flat_map(|scope| &scope.spans)
        .collect();
    assert_eq!(spans.len(), 1);
    let span = spans[0];
    assert_eq!(span.name, SPAN_NAME);
    assert_eq!(hex(&span.trace_id), TRACE_ID);
    assert_eq!(hex(&span.span_id), SPAN_ID);
    assert_eq!(hex(&span.parent_span_id), PARENT_ID);
    assert_eq!(span.flags & 0xff, u32::from(FLAGS));
    assert_eq!(span.kind, 3, "SPAN_KIND_CLIENT");
    assert_eq!(span.start_time_unix_nano, START_NANOS);
    assert_eq!(
        span.end_time_unix_nano,
        START_NANOS + DURATION_MS * 1_000_000
    );
    assert_eq!(span.status.as_ref().map(|status| status.code), Some(2));
    assert_eq!(span.events.len(), 1);
    assert_eq!(span.events[0].name, EVENT_NAME);
    assert_eq!(span.links.len(), 1);
    let link = &span.links[0];
    assert_eq!(hex(&link.trace_id), LINK_TRACE_ID);
    assert_eq!(hex(&link.span_id), LINK_SPAN_ID);
    assert_eq!(link.flags & 0xff, u32::from(LINK_FLAGS));
    assert_eq!(link.attributes.len(), 1);
    assert_eq!(link.attributes[0].key, "link.reason");
    assert_eq!(
        link.attributes[0]
            .value
            .as_ref()
            .and_then(|value| value.value.clone()),
        Some(AnyValue::StringValue("follows".to_owned()))
    );

    let metric_requests: Vec<_> = request_bodies(&captured, "/v1/metrics", PROTOBUF)
        .into_iter()
        .map(|body| ExportMetricsServiceRequest::decode(body).expect("metrics body decodes"))
        .collect();
    assert_eq!(
        metric_requests.len(),
        2,
        "metric admission then flush exports"
    );
    let decoded: Vec<_> = metric_requests
        .iter()
        .flat_map(|request| &request.resource_metrics)
        .flat_map(|resource| &resource.scope_metrics)
        .flat_map(|scope| &scope.metrics)
        .collect();
    assert_eq!(decoded.len(), expected_histograms().len());
    for (name, bounds, counts, count, sum) in expected_histograms() {
        let metric = decoded
            .iter()
            .find(|metric| metric.name == name)
            .unwrap_or_else(|| panic!("{name} exported: {decoded:?}"));
        let Some(Data::Histogram(histogram)) = &metric.data else {
            panic!("{name} is a histogram: {metric:?}");
        };
        assert_eq!(histogram.aggregation_temporality, 1, "DELTA");
        assert_eq!(histogram.data_points.len(), 1);
        let point = &histogram.data_points[0];
        assert_eq!(point.explicit_bounds, bounds, "{name}");
        assert_eq!(point.bucket_counts, counts, "{name}");
        assert_eq!(point.count, count, "{name}");
        assert_eq!(point.sum, Some(sum), "{name}");
        assert_eq!(point.start_time_unix_nano, START_NANOS, "{name}");
        assert_eq!(point.time_unix_nano, 2 * START_NANOS, "{name}");
    }
}

#[test]
fn canonical_ingress_exports_every_field_over_the_sync_http_backend() {
    let collector = Collector::start(&[]);
    let telemetry = Telemetry::new_typed(config(transport(
        &collector.endpoint(),
        ExporterBackend::SyncHttp,
        OtlpProtocol::HttpJson,
    )))
    .expect("sync-http telemetry");
    emit_canonical(&telemetry);
    telemetry.flush().expect("sync-http export completes");
    assert_healthy(&telemetry);
    telemetry.shutdown().expect("shutdown completes");
    let captured = collector.finish();

    let traces: Value =
        serde_json::from_slice(single_body(&captured, "/v1/traces", JSON)).expect("traces JSON");
    let spans = &traces["resourceSpans"][0]["scopeSpans"][0]["spans"];
    assert_eq!(spans.as_array().map(Vec::len), Some(1), "{traces}");
    let span = &spans[0];
    assert_eq!(span["name"], SPAN_NAME);
    assert_eq!(span["traceId"], TRACE_ID);
    assert_eq!(span["spanId"], SPAN_ID);
    assert_eq!(span["parentSpanId"], PARENT_ID);
    assert_eq!(span["flags"], u32::from(FLAGS));
    assert_eq!(span["kind"], 3, "SPAN_KIND_CLIENT");
    assert_eq!(span["startTimeUnixNano"], START_NANOS.to_string());
    assert_eq!(
        span["endTimeUnixNano"],
        (START_NANOS + DURATION_MS * 1_000_000).to_string()
    );
    assert_eq!(span["status"]["code"], "STATUS_CODE_ERROR");
    assert_eq!(span["events"][0]["name"], EVENT_NAME);
    assert_eq!(span["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(span["links"].as_array().map(Vec::len), Some(1));
    let link = &span["links"][0];
    assert_eq!(link["traceId"], LINK_TRACE_ID);
    assert_eq!(link["spanId"], LINK_SPAN_ID);
    assert_eq!(link["flags"], u32::from(LINK_FLAGS));
    assert_eq!(link["attributes"][0]["key"], "link.reason");
    assert_eq!(link["attributes"][0]["value"]["stringValue"], "follows");

    let metric_requests: Vec<Value> = request_bodies(&captured, "/v1/metrics", JSON)
        .into_iter()
        .map(|body| serde_json::from_slice(body).expect("metrics JSON"))
        .collect();
    assert_eq!(
        metric_requests.len(),
        2,
        "metric admission then flush exports"
    );
    let decoded: Vec<_> = metric_requests
        .iter()
        .flat_map(|request| {
            request["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
                .as_array()
                .expect("metrics array")
        })
        .collect();
    assert_eq!(
        decoded.len(),
        expected_histograms().len(),
        "{metric_requests:?}"
    );
    for (name, bounds, counts, count, sum) in expected_histograms() {
        let metric = decoded
            .iter()
            .find(|metric| metric["name"] == name)
            .unwrap_or_else(|| panic!("{name} exported: {metric_requests:?}"));
        let histogram = &metric["histogram"];
        assert_eq!(histogram["aggregationTemporality"], 1, "DELTA");
        let point = &histogram["dataPoints"][0];
        assert_eq!(point["explicitBounds"], serde_json::json!(bounds), "{name}");
        assert_eq!(point["bucketCounts"], serde_json::json!(counts), "{name}");
        assert_eq!(point["count"], count, "{name}");
        assert_eq!(point["sum"], sum, "{name}");
        assert_eq!(
            point["startTimeUnixNano"],
            START_NANOS.to_string(),
            "{name}"
        );
        assert_eq!(
            point["timeUnixNano"],
            (2 * START_NANOS).to_string(),
            "{name}"
        );
    }
}
