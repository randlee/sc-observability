//! Conversion, limit and failure-projection coverage for the Python send path.
use super::*;
use sc_observability_otlp::constants::{MAX_BATCH_RECORDS, MAX_INPUT_BYTES};
use sc_observability_otlp::sdk::error::OTelSdkError;
use serde_json::Value;
use std::ffi::{CStr, CString};

fn config(timeout_s: Option<f64>, root_certificate: Option<PathBuf>) -> Config {
    Config {
        endpoint: Some("http://127.0.0.1:9".into()),
        headers: Vec::new(),
        timeout_s,
        root_certificate,
        service_name: None,
    }
}

/// The registry code of an expected failure.
fn code<T>(result: Result<T, SyncError>) -> String {
    failure("send_log", &expect_err(result))
        .diagnostic()
        .code
        .clone()
}

fn log_fields<'py>(
    severity: &str,
    trace_id: Option<&str>,
    span_id: Option<&str>,
) -> LogFields<'py> {
    LogFields {
        body: "body".into(),
        severity: severity.into(),
        trace_id: trace_id.map(Into::into),
        span_id: span_id.map(Into::into),
        attributes: Vec::new(),
    }
}

fn expect_err<T>(result: Result<T, SyncError>) -> SyncError {
    match result {
        Ok(_) => std::panic::panic_any("expected a validation failure"),
        Err(error) => error,
    }
}

#[test]
fn root_certificate_uses_the_shared_bounded_regular_file_reader() {
    let directory = std::env::temp_dir().join(format!(
        "sc-observability-py-root-certificate-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).expect("create temporary certificate directory");
    let oversized = directory.join("oversized.pem");
    std::fs::write(&oversized, vec![b'x'; MAX_INPUT_BYTES + 1])
        .expect("write oversized root certificate");
    assert_eq!(
        code(client(&config(None, Some(oversized))).map(drop)),
        codes::INVALID_CONFIG
    );

    let non_regular = directory.join("certificate-directory");
    std::fs::create_dir(&non_regular).expect("create certificate directory");
    assert_eq!(
        code(client(&config(None, Some(non_regular))).map(drop)),
        codes::INVALID_CONFIG
    );
    std::fs::remove_dir_all(directory).expect("remove temporary certificate directory");
}

#[test]
fn failures_project_to_the_shared_failure_union_with_registry_codes() {
    let Failure::Validation { diagnostic, field } = failure(
        "send_log",
        &SyncError::validation(codes::INVALID_RECORD, "bad"),
    ) else {
        std::panic::panic_any("record rejection must be a validation failure");
    };
    assert_eq!(
        (diagnostic.code.as_str(), field.as_str()),
        (codes::INVALID_RECORD, "fields")
    );
    assert!(diagnostic.message.contains("bad"));
    let Failure::Validation { field, .. } = failure(
        "send_log",
        &SyncError::validation(codes::INVALID_CONFIG, "bad"),
    ) else {
        std::panic::panic_any("config rejection must be a validation failure");
    };
    assert_eq!(field, "config");
    let Failure::Unavailable { diagnostic } = failure(
        "send_span",
        &SyncError::Export(OTelSdkError::InternalFailure("refused".into())),
    ) else {
        std::panic::panic_any("a refused export must be unavailable");
    };
    assert_eq!(diagnostic.code, TELEMETRY_EXPORT_FAILED.as_str());
    assert!(diagnostic.message.contains("refused"));
    let Failure::Timeout {
        diagnostic,
        operation,
    } = failure(
        "send_metric",
        &SyncError::Export(OTelSdkError::Timeout(Duration::from_secs(3))),
    )
    else {
        std::panic::panic_any("an exporter deadline must be a timeout");
    };
    assert_eq!(
        (diagnostic.code.as_str(), operation.as_str()),
        (TELEMETRY_EXPORT_FAILED.as_str(), "send_metric")
    );
}

#[test]
fn log_and_span_field_combinations_are_validated() {
    Python::initialize();
    let trace = "4bf92f3577b34da6a3ce929d0e0e4736";
    for result in [
        log(log_fields("loud", None, None)).map(drop),
        log(log_fields("info", Some(trace), None)).map(drop),
        log(log_fields("info", None, Some("00f067aa0ba902b7"))).map(drop),
    ] {
        assert_eq!(code(result), codes::INVALID_RECORD);
    }
    assert!(log(log_fields("warn", Some(trace), Some("00f067aa0ba902b7"))).is_ok());
    let span_fields =
        |kind: &str, parent: Option<&str>, ok: bool, error: Option<&str>| SpanFields {
            name: "span".into(),
            trace_id: None,
            span_id: None,
            parent_span_id: parent.map(Into::into),
            kind: kind.into(),
            start_time_unix_nano: None,
            end_time_unix_nano: None,
            ok,
            error: error.map(Into::into),
            attributes: Vec::new(),
        };
    for result in [
        span(span_fields("sideways", None, false, None)).map(drop),
        span(span_fields("client", Some("00f067aa0ba902b7"), false, None)).map(drop),
        span(span_fields("client", None, true, Some("boom"))).map(drop),
    ] {
        assert_eq!(code(result), codes::INVALID_RECORD);
    }
    let generated = match span(span_fields("server", None, false, Some("boom"))) {
        Ok(span) => span,
        Err(error) => std::panic::panic_any(error.to_string()),
    };
    assert!(generated.span_context.is_valid());
    assert_eq!(generated.span_kind, SpanKind::Server);
    assert_eq!(generated.status, Status::error("boom"));
    assert_eq!(generated.parent_span_id, SpanId::INVALID);
}

#[test]
fn span_times_outside_the_unsigned_64_bit_range_are_validation_failures() {
    Python::initialize();
    Python::attach(|py| {
        let int = |source: &CStr| {
            py.eval(source, None, None)
                .expect("valid int")
                .cast_into::<PyInt>()
                .expect("an int")
        };
        assert!(unix_nanos("start_time_unix_nano", &int(c"0")).is_ok());
        assert!(unix_nanos("end_time_unix_nano", &int(c"2**64 - 1")).is_ok());
        for source in [c"-1", c"2**64"] {
            assert_eq!(
                code(unix_nanos("end_time_unix_nano", &int(source))),
                codes::INVALID_RECORD
            );
        }
    });
}

#[test]
fn a_parent_is_local_and_start_after_end_is_rejected() {
    Python::initialize();
    Python::attach(|py| {
        let int = |source: &CStr| {
            py.eval(source, None, None)
                .expect("valid int")
                .cast_into::<PyInt>()
                .expect("an int")
        };
        let fields = |start: &CStr, end: &CStr, parent: Option<&str>| SpanFields {
            name: "span".into(),
            trace_id: Some("4bf92f3577b34da6a3ce929d0e0e4736".into()),
            span_id: None,
            parent_span_id: parent.map(Into::into),
            kind: "internal".into(),
            start_time_unix_nano: Some(int(start)),
            end_time_unix_nano: Some(int(end)),
            ok: false,
            error: None,
            attributes: Vec::new(),
        };
        let child = match span(fields(c"1", c"2", Some("00f067aa0ba902b7"))) {
            Ok(span) => span,
            Err(error) => std::panic::panic_any(error.to_string()),
        };
        assert_eq!(child.parent_span_id, SpanId::from(0x00f0_67aa_0ba9_02b7));
        assert!(!child.parent_span_is_remote);
        assert!(span(fields(c"2", c"2", None)).is_ok());
        assert_eq!(
            code(span(fields(c"3", c"2", None)).map(drop)),
            codes::INVALID_RECORD
        );
    });
}

fn python_attributes<'py>(py: Python<'py>, source: &CStr) -> Vec<(String, Bound<'py, PyAny>)> {
    py.eval(source, None, None)
        .and_then(|value| value.extract())
        .expect("valid attribute list")
}

#[test]
fn attributes_map_python_scalars_and_reject_other_values() {
    Python::initialize();
    Python::attach(|py| {
        let converted = attributes(
            python_attributes(
                py,
                c"[('flag', True), ('count', 3), ('ratio', 0.5), ('name', 'x')]",
            ),
            "",
        )
        .expect("supported scalars");
        assert_eq!(
            converted,
            [
                ("flag".into(), Scalar::Bool(true)),
                ("count".into(), Scalar::Int(3)),
                ("ratio".into(), Scalar::Float(0.5)),
                ("name".into(), Scalar::Str("x".into())),
            ]
        );
        assert_eq!(
            attributes(python_attributes(py, c"[('huge', 2**63)]"), "")
                .expect("u64 above i64::MAX converts"),
            [("huge".into(), Scalar::Str("9223372036854775808".into()),)]
        );
        assert_eq!(
            attributes(
                python_attributes(py, c"[('big', 1.5e19), ('low', -(2**63))]"),
                ""
            )
            .expect("large float and i64::MIN stay accepted"),
            [
                ("big".into(), Scalar::Float(1.5e19)),
                ("low".into(), Scalar::Int(i64::MIN)),
            ]
        );
        for source in [
            c"[('k', None)]",
            c"[('k', [1])]",
            c"[('k', {'a': 1})]",
            c"[('k', 2**64)]",
            c"[('k', -(2**63) - 1)]",
        ] {
            let result = attributes(python_attributes(py, source), "");
            assert_eq!(code(result), codes::INVALID_RECORD);
        }
    });
}

#[test]
fn input_limits_count_attributes_and_text_bytes() {
    Python::initialize();
    Python::attach(|py| {
        let at_limit = format!("[('k' + str(i), 1) for i in range({MAX_BATCH_RECORDS})]");
        let over_limit = format!(
            "[('k' + str(i), 1) for i in range({})]",
            MAX_BATCH_RECORDS + 1
        );
        let long_value = format!("[('k', 'x' * {})]", MAX_INPUT_BYTES - 1);
        let eval =
            |source: String| python_attributes(py, &CString::new(source).expect("no interior NUL"));
        assert!(attributes(eval(at_limit), "").is_ok());
        assert!(attributes(eval(long_value.clone()), "").is_ok());
        for result in [
            attributes(eval(over_limit), ""),
            attributes(eval(long_value), "x"),
            attributes(Vec::new(), &"x".repeat(MAX_INPUT_BYTES + 1)),
        ] {
            assert_eq!(code(result), codes::INPUT_LIMIT_EXCEEDED);
        }
        let long_key = format!("[('k' * {MAX_INPUT_BYTES}, True)]");
        assert_eq!(
            code(attributes(eval(long_key), "")),
            codes::INPUT_LIMIT_EXCEEDED
        );
    });
}

#[test]
fn oversized_span_name_is_an_input_limit_failure() {
    let result = span(SpanFields {
        name: "x".repeat(MAX_INPUT_BYTES + 1),
        trace_id: None,
        span_id: None,
        parent_span_id: None,
        kind: "internal".into(),
        start_time_unix_nano: None,
        end_time_unix_nano: None,
        ok: false,
        error: None,
        attributes: Vec::new(),
    });
    assert_eq!(code(result), codes::INPUT_LIMIT_EXCEEDED);
}

#[test]
fn invalid_timeout_and_unreadable_certificate_are_config_failures_before_export() {
    let signal = || {
        Signal::Metric(Metric {
            name: "jobs".into(),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: None,
            description: None,
            attributes: Vec::new(),
        })
    };
    for config in [
        config(Some(f64::NAN), None),
        config(Some(-1.0), None),
        config(Some(0.0), None),
        config(
            None,
            Some(PathBuf::from("/nonexistent/sc-observability-ca.pem")),
        ),
    ] {
        assert_eq!(code(export(config, signal())), codes::INVALID_CONFIG);
    }
}

#[test]
fn generated_span_ids_come_from_the_sdk_generator_and_are_distinct() {
    Python::initialize();
    let fields = || SpanFields {
        name: "span".into(),
        trace_id: None,
        span_id: None,
        parent_span_id: None,
        kind: "internal".into(),
        start_time_unix_nano: None,
        end_time_unix_nano: None,
        ok: false,
        error: None,
        attributes: Vec::new(),
    };
    let mut traces = std::collections::HashSet::new();
    let mut spans = std::collections::HashSet::new();
    for _ in 0..1000 {
        let generated = match span(fields()) {
            Ok(span) => span,
            Err(error) => std::panic::panic_any(error.to_string()),
        };
        assert!(generated.span_context.is_valid());
        assert!(traces.insert(generated.span_context.trace_id()));
        assert!(spans.insert(generated.span_context.span_id()));
    }
}

/// Bounds every collector wait so a broken send cannot hang the suite.
const WATCHDOG: Duration = Duration::from_secs(30);

/// One request as the loopback collector received it.
struct Request {
    path: String,
    head: String,
    body: Vec<u8>,
}

/// Accepts one connection on a loopback port, reports the request, waits for
/// `release` and answers with `status` and `reply` as the body.
fn collector(
    status: u16,
    reply: &'static str,
) -> (
    String,
    std::sync::mpsc::Receiver<Request>,
    std::sync::mpsc::Sender<()>,
) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
    let endpoint = format!(
        "http://{}",
        listener.local_addr().expect("collector address")
    );
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept export");
        stream
            .set_read_timeout(Some(WATCHDOG))
            .expect("collector read timeout");
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        let header_end = loop {
            if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
            let read = stream.read(&mut chunk).expect("read request head");
            assert_ne!(read, 0, "connection closed before the request head");
            buffer.extend_from_slice(&chunk[..read]);
        };
        let head = String::from_utf8_lossy(&buffer[..header_end]).to_lowercase();
        let length = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = buffer[header_end..].to_vec();
        while body.len() < length {
            let read = stream.read(&mut chunk).expect("read request body");
            assert_ne!(read, 0, "connection closed before the request body");
            body.extend_from_slice(&chunk[..read]);
        }
        let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
        let _ = request_tx.send(Request { path, head, body });
        let _ = release_rx.recv_timeout(WATCHDOG);
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
            reply.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    (endpoint, request_rx, release_tx)
}

fn loopback(endpoint: String, headers: &[(&str, &str)]) -> Config {
    Config {
        endpoint: Some(endpoint),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect(),
        timeout_s: Some(WATCHDOG.as_secs_f64() / 2.0),
        root_certificate: None,
        service_name: Some("py-native-service".into()),
    }
}

fn envelope(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("ResultDto JSON envelope")
}

#[test]
fn send_log_forwards_fields_and_headers_on_the_wire() {
    const TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
    const SPAN: &str = "00f067aa0ba902b7";
    let (endpoint, requests, release) = collector(200, "");
    release.send(()).expect("pre-release collector");
    Python::initialize();
    let result = Python::attach(|py| {
        let mut fields = log_fields("error", Some(TRACE), Some(SPAN));
        fields.body = "job failed".into();
        fields.attributes = python_attributes(py, c"[('job', 'build'), ('ratio', 0.25)]");
        send_log(
            py,
            loopback(endpoint, &[("authorization", "explicit")]),
            fields,
        )
    });
    assert_eq!(envelope(&result)["kind"], "ok", "{result}");
    let request = requests.recv_timeout(WATCHDOG).expect("exported request");
    assert_eq!(request.path, "/v1/logs");
    assert!(
        request.head.contains("authorization: explicit"),
        "{}",
        request.head
    );
    let trace = u128::from_str_radix(TRACE, 16).expect("hex").to_be_bytes();
    let span = u64::from_str_radix(SPAN, 16).expect("hex").to_be_bytes();
    for expected in [
        &b"job failed"[..],
        b"ERROR",
        b"py-native-service",
        b"sc_observability",
        b"job",
        b"build",
        &trace,
        &span,
        &0.25_f64.to_le_bytes(),
    ] {
        assert!(
            request.body.windows(expected.len()).any(|w| w == expected),
            "{expected:?} missing from the exported body"
        );
    }
}

#[test]
fn rejected_export_message_is_redacted() {
    const SECRET: &str = "header-secret-value";
    const PASSWORD: &str = "userinfo-password";
    // The collector echoes the credential back in its error body; whatever the
    // client keeps of the rejection, the projected envelope must not carry
    // the header value or the endpoint's user information.
    let (endpoint, requests, release) = collector(401, "rejected header-secret-value");
    release.send(()).expect("pre-release collector");
    let endpoint = endpoint.replace("http://", &format!("http://otlp-user:{PASSWORD}@"));
    Python::initialize();
    let result = Python::attach(|py| {
        send_log(
            py,
            loopback(endpoint, &[("authorization", SECRET)]),
            log_fields("info", None, None),
        )
    });
    let error = &envelope(&result)["error"];
    assert_eq!(
        (&error["kind"], &error["code"]),
        (
            &serde_json::json!("unavailable"),
            &serde_json::json!(TELEMETRY_EXPORT_FAILED.as_str())
        ),
        "{result}"
    );
    assert!(
        error["message"].as_str().is_some_and(|m| m.contains("401")),
        "{result}"
    );
    for leaked in [SECRET, PASSWORD, "otlp-user"] {
        assert!(!result.contains(leaked), "{leaked} leaked: {result}");
    }
    let request = requests.recv_timeout(WATCHDOG).expect("exported request");
    assert!(
        request.head.contains(&format!("authorization: {SECRET}")),
        "the credential is still sent to the collector"
    );
}

#[test]
fn a_blocked_export_releases_the_gil() {
    // The collector answers only after this thread has run Python code. A
    // send holding the GIL blocks that until the client timeout, and the
    // export then fails instead of succeeding.
    let (endpoint, requests, release) = collector(200, "");
    Python::initialize();
    let sender = std::thread::spawn(move || {
        Python::attach(|py| send_log(py, loopback(endpoint, &[]), log_fields("info", None, None)))
    });
    requests.recv_timeout(WATCHDOG).expect("exported request");
    let ran = Python::attach(|py| {
        py.eval(c"6 * 7", None, None)
            .and_then(|value| value.extract::<i64>())
            .expect("Python evaluates while the send is blocked")
    });
    assert_eq!(ran, 42);
    release.send(()).expect("release collector");
    let result = sender.join().expect("send thread");
    assert_eq!(envelope(&result)["kind"], "ok", "{result}");
}

#[test]
fn panic_payload_does_not_reach_python_failure() {
    Python::initialize();
    Python::attach(|py| {
        let payload = "Authorization: Bearer top-secret";
        let result = run(py, config(None, None), "send_log", || {
            std::panic::panic_any(payload)
        });
        let envelope: Value = serde_json::from_str(&result).expect("valid failure envelope");
        assert_eq!(envelope["kind"], "error");
        assert_eq!(envelope["error"]["kind"], "internal");
        assert_eq!(
            envelope["error"]["code"],
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL
        );
        assert_eq!(
            envelope["error"]["message"],
            "native telemetry call panicked"
        );
        assert!(!result.contains(payload));
    });
}
