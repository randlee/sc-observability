use super::super::*;
use super::capture::{CAPTURE_TIMEOUT, capture_server};
use super::proto_json::{NonFiniteDouble, decode_forms};
use crate::config::{ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, SyncHttpRetryPolicy};
use sc_observability_types::{
    SpanId, Timestamp, TraceId,
    otlp::submission::{IdSource, Signal, SubmissionEnvelope},
};
use std::net::TcpListener;

struct Ids;
impl IdSource for Ids {
    fn trace_id(&mut self) -> TraceId {
        TraceId::new("0123456789abcdef0123456789abcdef").expect("trace id")
    }
    fn span_id(&mut self) -> SpanId {
        SpanId::new("0123456789abcdef").expect("span id")
    }
    fn now(&mut self) -> Timestamp {
        Timestamp::UNIX_EPOCH
    }
}

fn fixture(name: &str) -> SubmissionEnvelope {
    let path = format!(
        "{}/../sc-observability-types/tests/fixtures/otlp_submission/golden/{name}/input.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let input = std::fs::read_to_string(path).expect("fixture reads");
    let envelope = SubmissionEnvelope::from_json(&input, &mut Ids).expect("canonical envelope");
    let canonical = envelope.to_canonical_json();
    serde_json::from_str(&canonical).expect("store envelope parses")
}

fn submission_exporter(
    endpoint: String,
    retry: Option<SyncHttpRetryPolicy>,
) -> SyncHttpSubmissionExporter {
    let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    config.enabled = true;
    config.endpoint = Some(OtlpEndpoint::new_typed(endpoint).expect("loopback endpoint"));
    config.sync_http_retry = retry;
    let (config, bounds) = SyncHttpConfig::from_otel(&config).expect("valid test exporter");
    SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter: Mutex::new(None),
    }
}

fn retry_policy(max_retries: u32) -> SyncHttpRetryPolicy {
    SyncHttpRetryPolicy {
        max_retries: Some(max_retries),
        initial_backoff_ms: Some(5.into()),
        max_backoff_ms: Some(5.into()),
        retry_sequence_timeout_ms: Some(3_000.into()),
        retry_after_cap_ms: Some(20.into()),
        retry_jitter_percent: Some(0),
    }
}

#[test]
fn submission_exporter_round_trips_every_signal_variant() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) = capture_server(listener, &[200, 200, 200, 200]);
    exporter
        .export(
            Signal::Logs,
            &[fixture("logs"), fixture("non_finite_doubles")],
        )
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
    assert_eq!(
        requests
            .iter()
            .map(|request| request.0.as_str())
            .collect::<Vec<_>>(),
        [
            "/v1/logs",
            "/v1/traces",
            "/v1/metrics",
            "/v1development/profiles"
        ]
    );
    assert!(
        requests[0].1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["observedTimeUnixNano"]
            .is_string()
    );
    assert!(requests[1].1["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["events"].is_array());
    assert_eq!(
        requests[2].1["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(
        requests[3].1["dictionary"]["linkTable"][0]["traceId"],
        "AAAAAAAAAAAAAAAAAAAAAA=="
    );
    let decoded = requests
        .iter()
        .map(|request| decode_forms(&request.1).expect("decode captured proto JSON"))
        .collect::<Vec<_>>();
    assert_eq!(
        decoded[0].non_finite_doubles,
        vec![
            NonFiniteDouble::NaN,
            NonFiniteDouble::PositiveInfinity,
            NonFiniteDouble::NegativeInfinity,
        ]
    );
    assert!(decoded[3].key_indices.contains(&0));
}

#[test]
fn submission_exporter_classifies_400_as_terminal_and_exhausted_503_as_retryable() {
    for (status, expected_retryable) in [(400, false), (503, true)] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let exporter = submission_exporter(
            format!("http://{}", listener.local_addr().expect("address")),
            Some(retry_policy(1)),
        );
        let statuses = if expected_retryable {
            vec![status, status]
        } else {
            vec![status]
        };
        let (captured, server) = capture_server(listener, &statuses);
        let result = exporter.export(Signal::Logs, &[fixture("logs")]);
        for _ in statuses {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("captured request");
        }
        server.join().expect("capture server exits");
        assert_eq!(
            matches!(result, Err(SubmissionExportFailure::Retryable(_))),
            expected_retryable,
            "status {status}"
        );
        assert_eq!(
            matches!(result, Err(SubmissionExportFailure::Terminal(_))),
            !expected_retryable,
            "status {status}"
        );
    }
}

#[test]
fn submission_exporter_retries_503_then_succeeds() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        Some(retry_policy(1)),
    );
    let (captured, server) = capture_server(listener, &[503, 200]);
    exporter
        .export(Signal::Logs, &[fixture("logs")])
        .expect("retry succeeds");
    let first = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("first request");
    let second = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("retried request");
    server.join().expect("capture server exits");
    assert_eq!(first, second, "retry resubmits the same canonical payload");
}
