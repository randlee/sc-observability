//! Canonical error and signal conversions, with deferred limitations named below.
use sc_observability_dto::*;
use sc_observability_types::{self as core, v2};
use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
use serde_json::{Value, json};
use std::{cell::RefCell, fmt};

thread_local! {
    static SERDE_UNKNOWN_VARIANT_EXPECTED: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
struct CapturedUnknownVariant;

impl fmt::Display for CapturedUnknownVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("captured unknown variant")
    }
}

impl std::error::Error for CapturedUnknownVariant {}

impl de::Error for CapturedUnknownVariant {
    fn custom<T: fmt::Display>(_message: T) -> Self {
        Self
    }

    fn unknown_variant(_variant: &str, expected: &'static [&'static str]) -> Self {
        SERDE_UNKNOWN_VARIANT_EXPECTED.with(|captured| {
            captured
                .borrow_mut()
                .push(expected.iter().map(|tag| (*tag).to_owned()).collect());
        });
        Self
    }
}

struct UnknownFailureKindDeserializer;

impl<'de> de::Deserializer<'de> for UnknownFailureKindDeserializer {
    type Error = CapturedUnknownVariant;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(UnknownFailureKindMap { emitted_key: false })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

struct UnknownFailureKindMap {
    emitted_key: bool,
}

impl<'de> MapAccess<'de> for UnknownFailureKindMap {
    type Error = CapturedUnknownVariant;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.emitted_key {
            return Ok(None);
        }
        self.emitted_key = true;
        seed.deserialize(de::value::StrDeserializer::new("kind"))
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::StrDeserializer::new("future_failure_kind"))
    }
}

fn serde_generated_failure_kinds<T>() -> Vec<String>
where
    T: serde::de::DeserializeOwned,
{
    SERDE_UNKNOWN_VARIANT_EXPECTED.with(|captured| captured.borrow_mut().clear());
    assert!(T::deserialize(UnknownFailureKindDeserializer).is_err());
    SERDE_UNKNOWN_VARIANT_EXPECTED.with(|captured| {
        let mut captures = std::mem::take(&mut *captured.borrow_mut());
        assert_eq!(
            captures.len(),
            1,
            "expected one serde unknown-variant capture"
        );
        captures.pop().unwrap()
    })
}

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
fn shared_failure_shape_preserves_each_diagnostic_contract_losslessly() {
    let legacy_wire = json!({
        "kind": "queue_full",
        "at": "2024-01-01T00:00:00Z",
        "code": "OTLP_QUEUE_FULL",
        "message": "queue full",
        "remediation": { "kind": "recoverable", "steps": ["retry later"] }
    });
    let canonical_wire = json!({
        "kind": "queue_full",
        "at": "2024-01-01T00:00:00Z",
        "code": "OTLP_QUEUE_FULL",
        "message": "queue full",
        "remediation": { "kind": "recoverable", "steps": ["retry later"] },
        "cause": "bounded producer queue",
        "docs": "https://example.test/queue-full",
        "details": { "capacity": { "kind": "integer", "value": "64" } }
    });

    let legacy: Failure = serde_json::from_value(legacy_wire.clone()).unwrap();
    let canonical: CanonicalFailureDto = serde_json::from_value(canonical_wire.clone()).unwrap();

    assert_eq!(serde_json::to_value(legacy).unwrap(), legacy_wire);
    assert_eq!(serde_json::to_value(&canonical).unwrap(), canonical_wire);
    assert_eq!(
        canonical.diagnostic().cause.as_deref(),
        Some("bounded producer queue")
    );
    assert_eq!(
        canonical.diagnostic().docs.as_deref(),
        Some("https://example.test/queue-full")
    );
    assert!(canonical.diagnostic().details.contains_key("capacity"));
}
#[test]
fn histogram_conversion_is_lossless() {
    let mut wire = fixture("MetricRecordDto");
    // Attribute projection is deferred separately; retain full histogram coverage.
    wire["attributes"] = json!({});
    let record = decode_metric(wire.clone()).unwrap();
    match record.value() {
        v2::MetricValue::Histogram {
            point,
            temporality,
            start_time,
        } => {
            assert_eq!(point.count(), u64::MAX);
            assert_eq!(point.bucket_counts(), [1, 2, u64::MAX - 3]);
            assert_eq!(
                point
                    .explicit_bounds()
                    .iter()
                    .map(|v| v.get())
                    .collect::<Vec<_>>(),
                [1.0, 2.0]
            );
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
#[ignore = "obs-dto-attribute-projection: attributed DTO round trips deferred by user; see docs/plans/phase-d/known-limitations.md"]
fn metric_attributes_round_trip() {
    let wire = fixture("MetricRecordDto");
    let record = decode_metric(wire.clone()).unwrap();
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
        assert_eq!(record.duration_ms().as_u64(), u64::MAX);
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
    bad["data"]["duration_ms"] = Value::Null;
    assert!(decode_span(bad).is_err());
}

#[test]
fn span_signal_wire_variants_use_adjacent_tags() {
    let ended = fixture("SpanSignalDto");
    let mut started = ended.clone();
    started["kind"] = json!("started");
    started["data"]["duration_ms"] = Value::Null;
    started["data"]["status"] = json!("Unset");
    let event = json!({
        "kind": "event",
        "data": {
            "timestamp": "1970-01-01T00:00:00Z",
            "trace": {
                "trace_id": "0123456789abcdef0123456789abcdef",
                "span_id": "0123456789abcdef",
                "parent_span_id": null,
                "flags": 131
            },
            "name": "event",
            "attributes": {},
            "diagnostic": null
        }
    });

    for wire in [started, event, ended] {
        let signal: SpanSignalDto = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(signal).unwrap(), wire);
    }
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

#[test]
fn canonical_envelope_decodes_each_declared_failure_kind() {
    let diagnostic: CanonicalDiagnosticDto = serde_json::from_value(json!({
        "at": "2024-01-01T00:00:00Z",
        "code": "TEST_FAILURE",
        "message": "test failure",
        "remediation": { "kind": "recoverable", "steps": ["retry"] },
        "cause": "bounded cause",
        "docs": "https://example.test/failure",
        "details": {}
    }))
    .unwrap();
    let failures = vec![
        CanonicalFailureDto::Validation {
            diagnostic: Box::new(diagnostic.clone()),
            field: "field".into(),
        },
        CanonicalFailureDto::QueueFull {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::BelowBaseline {
            diagnostic: Box::new(diagnostic.clone()),
            requested: LevelFilterDto::Trace,
            configured: LevelFilterDto::Info,
        },
        CanonicalFailureDto::UnsupportedLevel {
            diagnostic: Box::new(diagnostic.clone()),
            requested: LevelFilterDto::Trace,
            available: LevelFilterDto::Info,
        },
        CanonicalFailureDto::PermissionDenied {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::Closed {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::Unavailable {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::Io {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::Timeout {
            diagnostic: Box::new(diagnostic.clone()),
            operation: "flush".into(),
        },
        CanonicalFailureDto::Cancelled {
            diagnostic: Box::new(diagnostic.clone()),
            operation: "flush".into(),
        },
        CanonicalFailureDto::UnsupportedVersion {
            diagnostic: Box::new(diagnostic.clone()),
            received: 2,
        },
        CanonicalFailureDto::Internal {
            diagnostic: Box::new(diagnostic.clone()),
        },
        CanonicalFailureDto::UnknownRemote {
            diagnostic: Box::new(diagnostic),
            remote_kind: "future_failure".into(),
        },
    ];

    for error in failures {
        let expected = CanonicalWireEnvelope::<AdmissionDto>::Error {
            schema_version: 1,
            error,
        };
        assert_eq!(
            decode_canonical_envelope(serde_json::to_value(&expected).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn serde_generated_failure_kinds_match_known_kinds() {
    let mut known_kinds = CanonicalFailureDto::KNOWN_KINDS
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect::<Vec<_>>();
    known_kinds.sort_unstable();

    let mut legacy_kinds = serde_generated_failure_kinds::<Failure<Diagnostic>>();
    legacy_kinds.sort_unstable();
    assert_eq!(legacy_kinds, known_kinds);

    let mut canonical_kinds = serde_generated_failure_kinds::<CanonicalFailureDto>();
    canonical_kinds.sort_unstable();
    assert_eq!(canonical_kinds, known_kinds);
}

#[test]
fn nested_export_timeout_keeps_lifecycle_category_and_code() {
    let code = core::error_codes::otlp::OTLP_LIFECYCLE_TIMEOUT;
    let context = || {
        Box::new(core::ErrorContext::new(
            code.clone(),
            "deadline elapsed",
            core::Remediation::recoverable("inspect health", [] as [&str; 0]),
        ))
    };
    let nested = v2::ExportError::LifecycleTimeout { context: context() };
    let error = v2::FlushError::Drain {
        context: Box::new(
            core::ErrorContext::new(
                code.clone(),
                "flush failed",
                core::Remediation::recoverable("inspect health", [] as [&str; 0]),
            )
            .source(Box::new(nested)),
        ),
    };
    let wire = CanonicalFailureDto::try_from(&error).unwrap();
    assert!(matches!(wire, CanonicalFailureDto::Timeout { .. }));
    assert_eq!(wire.diagnostic().diagnostic.code, code.as_str());
}

fn envelope_failure(wire: Value) -> (String, String, String) {
    match decode_canonical_envelope::<AdmissionDto>(wire) {
        Err(Failure::Validation { diagnostic, field }) => {
            (diagnostic.code, field, diagnostic.message)
        }
        other => panic!("expected validation failure, got {other:?}"),
    }
}

#[test]
fn canonical_envelope_error_arm_follows_binding_contract() {
    // binding-contract.md: malformed envelope -> INVALID_INPUT at `response`;
    // oversized diagnostic -> DIAGNOSTIC_TOO_LARGE at `response.error`, checked before timestamp.
    let wire = fixture("CanonicalWireEnvelopeAdmissionDto");
    let too_large = error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE;
    let invalid = error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT;
    let with = |edit: &dyn Fn(&mut Value)| {
        let mut w = wire.clone();
        edit(&mut w["error"]);
        envelope_failure(w)
    };
    for edit in [
        &(|e: &mut Value| e["message"] = json!("x".repeat(4097))) as &dyn Fn(&mut Value),
        &|e| e["at"] = json!("x".repeat(4097)),
        &|e| {
            e["message"] = json!("x".repeat(4097));
            e["at"] = json!("not-a-time");
        },
    ] {
        let (code, field, _) = with(edit);
        assert_eq!(
            (code.as_str(), field.as_str()),
            (too_large, "response.error")
        );
    }
    let (code, field, _) = with(&|e| e["at"] = json!("not-a-time"));
    assert_eq!((code.as_str(), field.as_str()), (invalid, "response.error"));
    let (code, field, message) = with(&|e| {
        e["at"] = json!("not-a-time");
        e["details"] = json!({"k": {"kind": "nope"}});
    });
    assert_eq!((code.as_str(), field.as_str()), (invalid, "response.error"));
    assert!(!message.contains("nope"), "{message}");
    for edit in [
        &(|e: &mut Value| drop(e.as_object_mut().unwrap().remove("message")))
            as &dyn Fn(&mut Value),
        &|e| {
            e["kind"] = json!("validation");
            e.as_object_mut().unwrap().remove("field");
        },
    ] {
        let (code, field, _) = with(edit);
        assert_eq!((code.as_str(), field.as_str()), (invalid, "response"));
    }
    // Canonical contract mapping, not base parity: these malformed-envelope
    // groups report `response` (base b6f54cb6 reported `response.error`).
    for (edit, needle) in [
        (
            &(|e: &mut Value| e["details"] = json!([1])) as &dyn Fn(&mut Value),
            "expected a map",
        ),
        (
            &|e| e["details"] = json!({"k": {"kind": "nope"}}),
            "unknown variant",
        ),
        (
            &|e| e["details"] = json!({"k": {"kind": "string", "value": "x".repeat(70_000)}}),
            "request exceeds",
        ),
    ] {
        let (code, field, message) = with(edit);
        assert_eq!((code.as_str(), field.as_str()), (invalid, "response"));
        assert!(message.contains(needle), "{message}");
    }
    for error in [Value::Null, json!("x")] {
        let mut w = wire.clone();
        w["error"] = error;
        let (code, field, message) = envelope_failure(w);
        assert_eq!((code.as_str(), field.as_str()), (invalid, "response"));
        assert!(message.contains("expected struct Diagnostic"), "{message}");
    }
}

#[test]
fn canonical_envelope_ok_value_is_not_request_bounded() {
    // binding-contract.md: request size and depth bounds do not limit output records.
    let deep = (0..40).fold(json!(1), |v, _| json!([v]));
    for value in [deep, json!("x".repeat(70_000))] {
        let wire = json!({"schema_version": 1, "kind": "ok", "value": value.clone()});
        match decode_canonical_envelope::<Value>(wire).unwrap() {
            CanonicalWireEnvelope::Ok { value: decoded, .. } => assert_eq!(decoded, value),
            other => panic!("expected ok, got {other:?}"),
        }
    }
}

#[test]
fn canonical_envelope_unknown_kind_keeps_canonical_metadata() {
    let mut wire = fixture("CanonicalWireEnvelopeAdmissionDto");
    wire["error"]["kind"] = json!("future_kind_xyz");
    wire["error"]["docs"] = json!("https://example.test/recovery");
    wire["error"]["details"] = json!({"k": {"kind": "string", "value": "v"}});
    match decode_canonical_envelope::<AdmissionDto>(wire.clone()).unwrap() {
        CanonicalWireEnvelope::Error {
            error:
                CanonicalFailureDto::UnknownRemote {
                    diagnostic,
                    remote_kind,
                },
            ..
        } => {
            assert_eq!(remote_kind, "future_kind_xyz");
            assert_eq!(
                diagnostic.docs.as_deref(),
                Some("https://example.test/recovery")
            );
            assert_eq!(
                serde_json::to_value(&diagnostic.details).unwrap(),
                wire["error"]["details"]
            );
        }
        other => panic!("expected unknown remote failure, got {other:?}"),
    }
}
