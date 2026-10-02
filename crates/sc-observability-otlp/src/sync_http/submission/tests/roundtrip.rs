use super::super::*;
use super::capture::{CAPTURE_TIMEOUT, capture_server};
use super::proto_json::{NonFiniteDouble, decode_any_value, decode_forms, decode_key_values};
use crate::config::{
    ExporterBackend, LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, ResourceAttributes,
    SyncHttpRetryPolicy, TelemetryConfig, prepared_backend_connection,
    validated_released_telemetry_bounds,
};
use crate::constants::MAX_OTLP_ENCODED_REQUEST_BYTES;
use sc_observability_types::{
    ServiceName, SpanId, Timestamp, TraceId,
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

fn envelope(input: &serde_json::Value) -> SubmissionEnvelope {
    SubmissionEnvelope::from_json(&input.to_string(), &mut Ids).expect("canonical envelope")
}

fn submission_exporter(
    endpoint: String,
    retry: Option<SyncHttpRetryPolicy>,
) -> SyncHttpSubmissionExporter {
    submission_exporter_with_shutdown(endpoint, retry, None)
}

fn submission_exporter_with_shutdown(
    endpoint: String,
    retry: Option<SyncHttpRetryPolicy>,
    shutdown_ms: Option<u64>,
) -> SyncHttpSubmissionExporter {
    let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    config.enabled = true;
    config.lifecycle_shutdown_timeout_ms = shutdown_ms.map(Into::into);
    if let Some(shutdown) = shutdown_ms {
        config.timeout_ms = Some(shutdown.into());
        config.lifecycle_flush_timeout_ms = Some(shutdown.into());
    }
    config.endpoint = Some(OtlpEndpoint::new_typed(endpoint).expect("loopback endpoint"));
    config.sync_http_retry = retry;
    let telemetry = TelemetryConfig {
        service_name: ServiceName::new("submission-capture").expect("service name"),
        resource: ResourceAttributes::default(),
        transport: config,
        logs: Some(LogsConfig::default()),
        traces: None,
        metrics: None,
    };
    // The released compatibility preparation path explicitly permits immediate
    // retry delays, which makes these capture assertions independent of the
    // scheduler while retaining production retry behavior.
    let bounds = validated_released_telemetry_bounds(&telemetry).expect("valid test exporter");
    let connection = prepared_backend_connection(&telemetry.transport, &bounds)
        .expect("prepared test connection");
    let config = SyncHttpConfig::from_prepared(&connection, &bounds).expect("valid test exporter");
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
        // Capture tests assert retry behavior directly, so keep their timing
        // independent of wall-clock scheduling.
        initial_backoff_ms: Some(0.into()),
        max_backoff_ms: Some(0.into()),
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
fn submission_exporter_classifies_400_and_exhausted_503_as_terminal() {
    for status in [400, 503] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let exporter = submission_exporter(
            format!("http://{}", listener.local_addr().expect("address")),
            Some(retry_policy(1)),
        );
        let statuses = if status == 503 {
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
        assert_eq!(server.join().expect("capture server exits"), statuses.len());
        assert!(
            matches!(result, Err(SubmissionExportFailure::Terminal(_))),
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
    let expected_attributes = plain_attributes.logs[0].record.attributes.clone();
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
    let decoded_attributes = decode_key_values(
        &logs.1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][1]["attributes"],
    )
    .expect("captured attributes decode back into canonical values");
    assert_eq!(
        decoded_attributes, expected_attributes,
        "the plain attribute fixture retains each canonical key/value pair"
    );
    assert!(
        traces.1["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"].is_string(),
        "the generated-id fixture is encoded into the trace request"
    );
}

#[test]
fn rich_log_and_span_fields_survive_capture_as_canonical_values() {
    let input = serde_json::json!({
        "version": 1,
        "logs": [{
            "time": "1970-01-01T00:00:00.000000000Z",
            "observed_time": "1970-01-01T00:00:01.000000000Z",
            "severity_number": 17,
            "severity_text": "ERROR",
            "event_name": "audit.event",
            "body": {"kind": "string", "data": "log body"},
            "attributes": [["log.attribute", {"kind": "bool", "data": true}]],
            "dropped_attributes_count": 2,
            "flags": 5,
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef"
        }],
        "spans": [{
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef",
            "trace_state": "vendor=state",
            "parent_span_id": "fedcba9876543210",
            "flags": 7,
            "name": "operation",
            "kind": "server",
            "start_time": "1970-01-01T00:00:00.000000000Z",
            "duration_nanos": 2_000_000_000_u64,
            "attributes": [["span.attribute", {"kind": "int", "data": 42}]],
            "dropped_attributes_count": 3,
            "events": [{
                "time": "1970-01-01T00:00:01.000000000Z",
                "name": "event",
                "attributes": [["event.attribute", {"kind": "string", "data": "value"}]],
                "dropped_attributes_count": 4
            }],
            "dropped_events_count": 5,
            "links": [{
                "trace_id": "11111111111111111111111111111111",
                "span_id": "2222222222222222",
                "trace_state": "link=state",
                "attributes": [["link.attribute", {"kind": "int", "data": 9}]],
                "dropped_attributes_count": 6,
                "flags": 8
            }],
            "dropped_links_count": 9,
            "status": {"code": "error", "message": "failed"}
        }]
    });
    let envelope = envelope(&input);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(Signal::Logs, std::slice::from_ref(&envelope))
        .expect("log delivery");
    exporter
        .export(Signal::Traces, std::slice::from_ref(&envelope))
        .expect("span delivery");
    let logs = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured logs")
        .1;
    let traces = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured traces")
        .1;
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_captured_log(&logs, &envelope);
    assert_captured_span(&traces, &envelope);
}

fn assert_captured_log(captured: &serde_json::Value, envelope: &SubmissionEnvelope) {
    let expected = &envelope.logs[0].record;
    let log = &captured["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert_eq!(log["timeUnixNano"], "0");
    assert_eq!(log["observedTimeUnixNano"], "1000000000");
    assert_eq!(log["severityNumber"], expected.severity_number.get());
    assert_eq!(
        log["severityText"],
        expected.severity_text.as_deref().unwrap()
    );
    assert_eq!(log["eventName"], expected.event_name.as_deref().unwrap());
    assert_eq!(
        decode_any_value(&log["body"]).expect("decode log body"),
        expected.body.clone().expect("log body")
    );
    assert_eq!(
        decode_key_values(&log["attributes"]).expect("decode log attributes"),
        expected.attributes
    );
    assert_eq!(log["droppedAttributesCount"], 2);
    assert_eq!(log["flags"], 5);
    assert_eq!(log["traceId"], "0123456789abcdef0123456789abcdef");
    assert_eq!(log["spanId"], "0123456789abcdef");
}

fn assert_captured_span(captured: &serde_json::Value, envelope: &SubmissionEnvelope) {
    let expected = &envelope.spans[0].record;
    let span = &captured["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    assert_eq!(span["traceId"], "0123456789abcdef0123456789abcdef");
    assert_eq!(span["spanId"], "0123456789abcdef");
    assert_eq!(span["parentSpanId"], "fedcba9876543210");
    assert_eq!(
        span["traceState"],
        expected.trace_state.as_ref().unwrap().as_str()
    );
    assert_eq!(span["flags"], expected.flags);
    assert_eq!(span["name"], expected.name);
    assert_eq!(span["kind"], 2);
    assert_eq!(span["startTimeUnixNano"], "0");
    assert_eq!(span["endTimeUnixNano"], "2000000000");
    assert_eq!(
        decode_key_values(&span["attributes"]).expect("decode span attributes"),
        expected.attributes
    );
    assert_eq!(
        span["droppedAttributesCount"],
        expected.dropped_attributes_count
    );
    assert_eq!(span["droppedEventsCount"], expected.dropped_events_count);
    assert_eq!(span["droppedLinksCount"], expected.dropped_links_count);
    assert_eq!(span["status"]["code"], 2);
    assert_eq!(span["status"]["message"], "failed");
    let event = &span["events"][0];
    assert_eq!(event["timeUnixNano"], "1000000000");
    assert_eq!(event["name"], expected.events[0].name);
    assert_eq!(
        decode_key_values(&event["attributes"]).expect("decode event attributes"),
        expected.events[0].attributes
    );
    assert_eq!(event["droppedAttributesCount"], 4);
    let link = &span["links"][0];
    assert_eq!(link["traceId"], "11111111111111111111111111111111");
    assert_eq!(link["spanId"], "2222222222222222");
    assert_eq!(link["traceState"], "link=state");
    assert_eq!(
        decode_key_values(&link["attributes"]).expect("decode link attributes"),
        expected.links[0].attributes
    );
    assert_eq!(link["droppedAttributesCount"], 6);
    assert_eq!(link["flags"], 8);
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

#[test]
fn runtime_short_shutdown_does_not_requeue_submission() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let exporter = submission_exporter_with_shutdown(
        format!("http://{}", listener.local_addr().unwrap()),
        Some(retry_policy(1)),
        Some(500),
    );
    assert_eq!(
        exporter.exporter.submission_wait_budget(),
        std::time::Duration::from_millis(3000) + crate::constants::SUBMISSION_DISPATCH_MARGIN
    );
    // Zero backoff and explicit responses exercise the worker's result without
    // sleeping or asserting scheduler-dependent elapsed time.
    let (captured, server) = capture_server(listener, &[503, 200]);
    exporter.export(Signal::Logs, &[fixture("logs")]).unwrap();
    for _ in 0..2 {
        captured.recv_timeout(CAPTURE_TIMEOUT).unwrap();
    }
    assert_eq!(server.join().unwrap(), 2);
}

#[test]
fn runtime_routes_preserve_existing_endpoint_suffixes() {
    for (signal, name, suffix) in [
        (Signal::Logs, "logs", "/v1/logs"),
        (Signal::Traces, "traces", "/v1/traces"),
        (Signal::Metrics, "metric_gauge", "/v1/metrics"),
        (Signal::Profiles, "profiles", "/v1development/profiles"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let exporter = submission_exporter(
            format!("http://{}{suffix}/", listener.local_addr().unwrap()),
            None,
        );
        let (captured, server) = capture_server(listener, &[200]);
        exporter.export(signal, &[fixture(name)]).unwrap();
        assert_eq!(captured.recv_timeout(CAPTURE_TIMEOUT).unwrap().0, suffix);
        assert_eq!(server.join().unwrap(), 1);
    }
}

#[test]
fn runtime_submission_rejects_blocking_calls_inside_tokio() {
    let exporter = submission_exporter("http://127.0.0.1:1".into(), None);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for (signal, name) in [(Signal::Logs, "logs"), (Signal::Profiles, "profiles")] {
            let error = exporter.export(signal, &[fixture(name)]).unwrap_err();
            assert!(matches!(
                error,
                SubmissionExportFailure::Retryable(
                    ExportError::BlockingBackendInAsyncContext { .. }
                )
            ));
        }
    });
}
