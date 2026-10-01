use super::super::*;
use super::capture::{CAPTURE_TIMEOUT, capture_server};
use super::proto_json::{NonFiniteDouble, decode_forms};
use crate::config::{ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, SyncHttpRetryPolicy};
use crate::constants::MAX_OTLP_ENCODED_REQUEST_BYTES;
use sc_observability_types::{
    SpanId, Timestamp, TraceId,
    otlp::{
        signals::AnyValue,
        submission::{IdSource, Signal, SubmissionEnvelope},
    },
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
    let input = golden_fixture(name, "input.json");
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
    let exporter = Arc::new(
        OtlpHttpExporter::from_prepared(config.clone(), &bounds)
            .expect("test exporter constructs eagerly"),
    );
    SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter,
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
    assert_eq!(server.join().expect("capture server exits"), 4);
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
        for _ in &statuses {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("captured request");
        }
        assert_eq!(server.join().expect("capture server exits"), statuses.len(),);
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
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(first, second, "retry resubmits the same canonical payload");
}

#[test]
fn profiles_with_distinct_dictionaries_are_submitted_separately() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let first = fixture("profiles");
    let mut second = fixture("profiles");
    second
        .profiles
        .as_mut()
        .expect("profile fixture contains profiles")
        .dictionary
        .string_table[0] = "a distinct dictionary entry".to_owned();
    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(Signal::Profiles, &[first, second])
        .expect("profiles delivery");
    let first = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("first request");
    let second = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("second request");
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(first.0, "/v1development/profiles");
    assert_eq!(second.0, "/v1development/profiles");
    assert_ne!(
        first.1["dictionary"]["stringTable"][0], second.1["dictionary"]["stringTable"][0],
        "each request retains the dictionary that owns its profile indices"
    );
}

#[test]
fn generated_id_and_plain_attribute_fixtures_reach_their_signal_routes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let paired = fixture("paired_log_span_generated_ids");
    let plain_attributes = fixture("plain_attribute_map");
    let (captured, server) = capture_server(listener, &[200, 200]);

    exporter
        .export(Signal::Logs, &[paired.clone(), plain_attributes])
        .expect("log fixtures deliver");
    exporter
        .export(Signal::Traces, &[paired])
        .expect("generated-id span fixture delivers");

    let logs = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured logs fixture request");
    let traces = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured traces fixture request");
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(logs.0, "/v1/logs");
    assert_eq!(traces.0, "/v1/traces");
    assert_eq!(
        logs.1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][1]["attributes"],
        serde_json::json!([
            {"key": "a", "value": {"intValue": "2"}},
            {"key": "z", "value": {"boolValue": true}},
        ]),
        "the plain attribute fixture retains its canonical key/value pairs"
    );
    assert!(
        traces.1["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"].is_string(),
        "the generated-id fixture is encoded into the trace request"
    );
}

#[test]
fn logs_are_grouped_by_distinct_resource_and_scope() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let first = fixture("logs");
    let mut second_scope = first.clone();
    second_scope.logs[0].scope.name = "second-scope".to_owned();
    let mut second_resource = first.clone();
    second_resource.logs[0].resource.schema_url =
        Some("https://example.test/second-resource".to_owned());
    let (captured, server) = capture_server(listener, &[200]);

    exporter
        .export(Signal::Logs, &[first, second_scope, second_resource])
        .expect("grouped logs deliver");
    let (_, request) = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured grouped request");
    assert_eq!(server.join().expect("capture server exits"), 1);
    let resources = request["resourceLogs"].as_array().expect("resource groups");
    assert_eq!(resources.len(), 2, "different resources must not coalesce");
    assert_eq!(
        resources[0]["scopeLogs"].as_array().map(Vec::len),
        Some(2),
        "different scopes within one resource stay distinct"
    );
    assert_eq!(
        resources[1]["resource"]["schemaUrl"],
        "https://example.test/second-resource"
    );
}

#[test]
fn oversized_multi_envelope_submission_splits_at_encoded_request_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let mut first = fixture("logs");
    let mut second = fixture("logs");
    let first_body = "a".repeat(MAX_OTLP_ENCODED_REQUEST_BYTES / 2);
    let second_body = "b".repeat(MAX_OTLP_ENCODED_REQUEST_BYTES / 2);
    first.logs[0].record.body = Some(AnyValue::String(first_body.clone()));
    second.logs[0].record.body = Some(AnyValue::String(second_body.clone()));

    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(Signal::Logs, &[first, second])
        .expect("oversized batch is split and delivered");
    let requests = (0..2)
        .map(|_| {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("split request")
        })
        .collect::<Vec<_>>();

    assert_eq!(server.join().expect("capture server exits"), 2);
    assert!(
        requests
            .iter()
            .all(|(_, request)| request.to_string().len() <= MAX_OTLP_ENCODED_REQUEST_BYTES),
        "every split request respects the encoded-byte limit"
    );
    assert_eq!(
        requests
            .iter()
            .map(
                |(_, request)| request["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]
                    ["stringValue"]
                    .as_str()
            )
            .collect::<Vec<_>>(),
        [Some(first_body.as_str()), Some(second_body.as_str())],
        "the split retains each original envelope exactly once"
    );
}
