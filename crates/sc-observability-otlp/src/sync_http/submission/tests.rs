use super::*;
use crate::config::{ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol};
use sc_observability_types::otlp::submission::SubmissionEnvelope;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

fn submission_exporter(endpoint: String) -> SyncHttpSubmissionExporter {
    let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    config.enabled = true;
    config.endpoint = Some(OtlpEndpoint::new_typed(endpoint).expect("loopback endpoint"));
    let (config, bounds) = SyncHttpConfig::from_otel(&config).expect("valid test exporter");
    SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter: Mutex::new(None),
    }
}

fn fixture(name: &str) -> SubmissionEnvelope {
    let path = format!(
        "{}/../sc-observability-types/tests/fixtures/otlp_submission/golden/{name}/expected.envelope.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture reads"))
        .expect("fixture envelope parses")
}

fn read_request(stream: &mut TcpStream) -> (String, Value) {
    stream
        .set_read_timeout(Some(CAPTURE_TIMEOUT))
        .expect("set capture timeout");
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).expect("read request");
        assert_ne!(count, 0, "client closed before complete request");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end]).expect("request headers UTF-8");
    let request_line = headers.lines().next().expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .expect("content length")
        .parse::<usize>()
        .expect("numeric content length");
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).expect("read request body");
        assert_ne!(count, 0, "client closed before complete request body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let body = serde_json::from_slice(&bytes[header_end..header_end + content_length])
        .expect("OTLP/JSON body parses");
    (path, body)
}

fn capture_server(
    listener: TcpListener,
    expected: usize,
) -> (mpsc::Receiver<(String, Value)>, thread::JoinHandle<()>) {
    let (captured_tx, captured_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        for _ in 0..expected {
            let (mut stream, _) = listener.accept().expect("accept submission request");
            let request = read_request(&mut stream);
            captured_tx.send(request).expect("deliver captured request");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("write capture response");
        }
    });
    (captured_rx, server)
}

#[test]
fn submission_exporter_posts_proto_json_to_every_signal_path() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let endpoint = format!(
        "http://{}",
        listener.local_addr().expect("listener address")
    );
    let (captured, server) = capture_server(listener, 4);
    let exporter = submission_exporter(endpoint);

    exporter
        .export(Signal::Logs, &[fixture("logs")])
        .expect("logs delivery");
    exporter
        .export(Signal::Traces, &[fixture("traces")])
        .expect("traces delivery");
    exporter
        .export(
            Signal::Metrics,
            &[
                fixture("metric_gauge"),
                fixture("metric_sum"),
                fixture("metric_histogram"),
                fixture("metric_exponential_histogram"),
                fixture("metric_summary"),
            ],
        )
        .expect("metrics delivery");
    exporter
        .export(Signal::Profiles, &[fixture("profiles")])
        .expect("profiles delivery");

    let requests = (0..4)
        .map(|_| {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("captured request")
        })
        .collect::<Vec<_>>();
    server.join().expect("capture server exits");

    assert_eq!(requests[0].0, "/v1/logs");
    assert!(
        requests[0].1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["observedTimeUnixNano"]
            .is_string()
    );
    assert_eq!(requests[1].0, "/v1/traces");
    assert!(requests[1].1["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["events"].is_array());
    assert_eq!(requests[2].0, "/v1/metrics");
    assert_eq!(
        requests[2].1["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(requests[3].0, "/v1development/profiles");
    assert_eq!(
        requests[3].1["dictionary"]["linkTable"][0]["traceId"],
        "AAAAAAAAAAAAAAAAAAAAAA=="
    );
}

#[test]
fn submission_exporter_classifies_terminal_http_status_as_terminal() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind terminal listener");
    let endpoint = format!(
        "http://{}",
        listener.local_addr().expect("listener address")
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept submission request");
        let _ = read_request(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write terminal response");
    });
    let result = submission_exporter(endpoint).export(Signal::Logs, &[fixture("logs")]);
    server.join().expect("terminal server exits");
    match result {
        Err(SubmissionExportFailure::Terminal(error)) => {
            assert!(matches!(error, ExportError::NonRetryableHttpStatus { .. }));
        }
        result => panic!("expected terminal submission failure, got {result:?}"),
    }
}

#[test]
fn classification_preserves_retryable_failure_context() {
    let failure = classify(super::super::implementation::worker_terminated_error());
    match failure {
        SubmissionExportFailure::Retryable(error) => {
            assert!(matches!(error, ExportError::WorkerTerminated { .. }));
        }
        failure @ SubmissionExportFailure::Terminal(_) => {
            panic!("expected retryable submission failure, got {failure:?}")
        }
    }
}
