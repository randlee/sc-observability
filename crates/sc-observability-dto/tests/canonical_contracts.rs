//! Canonical error and signal conversions remain lossless at the neutral boundary.
use sc_observability_dto::*;
use sc_observability_types::{self as core, v2};
use serde_json::{Value, json};

fn fixture(name: &str) -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../bindings/conformance/v1/schema-cases.json"
    ))
    .unwrap();
    cases
        .into_iter()
        .find(|c| c["id"] == format!("phase-d-Input{name}"))
        .unwrap()["value"]
        .clone()
}
fn context() -> Box<core::ErrorContext> {
    Box::new(
        core::ErrorContext::new(
            core::error_codes::otlp::OTLP_QUEUE_FULL,
            "queue full",
            core::Remediation::recoverable("retry later", ["inspect health"]),
        )
        .docs("https://example.test/recovery")
        .cause("bounded cause")
        .detail("depth", json!(u64::MAX))
        .source(Box::new(std::io::Error::other("native source not on wire"))),
    )
}
#[test]
fn canonical_error_projection_preserves_context() {
    let error = v2::ExportError::QueueFull { context: context() };
    let pointer = std::ptr::from_ref(error.context());
    let projected = CanonicalFailureDto::try_from(&error).unwrap();
    let d = projected.diagnostic();
    assert_eq!(d.diagnostic.code, "OTLP_QUEUE_FULL");
    assert_eq!(
        d.diagnostic.remediation,
        error.diagnostic().remediation.clone().into()
    );
    assert_eq!(d.docs.as_deref(), Some("https://example.test/recovery"));
    assert_eq!(d.cause.as_deref(), Some("bounded cause"));
    assert_eq!(
        serde_json::to_value(&d.details).unwrap()["depth"]["value"],
        u64::MAX.to_string()
    );
    assert_eq!(std::ptr::from_ref(error.context()), pointer);
    assert!(
        std::error::Error::source(error.context())
            .unwrap()
            .is::<std::io::Error>()
    );
    let wire = serde_json::to_value(CanonicalWireEnvelope::<AdmissionDto>::Error {
        schema_version: 1,
        error: projected,
    })
    .unwrap();
    assert!(!wire.to_string().contains("native source not on wire"));
    assert!(matches!(
        decode_canonical_envelope::<AdmissionDto>(wire.clone()).unwrap(),
        CanonicalWireEnvelope::Error {
            error: CanonicalFailureDto::QueueFull { .. },
            ..
        }
    ));
    assert!(matches!(
        decode_envelope::<AdmissionDto>(wire).unwrap(),
        WireEnvelope::Error {
            error: Failure::QueueFull { .. },
            ..
        }
    ));
}
#[test]
fn histogram_conversion_is_lossless() {
    let wire = fixture("MetricRecordDto");
    let record = decode_metric(wire.clone()).unwrap();
    match record.value() {
        v2::MetricValue::Histogram {
            point,
            temporality,
            start_time,
        } => {
            assert_eq!(point.count(), u64::MAX);
            assert_eq!(point.bucket_counts(), [1, 2, u64::MAX - 3]);
            assert_eq!(point.explicit_bounds(), [1.0, 2.0]);
            assert_eq!(point.sum().get(), 12.0);
            assert_eq!(*temporality, v2::AggregationTemporality::Delta);
            assert_eq!(*start_time, core::Timestamp::UNIX_EPOCH);
        }
        _ => panic!("expected histogram"),
    }
    let dto = MetricRecordDto::try_from(&record).unwrap();
    assert_eq!(serde_json::to_value(dto).unwrap(), wire);
}
#[test]
fn invalid_histogram_rejected() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../bindings/conformance/v1/conversion-cases.json"
    ))
    .unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|c| c["operation"] == "metric") {
        let error = decode_metric(case["value"].clone()).unwrap_err();
        assert_eq!(error.diagnostic().code, case["code"].as_str().unwrap());
        count += 1;
    }
    assert!(count >= 6);
}
#[test]
fn span_flags_links_duration_and_typestate_round_trip() {
    let wire = fixture("SpanSignalDto");
    let span = decode_span(wire.clone()).unwrap();
    if let v2::SpanSignal::Ended(record) = &span {
        assert_eq!(record.duration_ms().unwrap().as_u64(), u64::MAX);
        assert_eq!(record.trace().flags.bits(), 131);
        assert_eq!(record.links()[0].flags.bits(), 131);
        assert_eq!(record.kind(), v2::SpanKind::Server);
    } else {
        panic!("expected ended span");
    }
    assert_eq!(
        serde_json::to_value(SpanSignalDto::try_from(&span).unwrap()).unwrap(),
        wire
    );
    let mut bad = wire;
    bad["Ended"]["duration_ms"] = Value::Null;
    assert!(decode_span(bad).is_err());
}
#[test]
fn unknown_errors_remain_tagged_failures() {
    let mut wire = fixture("CanonicalWireEnvelopeAdmissionDto");
    wire["error"]["kind"] = json!("future_error");
    let envelope = decode_canonical_envelope::<AdmissionDto>(wire).unwrap();
    match envelope {
        CanonicalWireEnvelope::Error {
            error:
                CanonicalFailureDto::UnknownRemote {
                    diagnostic,
                    remote_kind,
                },
            ..
        } => {
            assert_eq!(remote_kind, "future_error");
            assert_eq!(diagnostic.diagnostic.code, "OTLP_QUEUE_FULL");
            assert_eq!(diagnostic.cause.as_deref(), Some("bounded cause"));
        }
        _ => panic!("unknown error was not retained"),
    }
}
