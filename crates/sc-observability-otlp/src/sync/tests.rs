//! Native construction, export, validation and hardening tests for the
//! synchronous client against loopback OTLP/HTTP and TLS collectors.

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use opentelemetry::logs::{AnyValue, LogRecord as _, Severity};
use opentelemetry::trace::{
    Event, Link, SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
};
use opentelemetry::{InstrumentationScope, KeyValue};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::KeyValue as ProtoKeyValue;
use opentelemetry_proto::tonic::common::v1::any_value::Value as ProtoValue;
use opentelemetry_proto::tonic::metrics::v1::metric::Data;
use opentelemetry_proto::tonic::metrics::v1::number_data_point::Value as NumberValue;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{SpanData, SpanEvents, SpanLinks};
use prost::Message;

use super::{Client, SyncError, check_input_limits};
use crate::constants::{MAX_BATCH_RECORDS, MAX_INPUT_BYTES};
use crate::error_codes::sync as codes;

/// Upper bound for any fixture wait so a broken client cannot hang the suite.
const FIXTURE_WATCHDOG: Duration = Duration::from_secs(10);
/// Client timeout used by the stalled-endpoint tests.
const STALL_TIMEOUT: Duration = Duration::from_millis(250);
/// Bounds the child-process precedence proof while its collector remains held.
const PRECEDENCE_CHILD_WATCHDOG: Duration = Duration::from_secs(5);
const SERVICE: &str = "sync-client-test";
const ACCEPT_POLL: Duration = Duration::from_millis(5);

enum Reply {
    Ok,
    Status(u16, &'static str),
    Stall(StalledRequest),
}

#[derive(Debug, Clone)]
struct Captured {
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Loopback OTLP/HTTP collector that records each request and answers it.
struct Collector {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<Captured>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

/// Test-side controls for a collector which holds a received request open.
struct StalledCollector {
    request_arrived: Receiver<()>,
    release: Sender<()>,
}

/// Server-side half of [`StalledCollector`].
struct StalledRequest {
    request_arrived: Sender<()>,
    release: Receiver<()>,
}

impl Collector {
    fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
        listener
            .set_nonblocking(true)
            .expect("nonblocking collector listener");
        let address = listener.local_addr().expect("collector address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            thread::spawn(move || serve(&listener, &reply, &requests, &stop, None))
        };
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn start_stalled() -> (Self, StalledCollector) {
        let (request_arrived_tx, request_arrived) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let collector = Self::start(Reply::Stall(StalledRequest {
            request_arrived: request_arrived_tx,
            release: release_rx,
        }));
        (
            collector,
            StalledCollector {
                request_arrived,
                release,
            },
        )
    }

    fn endpoint(&self) -> String {
        format!("http://{}", self.address)
    }

    fn requests(&self) -> Vec<Captured> {
        self.requests.lock().expect("collector requests").clone()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("join loopback collector");
        }
    }
}

type TlsConfig = Arc<rustls::ServerConfig>;

fn serve(
    listener: &TcpListener,
    reply: &Reply,
    requests: &Mutex<Vec<Captured>>,
    stop: &AtomicBool,
    tls: Option<&TlsConfig>,
) {
    let deadline = Instant::now() + FIXTURE_WATCHDOG * 3;
    while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted stream");
                stream
                    .set_read_timeout(Some(FIXTURE_WATCHDOG))
                    .expect("stream read timeout");
                match tls {
                    None => handle(stream, reply, requests),
                    Some(config) => {
                        let connection = rustls::ServerConnection::new(Arc::clone(config))
                            .expect("TLS server connection");
                        handle(
                            rustls::StreamOwned::new(connection, stream),
                            reply,
                            requests,
                        );
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL);
            }
            Err(error) => panic!("collector accept failed: {error}"),
        }
    }
}

fn handle<S: Read + Write>(mut stream: S, reply: &Reply, requests: &Mutex<Vec<Captured>>) {
    let Some(request) = read_request(&mut stream) else {
        // Failed TLS handshakes and dropped connections have no request.
        return;
    };
    requests.lock().expect("collector requests").push(request);
    match reply {
        Reply::Ok => respond(&mut stream, 200, ""),
        Reply::Status(code, body) => respond(&mut stream, *code, body),
        Reply::Stall(stalled) => {
            if stalled.request_arrived.send(()).is_ok() {
                let _ = stalled.release.recv_timeout(FIXTURE_WATCHDOG);
            }
        }
    }
}

fn read_request<S: Read>(stream: &mut S) -> Option<Captured> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let path = lines.next()?.split(' ').nth(1)?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buffer[header_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Some(Captured {
        path,
        headers,
        body,
    })
}

fn respond<S: Write>(stream: &mut S, code: u16, body: &str) {
    let response = format!(
        "HTTP/1.1 {code} Test\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn resource() -> Resource {
    Resource::builder_empty()
        .with_service_name(SERVICE)
        .with_attribute(KeyValue::new("deployment.environment", "test"))
        .build()
}

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder("sync-client")
        .with_version("1.2.3")
        .build()
}

fn trace_id() -> TraceId {
    TraceId::from_hex("0af7651916cd43dd8448eb211c80319c").expect("trace id")
}

fn span_id() -> SpanId {
    SpanId::from_hex("b7ad6b7169203331").expect("span id")
}

fn completed_span() -> SpanData {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let linked = SpanContext::new(
        TraceId::from_hex("1af7651916cd43dd8448eb211c80319c").expect("linked trace"),
        SpanId::from_hex("c7ad6b7169203331").expect("linked span"),
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    );
    // The SDK span collections are non-exhaustive: build them from defaults.
    let mut events = SpanEvents::default();
    events.events.push(Event::new(
        "retry",
        start + Duration::from_millis(500),
        vec![KeyValue::new("attempt", 2_i64)],
        0,
    ));
    let mut links = SpanLinks::default();
    links.links.push(Link::with_context(linked));
    SpanData {
        span_context: SpanContext::new(
            trace_id(),
            span_id(),
            TraceFlags::SAMPLED,
            false,
            TraceState::default(),
        ),
        parent_span_id: SpanId::from_hex("a7ad6b7169203331").expect("parent span"),
        parent_span_is_remote: true,
        span_kind: SpanKind::Client,
        name: "upload".into(),
        start_time: start,
        end_time: start + Duration::from_millis(1_500),
        attributes: vec![KeyValue::new("http.request.method", "POST")],
        dropped_attributes_count: 0,
        events,
        links,
        status: Status::error("upload failed"),
        instrumentation_scope: scope(),
    }
}

fn string_attribute<'a>(attributes: &'a [ProtoKeyValue], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.key == key)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| match value.value.as_ref() {
            Some(ProtoValue::StringValue(text)) => Some(text.as_str()),
            _ => None,
        })
}

fn validation_code(result: Result<(), SyncError>) -> &'static str {
    match result {
        Err(SyncError::Validation { code, .. }) => code,
        other => panic!("expected a validation failure, got {other:?}"),
    }
}

fn unix_nanos(time: SystemTime) -> u64 {
    u64::try_from(
        time.duration_since(SystemTime::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos(),
    )
    .expect("nanoseconds fit in u64")
}

#[test]
fn send_log_exports_native_record_fields_and_headers() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint())
        .and_then(|client| client.with_header("x-tenant", "acme"))
        .expect("client");
    let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let observed = timestamp + Duration::from_secs(2);
    client
        .send_log(&resource(), scope(), |record| {
            record.set_timestamp(timestamp);
            record.set_observed_timestamp(observed);
            record.set_severity_number(Severity::Warn);
            record.set_severity_text("WARN");
            record.set_body(AnyValue::from("disk nearly full"));
            record.add_attribute("disk.free_bytes", 1_024_i64);
            record.set_trace_context(trace_id(), span_id(), Some(TraceFlags::SAMPLED));
            Ok(())
        })
        .expect("log exported");

    let requests = collector.requests();
    assert_eq!(requests.len(), 1, "one export request");
    let request = &requests[0];
    assert_eq!(request.path, "/v1/logs");
    assert_eq!(
        request.header("content-type"),
        Some("application/x-protobuf")
    );
    assert_eq!(request.header("x-tenant"), Some("acme"));
    let decoded = ExportLogsServiceRequest::decode(request.body.as_slice()).expect("logs proto");
    let resource_logs = &decoded.resource_logs[0];
    let resource = resource_logs.resource.as_ref().expect("resource");
    assert_eq!(
        string_attribute(&resource.attributes, "service.name"),
        Some(SERVICE)
    );
    assert_eq!(
        string_attribute(&resource.attributes, "deployment.environment"),
        Some("test")
    );
    let scope_logs = &resource_logs.scope_logs[0];
    let scope = scope_logs.scope.as_ref().expect("scope");
    assert_eq!(
        (scope.name.as_str(), scope.version.as_str()),
        ("sync-client", "1.2.3")
    );
    let record = &scope_logs.log_records[0];
    assert_eq!(record.time_unix_nano, unix_nanos(timestamp));
    assert_eq!(record.observed_time_unix_nano, unix_nanos(observed));
    assert_eq!(record.severity_number, Severity::Warn as i32);
    assert_eq!(record.severity_text, "WARN");
    assert!(matches!(
        record.body.as_ref().and_then(|body| body.value.as_ref()),
        Some(ProtoValue::StringValue(text)) if text == "disk nearly full"
    ));
    assert_eq!(record.trace_id, trace_id().to_bytes().to_vec());
    assert_eq!(record.span_id, span_id().to_bytes().to_vec());
    assert_eq!(record.flags & 0xff, u32::from(TraceFlags::SAMPLED.to_u8()));
}

#[test]
fn send_log_defaults_observed_timestamp_when_closure_leaves_it_unset() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint()).expect("client");
    let before = unix_nanos(SystemTime::now());
    client
        .send_log(&resource(), scope(), |record| {
            record.set_body(AnyValue::from("no timestamps"));
            Ok(())
        })
        .expect("log exported");
    let requests = collector.requests();
    let decoded =
        ExportLogsServiceRequest::decode(requests[0].body.as_slice()).expect("logs proto");
    let record = &decoded.resource_logs[0].scope_logs[0].log_records[0];
    assert!(
        record.observed_time_unix_nano >= before,
        "observed timestamp defaulted to now"
    );
    assert_eq!(record.time_unix_nano, 0, "event timestamp stays unset");
}

#[test]
fn send_span_exports_completed_native_span_data() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint()).expect("client");
    let span = completed_span();
    client
        .send_span(&resource(), span.clone())
        .expect("span exported");

    let requests = collector.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/traces");
    let decoded =
        ExportTraceServiceRequest::decode(requests[0].body.as_slice()).expect("trace proto");
    let resource_spans = &decoded.resource_spans[0];
    assert_eq!(
        string_attribute(
            &resource_spans
                .resource
                .as_ref()
                .expect("resource")
                .attributes,
            "service.name"
        ),
        Some(SERVICE)
    );
    let scope_spans = &resource_spans.scope_spans[0];
    assert_eq!(
        scope_spans.scope.as_ref().expect("scope").name,
        "sync-client"
    );
    let exported = &scope_spans.spans[0];
    assert_eq!(exported.name, "upload");
    assert_eq!(exported.trace_id, trace_id().to_bytes().to_vec());
    assert_eq!(exported.span_id, span_id().to_bytes().to_vec());
    assert_eq!(
        exported.parent_span_id,
        span.parent_span_id.to_bytes().to_vec()
    );
    assert_eq!(exported.kind, 3, "client span kind");
    assert_eq!(exported.start_time_unix_nano, unix_nanos(span.start_time));
    assert_eq!(exported.end_time_unix_nano, unix_nanos(span.end_time));
    assert_eq!(
        string_attribute(&exported.attributes, "http.request.method"),
        Some("POST")
    );
    assert_eq!(exported.events[0].name, "retry");
    assert_eq!(exported.links.len(), 1);
    let status = exported.status.as_ref().expect("status");
    assert_eq!((status.code, status.message.as_str()), (2, "upload failed"));
}

#[test]
fn send_metrics_exports_typed_observations_in_one_request() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint()).expect("client");
    client
        .send_metrics(&resource(), scope(), |meter| {
            meter
                .u64_counter("jobs.completed")
                .with_description("Completed jobs")
                .with_unit("{job}")
                .build()
                .add(3, &[KeyValue::new("queue", "default")]);
            meter.f64_gauge("queue.depth").build().record(7.5, &[]);
            meter
                .f64_histogram("job.duration")
                .with_unit("s")
                .build()
                .record(0.25, &[]);
            Ok(())
        })
        .expect("metrics exported");

    let requests = collector.requests();
    assert_eq!(
        requests.len(),
        1,
        "flush exports once; shutdown never resends"
    );
    assert_eq!(requests[0].path, "/v1/metrics");
    let decoded =
        ExportMetricsServiceRequest::decode(requests[0].body.as_slice()).expect("metrics proto");
    let resource_metrics = &decoded.resource_metrics[0];
    assert_eq!(
        string_attribute(
            &resource_metrics
                .resource
                .as_ref()
                .expect("resource")
                .attributes,
            "service.name"
        ),
        Some(SERVICE)
    );
    let scope_metrics = &resource_metrics.scope_metrics[0];
    assert_eq!(
        scope_metrics.scope.as_ref().expect("scope").version,
        "1.2.3"
    );
    let metric = |name: &str| {
        scope_metrics
            .metrics
            .iter()
            .find(|metric| metric.name == name)
            .unwrap_or_else(|| panic!("metric {name} exported"))
    };
    let counter = metric("jobs.completed");
    assert_eq!(
        (counter.description.as_str(), counter.unit.as_str()),
        ("Completed jobs", "{job}")
    );
    let Some(Data::Sum(sum)) = &counter.data else {
        panic!("counter exports a sum");
    };
    assert_eq!(sum.data_points[0].value, Some(NumberValue::AsInt(3)));
    assert_eq!(
        string_attribute(&sum.data_points[0].attributes, "queue"),
        Some("default")
    );
    let Some(Data::Gauge(gauge)) = &metric("queue.depth").data else {
        panic!("gauge exports a gauge");
    };
    assert_eq!(gauge.data_points[0].value, Some(NumberValue::AsDouble(7.5)));
    let histogram = metric("job.duration");
    assert_eq!(histogram.unit, "s");
    let Some(Data::Histogram(histogram)) = &histogram.data else {
        panic!("histogram exports a histogram");
    };
    assert_eq!(histogram.data_points[0].count, 1);
    assert_eq!(histogram.data_points[0].sum, Some(0.25));
}

#[test]
fn dropped_metric_measurement_is_invalid_and_sends_nothing() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint()).expect("client");

    let result = client.send_metrics(&resource(), scope(), |meter| {
        // The SDK replaces an invalid instrument name with a no-op, producing
        // an empty `ResourceMetrics` collection at flush time.
        meter.u64_counter("1-invalid name").build().add(1, &[]);
        Ok(())
    });

    let requests = collector.requests();
    assert!(
        requests.is_empty(),
        "the HTTP 200 collector receives no empty metrics export"
    );
    assert_eq!(validation_code(result), codes::INVALID_RECORD);
}

#[test]
fn invalid_input_is_validation_and_sends_nothing() {
    let collector = Collector::start(Reply::Ok);
    let mut client = Client::new(&collector.endpoint()).expect("client");

    let mut zero_ids = completed_span();
    zero_ids.span_context = SpanContext::empty_context();
    assert_eq!(
        validation_code(client.send_span(&resource(), zero_ids)),
        codes::INVALID_RECORD
    );

    let mut inverted = completed_span();
    inverted.end_time = inverted.start_time - Duration::from_secs(1);
    assert_eq!(
        validation_code(client.send_span(&resource(), inverted)),
        codes::INVALID_RECORD
    );

    let invalid_context = client.send_log(&resource(), scope(), |record| {
        record.set_trace_context(TraceId::INVALID, span_id(), None);
        Ok(())
    });
    assert_eq!(validation_code(invalid_context), codes::INVALID_RECORD);

    let rejected = client.send_log(&resource(), scope(), |_| {
        Err(SyncError::validation(
            codes::INVALID_RECORD,
            "unsupported severity",
        ))
    });
    assert_eq!(validation_code(rejected), codes::INVALID_RECORD);

    let closure_export_error = client.send_log(&resource(), scope(), |_| {
        Err(SyncError::Export(
            crate::sdk::error::OTelSdkError::InternalFailure("frontend failure".to_owned()),
        ))
    });
    assert_eq!(
        validation_code(closure_export_error),
        codes::CALLER_REJECTED
    );

    let metric_closure = client.send_metrics(&resource(), scope(), |meter| {
        meter
            .u64_counter("recorded.before.failure")
            .build()
            .add(1, &[]);
        Err(SyncError::validation(
            codes::INVALID_RECORD,
            "metric timestamps are unsupported",
        ))
    });
    assert_eq!(validation_code(metric_closure), codes::INVALID_RECORD);

    let nothing_recorded = client.send_metrics(&resource(), scope(), |meter| {
        // The SDK replaces an instrument with an invalid name by a no-op.
        meter.u64_counter("1-invalid name").build().add(1, &[]);
        Ok(())
    });
    assert_eq!(validation_code(nothing_recorded), codes::INVALID_RECORD);

    assert!(
        collector.requests().is_empty(),
        "no validation failure reaches the collector"
    );
}

#[test]
fn collector_failures_are_export_errors() {
    let refusing_listener = TcpListener::bind("127.0.0.1:0").expect("bind refusing listener");
    let mut client = Client::new(&format!(
        "http://{}",
        refusing_listener
            .local_addr()
            .expect("refusing listener address")
    ))
    .and_then(|client| client.with_timeout(STALL_TIMEOUT))
    .expect("client");
    assert!(matches!(
        client.send_span(&resource(), completed_span()),
        Err(SyncError::Export(_))
    ));

    let collector = Collector::start(Reply::Status(400, "bad request"));
    let mut client = Client::new(&collector.endpoint()).expect("client");
    assert!(matches!(
        client.send_span(&resource(), completed_span()),
        Err(SyncError::Export(_))
    ));
    assert!(matches!(
        client.send_metrics(&resource(), scope(), |meter| {
            meter.u64_counter("rejected").build().add(1, &[]);
            Ok(())
        }),
        Err(SyncError::Export(_))
    ));
}

fn assert_stalled_send_returns(
    send: impl FnOnce(&mut Client) -> Result<(), SyncError> + Send + 'static,
) {
    let (collector, stalled) = Collector::start_stalled();
    let endpoint = collector.endpoint();
    let (result_tx, result_rx) = mpsc::channel();
    let send_thread = thread::spawn(move || {
        let mut client = Client::new(&endpoint)
            .and_then(|client| client.with_timeout(STALL_TIMEOUT))
            .expect("client");
        let _ = result_tx.send(send(&mut client));
    });

    if let Err(error) = stalled.request_arrived.recv_timeout(FIXTURE_WATCHDOG) {
        let _ = stalled.release.send(());
        send_thread.join().expect("join stalled send worker");
        panic!("stalled collector received no request before watchdog: {error:?}");
    }
    let result = match result_rx.recv_timeout(FIXTURE_WATCHDOG) {
        Ok(result) => result,
        Err(error) => {
            let _ = stalled.release.send(());
            send_thread.join().expect("join stalled send worker");
            panic!("stalled send did not finish before watchdog: {error:?}");
        }
    };
    assert!(
        matches!(result, Err(SyncError::Export(_))),
        "stall is an export failure: {result:?}"
    );
    stalled
        .release
        .send(())
        .expect("release stalled collector after client timeout");
    send_thread.join().expect("join stalled send worker");
}

#[test]
fn stalled_log_export_times_out_while_collector_is_held() {
    assert_stalled_send_returns(|client| {
        client.send_log(&resource(), scope(), |record| {
            record.set_body(AnyValue::from("stalled"));
            Ok(())
        })
    });
}

#[test]
fn stalled_span_export_times_out_while_collector_is_held() {
    assert_stalled_send_returns(|client| client.send_span(&resource(), completed_span()));
}

#[test]
fn stalled_metric_flush_times_out_while_collector_is_held() {
    assert_stalled_send_returns(|client| {
        client.send_metrics(&resource(), scope(), |meter| {
            meter.u64_counter("stalled").build().add(1, &[]);
            Ok(())
        })
    });
}

#[test]
fn entered_tokio_runtime_is_rejected_and_drop_is_safe() {
    let collector = Collector::start(Reply::Ok);
    let prebuilt = Client::new(&collector.endpoint()).expect("client outside a runtime");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime");
    let endpoint = collector.endpoint();
    runtime.block_on(async move {
        assert_eq!(
            validation_code(Client::new(&endpoint).map(drop)),
            codes::RUNTIME_ENTERED
        );
        let mut client = prebuilt;
        assert_eq!(
            validation_code(client.send_span(&resource(), completed_span())),
            codes::RUNTIME_ENTERED
        );
        // The client owns no transport, so dropping it inside the runtime is safe.
        drop(client);
    });
    assert!(collector.requests().is_empty());
}

#[test]
fn input_limits_accept_at_limit_and_reject_above() {
    check_input_limits(MAX_INPUT_BYTES, MAX_BATCH_RECORDS).expect("at-limit input accepted");
    assert_eq!(
        validation_code(check_input_limits(MAX_INPUT_BYTES + 1, 1)),
        codes::INPUT_LIMIT_EXCEEDED
    );
    assert_eq!(
        validation_code(check_input_limits(1, MAX_BATCH_RECORDS + 1)),
        codes::INPUT_LIMIT_EXCEEDED
    );
}

#[test]
fn invalid_configuration_is_rejected() {
    for endpoint in [
        "not a url",
        "ftp://collector:4318",
        "http://collector:4318/?q=1",
    ] {
        assert_eq!(
            validation_code(Client::new(endpoint).map(drop)),
            codes::INVALID_CONFIG
        );
    }
    let client = || Client::new("http://127.0.0.1:4318").expect("client");
    assert_eq!(
        validation_code(client().with_header("bad header", "v").map(drop)),
        codes::INVALID_CONFIG
    );
    assert_eq!(
        validation_code(client().with_header("x-token", "line\nbreak").map(drop)),
        codes::INVALID_CONFIG
    );
    assert_eq!(
        validation_code(client().with_timeout(Duration::ZERO).map(drop)),
        codes::INVALID_CONFIG
    );
    assert_eq!(
        validation_code(
            client()
                .with_root_certificate_pem(b"not a certificate")
                .map(drop)
        ),
        codes::INVALID_CONFIG
    );
}

#[test]
fn credentials_never_appear_in_failure_text() {
    const TOKEN: &str = "s3cr3t-token-value";
    const PASSWORD: &str = "hunter2-password";
    // The collector echoes the credential back in its error body.
    let collector = Collector::start(Reply::Status(401, "rejected Bearer s3cr3t-token-value"));
    let endpoint = format!("http://otlp-user:{PASSWORD}@{}", collector.address);
    let mut client = Client::new(&endpoint)
        .and_then(|client| client.with_header("authorization", &format!("Bearer {TOKEN}")))
        .expect("client");
    let error = client
        .send_span(&resource(), completed_span())
        .expect_err("collector rejects the export");
    let mut rendered = vec![
        error.to_string(),
        format!("{error:?}"),
        format!("{client:?}"),
    ];
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        rendered.push(cause.to_string());
        source = cause.source();
    }
    for text in rendered {
        assert!(!text.contains(TOKEN), "token leaked: {text}");
        assert!(!text.contains(PASSWORD), "password leaked: {text}");
        assert!(!text.contains("otlp-user"), "user name leaked: {text}");
    }
    let requests = collector.requests();
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str()),
        "the credential is still sent to the collector"
    );
}

/// Explicit header the precedence child sets and every environment collides with.
const EXPLICIT_TENANT: &str = "explicit-tenant";

/// Child half of the precedence tests: runs only when re-invoked with the OTEL
/// environment set, so the variables never leak into this process's other
/// tests. Sends one log, span and metric export with explicit configuration.
#[test]
fn explicit_configuration_child() {
    let Ok(endpoint) = std::env::var("SC_OTLP_SYNC_PRECEDENCE_ENDPOINT") else {
        return;
    };
    let mut client = Client::new(&endpoint)
        .and_then(|client| client.with_header("x-tenant", EXPLICIT_TENANT))
        .expect("client");
    client
        .send_log(&resource(), scope(), |record| {
            record.set_body(AnyValue::from("precedence"));
            Ok(())
        })
        .expect("log sent to the explicit endpoint");
    client
        .send_span(&resource(), completed_span())
        .expect("span sent to the explicit endpoint");
    client
        .send_metrics(&resource(), scope(), |meter| {
            meter.u64_counter("precedence").build().add(1, &[]);
            Ok(())
        })
        .expect("metrics sent to the explicit endpoint");
}

/// Child half of the timeout-precedence proof. Its explicit timeout must win
/// over the conflicting OTEL environment timeouts set by the parent.
#[test]
fn explicit_timeout_precedence_child() {
    let Ok(endpoint) = std::env::var("SC_OTLP_SYNC_PRECEDENCE_STALL_ENDPOINT") else {
        return;
    };
    let mut client = Client::new(&endpoint)
        .and_then(|client| client.with_timeout(STALL_TIMEOUT))
        .expect("client with explicit timeout");
    assert!(matches!(
        client.send_span(&resource(), completed_span()),
        Err(SyncError::Export(_))
    ));
}

fn precedence_child_command(test_name: &str, endpoint: String) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("test binary"));
    command
        .args(["--exact", test_name, "--nocapture"])
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9");
    for signal in ["LOGS", "TRACES", "METRICS"] {
        command
            .env(
                format!("OTEL_EXPORTER_OTLP_{signal}_ENDPOINT"),
                "http://127.0.0.1:9/v1/other",
            )
            .env(format!("OTEL_EXPORTER_OTLP_{signal}_TIMEOUT"), "60000");
    }
    command
        .env("OTEL_EXPORTER_OTLP_TIMEOUT", "60000")
        .env("SC_OTLP_SYNC_PRECEDENCE_ENDPOINT", &endpoint)
        .env("SC_OTLP_SYNC_PRECEDENCE_STALL_ENDPOINT", endpoint);
    command
}

/// Runs [`explicit_configuration_child`] with conflicting OTEL endpoint and
/// timeout settings plus `extra` and returns the requests the explicit
/// endpoint received, by signal path.
fn run_precedence_child(extra: &[(&str, &str)]) -> Vec<Captured> {
    let collector = Collector::start(Reply::Ok);
    let mut command = precedence_child_command(
        "sync::tests::explicit_configuration_child",
        collector.endpoint(),
    );
    command.envs(extra.iter().copied());
    let output = command.output().expect("run precedence child test");
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "child test ran"
    );
    let mut requests = collector.requests();
    requests.sort_by(|left, right| left.path.cmp(&right.path));
    let paths: Vec<&str> = requests
        .iter()
        .map(|request| request.path.as_str())
        .collect();
    assert_eq!(
        paths,
        ["/v1/logs", "/v1/metrics", "/v1/traces"],
        "every signal reached the explicit endpoint"
    );
    requests
}

#[test]
fn explicit_timeout_wins_over_environment_while_collector_is_held() {
    let (collector, stalled) = Collector::start_stalled();
    let mut command = precedence_child_command(
        "sync::tests::explicit_timeout_precedence_child",
        collector.endpoint(),
    );
    let (output_tx, output_rx) = mpsc::channel();
    let child = thread::spawn(move || {
        let _ = output_tx.send(command.output());
    });

    if let Err(error) = stalled.request_arrived.recv_timeout(FIXTURE_WATCHDOG) {
        let _ = stalled.release.send(());
        child.join().expect("join precedence child");
        panic!("precedence child never reached the stalled collector: {error:?}");
    }
    let output = match output_rx.recv_timeout(PRECEDENCE_CHILD_WATCHDOG) {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            let _ = stalled.release.send(());
            child.join().expect("join precedence child");
            panic!("run precedence child: {error}");
        }
        Err(error) => {
            let _ = stalled.release.send(());
            child.join().expect("join precedence child");
            panic!("explicit timeout did not beat environment timeout: {error:?}");
        }
    };
    assert!(
        output.status.success(),
        "precedence child failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    stalled
        .release
        .send(())
        .expect("release stalled collector after explicit timeout");
    child.join().expect("join precedence child");
}

#[test]
fn explicit_endpoint_and_timeout_win_over_environment() {
    for request in run_precedence_child(&[]) {
        assert_eq!(request.header("x-tenant"), Some(EXPLICIT_TENANT));
    }
}

#[test]
fn explicit_headers_win_over_general_environment_headers() {
    let requests = run_precedence_child(&[(
        "OTEL_EXPORTER_OTLP_HEADERS",
        "x-tenant=env-tenant,x-env-general=applied",
    )]);
    for request in requests {
        assert_eq!(
            request.header("x-tenant"),
            Some(EXPLICIT_TENANT),
            "{}: explicit header replaced by the environment",
            request.path
        );
        assert_eq!(
            request.header("x-env-general"),
            Some("applied"),
            "{}: environment headers were applied",
            request.path
        );
    }
}

#[test]
fn explicit_headers_win_over_signal_environment_headers() {
    let requests = run_precedence_child(&[
        ("OTEL_EXPORTER_OTLP_HEADERS", "x-tenant=env-general"),
        (
            "OTEL_EXPORTER_OTLP_LOGS_HEADERS",
            "x-tenant=env-logs,x-env-signal=logs",
        ),
        (
            "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
            "x-tenant=env-traces,x-env-signal=traces",
        ),
        (
            "OTEL_EXPORTER_OTLP_METRICS_HEADERS",
            "x-tenant=env-metrics,x-env-signal=metrics",
        ),
    ]);
    for request in requests {
        let signal = request.path.trim_start_matches("/v1/");
        assert_eq!(
            request.header("x-tenant"),
            Some(EXPLICIT_TENANT),
            "{signal}: explicit header replaced by the environment"
        );
        assert_eq!(
            request.header("x-env-signal"),
            Some(signal),
            "{signal}: signal environment headers were applied"
        );
    }
}

/// Self-signed loopback certificate and key generated with the `openssl` CLI.
struct TestCertificate {
    cert: PathBuf,
    key: PathBuf,
}

impl TestCertificate {
    fn generate() -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let base =
            std::env::temp_dir().join(format!("sc-otlp-sync-{}-{nonce}", std::process::id()));
        let cert = base.with_extension("crt");
        let key = base.with_extension("key");
        let generated = Command::new("openssl")
            .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .args([
                "-days",
                "2",
                "-subj",
                "/CN=127.0.0.1",
                "-addext",
                "subjectAltName=IP:127.0.0.1",
                "-addext",
                "basicConstraints=critical,CA:FALSE",
                "-addext",
                "extendedKeyUsage=serverAuth",
            ])
            .output()
            .expect("openssl is available for the TLS test");
        assert!(
            generated.status.success(),
            "generate loopback TLS certificate"
        );
        Self { cert, key }
    }

    fn server_config(&self) -> TlsConfig {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};
        let chain = CertificateDer::pem_file_iter(&self.cert)
            .expect("open certificate")
            .collect::<Result<Vec<_>, _>>()
            .expect("parse certificate");
        let key = PrivateKeyDer::from_pem_file(&self.key).expect("parse key");
        Arc::new(
            rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("TLS protocol versions")
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .expect("server certificate"),
        )
    }
}

impl Drop for TestCertificate {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.cert);
        let _ = fs::remove_file(&self.key);
    }
}

fn start_tls_collector(config: TlsConfig) -> Collector {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind TLS collector");
    listener
        .set_nonblocking(true)
        .expect("nonblocking TLS listener");
    let address = listener.local_addr().expect("TLS collector address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let requests = Arc::clone(&requests);
        let stop = Arc::clone(&stop);
        thread::spawn(move || serve(&listener, &Reply::Ok, &requests, &stop, Some(&config)))
    };
    Collector {
        address,
        requests,
        stop,
        thread: Some(thread),
    }
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).expect("read certificate")
}

#[test]
fn tls_verifies_trusted_and_rejects_untrusted_certificates() {
    let certificate = TestCertificate::generate();
    let collector = start_tls_collector(certificate.server_config());
    let endpoint = format!("https://{}", collector.address);

    let mut trusted = Client::new(&endpoint)
        .and_then(|client| client.with_root_certificate_pem(&read(&certificate.cert)))
        .expect("client trusting the loopback certificate");
    trusted
        .send_span(&resource(), completed_span())
        .expect("trusted certificate completes the export");

    let mut untrusted = Client::new(&endpoint)
        .and_then(|client| client.with_timeout(STALL_TIMEOUT))
        .expect("client without the loopback certificate");
    assert!(matches!(
        untrusted.send_span(&resource(), completed_span()),
        Err(SyncError::Export(_))
    ));
    assert_eq!(
        collector.requests().len(),
        1,
        "only the verified export was delivered"
    );
}
