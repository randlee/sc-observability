//! Conversion, limit and failure-projection coverage for the Python send path.
use super::*;
use sc_observability_otlp::constants::{MAX_BATCH_RECORDS, MAX_INPUT_BYTES};
use sc_observability_otlp::sdk::error::OTelSdkError;
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
            start_time_unix_nano: Some(1),
            end_time_unix_nano: Some(2),
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
            0,
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
        for source in [
            c"[('k', None)]",
            c"[('k', [1])]",
            c"[('k', {'a': 1})]",
            c"[('k', 2**63)]",
        ] {
            let result = attributes(python_attributes(py, source), 0);
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
        assert!(attributes(eval(at_limit), 0).is_ok());
        assert!(attributes(eval(long_value.clone()), 0).is_ok());
        for result in [
            attributes(eval(over_limit), 0),
            attributes(eval(long_value), 1),
            attributes(Vec::new(), MAX_INPUT_BYTES + 1),
        ] {
            assert_eq!(code(result), codes::INPUT_LIMIT_EXCEEDED);
        }
    });
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
fn random_ids_are_nonzero_and_distinct() {
    let first = random();
    assert_ne!(first, 0);
    assert_ne!(first, random());
}
