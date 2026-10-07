//! Contract tests for consumers of the staged canonical surface.
use sc_observability_types::v2::*;
use sc_observability_types::{
    DiagnosticInfo, ErrorContext, MetricUnit, Remediation, Timestamp, error_codes,
};
use serde_json::json;
use std::error::Error;

#[test]
fn attribute_integer_variants_survive_native_serde() {
    for original in [
        AttributeValue::UInt(5),
        AttributeValue::Int(5),
        AttributeValue::Int(0),
        AttributeValue::UInt(0),
        AttributeValue::Int(i64::MIN),
        AttributeValue::Int(i64::MAX),
        AttributeValue::UInt(u64::try_from(i64::MAX).unwrap()),
        AttributeValue::UInt(u64::MAX),
    ] {
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: AttributeValue = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, original, "variant lost in {encoded}");
        let encoded = serde_json::to_value(&original).unwrap();
        let decoded: AttributeValue = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, original);
    }
}

#[test]
fn attribute_tags_preserve_nested_values_in_metric_records() {
    let values = vec![
        AttributeValue::Bool(true),
        AttributeValue::Int(5),
        AttributeValue::UInt(5),
        AttributeValue::Float(finite(5.0)),
        AttributeValue::String("5".to_owned()),
        AttributeValue::Null,
    ];
    for (value, tag) in values
        .iter()
        .zip(["bool", "int", "uint", "float", "string", "null"])
    {
        let encoded = serde_json::to_value(value).unwrap();
        assert_eq!(encoded["kind"], tag);
        if tag == "null" {
            assert_eq!(encoded, json!({"kind": "null"}));
        }
    }
    assert_eq!(
        serde_json::to_value(AttributeValue::UInt(5)).unwrap(),
        json!({"kind": "uint", "data": 5})
    );
    assert_ne!(AttributeValue::Int(5), AttributeValue::UInt(5));
    let object = AttributeValue::Object(Attributes::from([
        ("kind".to_owned(), AttributeValue::String("uint".to_owned())),
        ("data".to_owned(), AttributeValue::Array(values)),
    ]));
    let record = MetricRecord::try_new(
        Timestamp::UNIX_EPOCH,
        sc_observability_types::ServiceName::new("test").unwrap(),
        sc_observability_types::MetricName::new("gauge").unwrap(),
        MetricValue::Gauge(finite(1.0)),
    )
    .unwrap()
    .with_attributes(Attributes::from([("nested".to_owned(), object)]));
    let encoded = serde_json::to_string(&record).unwrap();
    let decoded: MetricRecord = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, record);
}

#[test]
fn attribute_serde_rejects_ambiguous_or_invalid_integer_payloads() {
    for encoded in [
        "5",
        r#"{"kind":"uint","data":-1}"#,
        r#"{"kind":"uint","data":18446744073709551616}"#,
        r#"{"kind":"int","data":9223372036854775808}"#,
        r#"{"kind":"int","data":1.5}"#,
        r#"{"kind":"uint","data":"5"}"#,
        r#"{"kind":"int"}"#,
        r#"{"kind":"unknown","data":5}"#,
    ] {
        assert!(
            serde_json::from_str::<AttributeValue>(encoded).is_err(),
            "accepted {encoded}"
        );
    }
}

#[derive(Debug, thiserror::Error)]
#[error("sentinel source")]
struct Sentinel(u64);

fn context(code: sc_observability_types::ErrorCode) -> Box<ErrorContext> {
    Box::new(
        ErrorContext::new(
            code,
            "failure",
            Remediation::recoverable("correct input", ["retry"]),
        )
        .docs("https://example.test/recovery")
        .cause("bounded cause")
        .detail("config_field", json!("queue_capacity"))
        .source(Box::new(Sentinel(42))),
    )
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive inventory proves the same context contract for every variant"
)]
fn canonical_error_variants_preserve_context() {
    macro_rules! check {
        ($name:ident::$variant:ident, $code:expr) => {{ check!($name::$variant, error_codes::VALUE_VALIDATION_FAILED, $code) }};
        ($name:ident::$variant:ident, $context_code:expr, $code:expr) => {{
            let original = context($context_code);
            let diagnostic = original.diagnostic().clone();
            let pointer = std::ptr::from_ref(&*original);
            let error = $name::$variant { context: original };
            assert_eq!(std::ptr::from_ref(error.context()), pointer);
            assert_eq!(DiagnosticInfo::diagnostic(&error), &diagnostic);
            assert_eq!(error.code(), $code);
            assert_eq!(
                error.to_string(),
                "failure: bounded cause; caused by: sentinel source"
            );
            let original_source = error.source().unwrap().downcast_ref::<Sentinel>().unwrap();
            assert_eq!(original_source.0, 42);
            let saved = serde_json::to_value(&error).unwrap();
            assert!(saved.get("kind").is_some());
            assert_eq!(
                saved["context"]["diagnostic"]["remediation"]["kind"],
                "recoverable"
            );
            assert_eq!(std::ptr::from_ref(&*error.into_context()), pointer);
        }};
    }
    check!(
        IdentityError::Process,
        error_codes::IDENTITY_RESOLUTION_FAILED
    );
    check!(InitError::Configuration, error_codes::DIAGNOSTIC_INVALID);
    check!(InitError::Runtime, error_codes::DIAGNOSTIC_INVALID);
    check!(EventError::Validation, error_codes::DIAGNOSTIC_INVALID);
    check!(EventError::Routing, error_codes::DIAGNOSTIC_INVALID);
    check!(FlushError::Drain, error_codes::DIAGNOSTIC_INVALID);
    check!(ShutdownError::Timeout, error_codes::DIAGNOSTIC_INVALID);
    check!(ShutdownError::Drain, error_codes::DIAGNOSTIC_INVALID);
    check!(ProjectionError::Projection, error_codes::DIAGNOSTIC_INVALID);
    check!(SubscriberError::Subscriber, error_codes::DIAGNOSTIC_INVALID);
    check!(LogSinkError::Write, error_codes::DIAGNOSTIC_INVALID);
    check!(LogSinkError::Flush, error_codes::DIAGNOSTIC_INVALID);
    check!(
        ExportError::Transport,
        error_codes::IDENTITY_RESOLUTION_FAILED,
        error_codes::IDENTITY_RESOLUTION_FAILED
    );
    check!(
        ExportError::BlockingBackendInAsyncContext,
        error_codes::otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT
    );
    check!(
        ExportError::AsyncLifecycleRequired,
        error_codes::otlp::OTLP_ASYNC_LIFECYCLE_REQUIRED
    );
    check!(
        ExportError::RuntimeTerminated,
        error_codes::otlp::OTLP_RUNTIME_TERMINATED
    );
    check!(
        ExportError::LifecycleTimeout,
        error_codes::otlp::OTLP_LIFECYCLE_TIMEOUT
    );
    check!(ExportError::QueueFull, error_codes::otlp::OTLP_QUEUE_FULL);
    check!(
        ExportError::WorkerTerminated,
        error_codes::otlp::OTLP_WORKER_TERMINATED
    );
    check!(
        ExportError::ShutdownCancelledRetry,
        error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY
    );
    check!(
        ExportError::RetryDeadlineExhausted,
        error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED
    );
    check!(
        ExportError::NonRetryableHttpStatus,
        error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL
    );
    check!(
        ExportError::RetryAttemptsExhausted,
        error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED
    );
    check!(
        ExportError::TerminalExportFailure,
        error_codes::otlp::OTLP_EXPORT_TERMINAL
    );
    check!(
        ConfigFailure::ZeroDuration,
        error_codes::otlp::OTLP_CONFIG_ZERO_DURATION
    );
    check!(
        ConfigFailure::DurationOverflow,
        error_codes::otlp::OTLP_CONFIG_DURATION_OVERFLOW
    );
    check!(
        ConfigFailure::InvalidBoundOrdering,
        error_codes::otlp::OTLP_CONFIG_BOUND_ORDER
    );
    check!(
        ConfigFailure::InvalidJitterPercent,
        error_codes::otlp::OTLP_CONFIG_JITTER_PERCENT
    );
    check!(
        ConfigFailure::InvalidQueueCapacity,
        error_codes::otlp::OTLP_CONFIG_QUEUE_CAPACITY
    );
    check!(
        ConfigFailure::InvalidQueueByteCapacity,
        error_codes::otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY
    );
    check!(
        ConfigFailure::ConfigFieldNotApplicable,
        error_codes::otlp::OTLP_CONFIG_FIELD_NOT_APPLICABLE
    );
    check!(
        ConfigFailure::InsecureTransportRejected,
        error_codes::otlp::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED
    );
    check!(
        ConfigFailure::InvalidEndpoint,
        error_codes::otlp::OTLP_CONFIG_INVALID_ENDPOINT
    );
    check!(
        ConfigFailure::InvalidHeader,
        error_codes::otlp::OTLP_CONFIG_INVALID_HEADER
    );
    check!(
        ConfigFailure::TransportConstructionFailed,
        error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED
    );
    check!(
        ConfigFailure::UnsupportedBackend,
        error_codes::otlp::OTLP_UNSUPPORTED_BACKEND
    );
    check!(
        ConfigFailure::UnsupportedProtocol,
        error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL
    );
    check!(
        ConfigFailure::TokioRuntimeRequired,
        error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED
    );
    check!(
        MetricModelError::InvalidHistogram,
        error_codes::SC_METRIC_INVALID_HISTOGRAM
    );
    check!(
        MetricModelError::InvalidTemporality,
        error_codes::SC_METRIC_INVALID_TEMPORALITY
    );
    check!(
        MetricModelError::InvalidInterval,
        error_codes::SC_METRIC_INVALID_INTERVAL
    );
}

#[test]
fn telemetry_shutdown_preserves_context_and_diagnostic() {
    let original = context(error_codes::otlp::OTLP_TELEMETRY_SHUTDOWN);
    let diagnostic = original.diagnostic().clone();
    let pointer = std::ptr::from_ref(&*original);
    let shutdown = TelemetryError::Shutdown { context: original };

    assert_eq!(std::ptr::from_ref(shutdown.context()), pointer);
    assert_eq!(DiagnosticInfo::diagnostic(&shutdown), &diagnostic);
    assert!(
        shutdown
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<Sentinel>()
    );
    let saved = serde_json::to_value(&shutdown).unwrap();
    assert_eq!(
        saved["Shutdown"]["context"]["diagnostic"]["remediation"]["kind"],
        "recoverable"
    );
    assert_eq!(std::ptr::from_ref(&*shutdown.into_context()), pointer);
}

#[test]
fn cloned_canonical_error_preserves_source_identity() {
    let original = FlushError::Drain {
        context: context(error_codes::VALUE_VALIDATION_FAILED),
    };
    let cloned = original.clone();
    let original_source = std::error::Error::source(original.context())
        .expect("original context must retain its source");
    let cloned_source =
        std::error::Error::source(cloned.context()).expect("cloned context must retain its source");

    assert!(std::ptr::eq(original_source, cloned_source));
    assert_eq!(original_source.to_string(), "sentinel source");
    assert_eq!(cloned_source.to_string(), "sentinel source");
}

#[test]
fn stable_failure_codes() {
    for (capacity, byte_capacity) in [(0u64, 0u64), (65_537, 67_108_865), (u64::MAX, u64::MAX)] {
        let record = ConfigFailure::InvalidQueueCapacity {
            context: Box::new(
                ErrorContext::new(
                    error_codes::otlp::OTLP_CONFIG_QUEUE_CAPACITY,
                    "invalid capacity",
                    Remediation::recoverable("choose 1..=65536 records", [] as [&str; 0]),
                )
                .detail("value", json!(capacity)),
            ),
        };
        let bytes = ConfigFailure::InvalidQueueByteCapacity {
            context: Box::new(
                ErrorContext::new(
                    error_codes::otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
                    "invalid bytes",
                    Remediation::recoverable("choose 1..=64 MiB", [] as [&str; 0]),
                )
                .detail("value", json!(byte_capacity)),
            ),
        };
        assert_ne!(record.diagnostic().code, bytes.diagnostic().code);
    }
    let telemetry: TelemetryError = ExportError::QueueFull {
        context: context(error_codes::VALUE_VALIDATION_FAILED),
    }
    .into();
    assert_eq!(telemetry.code().as_str(), "OTLP_QUEUE_FULL");
    assert_eq!(
        DiagnosticInfo::diagnostic(&telemetry).code,
        error_codes::VALUE_VALIDATION_FAILED
    );
    assert!(matches!(
        &telemetry,
        TelemetryError::ExportFailure(ExportError::QueueFull { .. })
    ));
    let TelemetryError::ExportFailure(error) = &telemetry else {
        unreachable!("queue-full export failure must remain preserved")
    };
    assert_eq!(
        error.diagnostic().code,
        error_codes::VALUE_VALIDATION_FAILED,
        "the preserved diagnostic may describe the underlying source, while the error variant fixes the public stable code"
    );
    assert!(
        telemetry
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<Sentinel>()
    );
    let shutdown = TelemetryError::Shutdown {
        context: Box::new(ErrorContext::new(
            error_codes::otlp::OTLP_TELEMETRY_SHUTDOWN,
            "telemetry runtime is shut down",
            Remediation::recoverable("construct a new telemetry instance", [] as [&str; 0]),
        )),
    };
    assert_eq!(shutdown.code().as_str(), "OTLP_TELEMETRY_SHUTDOWN");
    assert_eq!(
        shutdown.diagnostic().remediation,
        Remediation::recoverable("construct a new telemetry instance", [] as [&str; 0])
    );
}
fn finite(n: f64) -> FiniteF64 {
    FiniteF64::new(n).unwrap()
}
fn histogram() -> HistogramPoint {
    HistogramPoint::try_new(
        vec![finite(1.0), finite(2.0)],
        vec![1, 2, 3],
        6,
        finite(12.0),
    )
    .unwrap()
}
#[test]
fn histogram_point_serde_rejects_invalid() {
    let good = serde_json::to_value(histogram()).unwrap();
    assert_eq!(
        serde_json::from_value::<HistogramPoint>(good.clone()).unwrap(),
        histogram()
    );
    for (field, value) in [
        ("explicit_bounds", json!([2.0, 1.0])),
        ("explicit_bounds", json!([1.0, 1.0])),
        ("bucket_counts", json!([1, 5])),
        ("bucket_counts", json!([u64::MAX, 1, 0])),
        ("count", json!(7)),
        ("sum", json!(null)),
    ] {
        let mut bad = good.clone();
        bad[field] = value;
        assert!(
            serde_json::from_value::<HistogramPoint>(bad).is_err(),
            "accepted {field}"
        );
    }
    assert!(
        serde_json::from_value::<HistogramPoint>(json!({
            "explicit_bounds": [f64::INFINITY],
            "bucket_counts": [0, 0],
            "count": 0,
            "sum": 0.0
        }))
        .is_err()
    );
    let error = HistogramPoint::try_new(vec![], vec![0], 0, finite(1.0))
        .expect_err("a nonzero sum cannot have zero samples");
    assert_eq!(
        error.diagnostic().code,
        error_codes::SC_METRIC_INVALID_HISTOGRAM
    );
    assert!(HistogramPoint::try_new(vec![], vec![0], 0, finite(0.0)).is_ok());
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = FiniteF64::new(value).expect_err("non-finite values must be rejected");
        assert_eq!(error.code(), &error_codes::SC_METRIC_NON_FINITE);
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive test keeps metric validation and every frozen wire envelope together"
)]
fn metric_model_failures() {
    let start = Timestamp::UNIX_EPOCH;
    let end = start + time::Duration::seconds(1);
    let sum = |temporality, start_time| MetricValue::Sum {
        value: finite(1.0),
        monotonic: true,
        temporality,
        start_time,
    };
    let error = sum(AggregationTemporality::Delta, start)
        .validate_at(start)
        .unwrap_err();
    assert!(matches!(error, MetricModelError::InvalidTemporality { .. }));
    assert_eq!(
        error.diagnostic().code.as_str(),
        "SC_METRIC_INVALID_TEMPORALITY"
    );
    let error = sum(AggregationTemporality::Cumulative, end)
        .validate_at(start)
        .unwrap_err();
    assert!(matches!(error, MetricModelError::InvalidInterval { .. }));
    assert_eq!(
        error.diagnostic().code.as_str(),
        "SC_METRIC_INVALID_INTERVAL"
    );
    assert!(
        sum(AggregationTemporality::Delta, start)
            .validate_at(end)
            .is_ok()
    );
    assert!(
        sum(AggregationTemporality::Cumulative, start)
            .validate_at(start)
            .is_ok()
    );
    assert!(MetricValue::Gauge(finite(-1.0)).validate_at(start).is_ok());
    assert!(
        MetricValue::Sum {
            value: finite(-1.0),
            monotonic: true,
            temporality: AggregationTemporality::Cumulative,
            start_time: start
        }
        .validate_at(end)
        .is_err()
    );
    let record = MetricRecord::try_new(
        end,
        sc_observability_types::ServiceName::new("demo").unwrap(),
        sc_observability_types::MetricName::new("latency").unwrap(),
        MetricValue::Histogram {
            point: histogram(),
            temporality: AggregationTemporality::Delta,
            start_time: start,
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(MetricValue::Gauge(finite(1.5))).unwrap(),
        json!({"kind":"gauge","data":1.5})
    );
    assert_eq!(
        serde_json::to_value(MetricValue::Sum {
            value: finite(1.5),
            monotonic: true,
            temporality: AggregationTemporality::Delta,
            start_time: start,
        })
        .unwrap(),
        json!({
            "kind": "sum",
            "data": {
                "value": 1.5,
                "monotonic": true,
                "temporality": "delta",
                "start_time": "1970-01-01T00:00:00Z",
            }
        })
    );
    assert_eq!(
        serde_json::to_value(MetricValue::Histogram {
            point: histogram(),
            temporality: AggregationTemporality::Delta,
            start_time: start,
        })
        .unwrap(),
        json!({
            "kind": "histogram",
            "data": {
                "point": {
                    "explicit_bounds": [1.0, 2.0],
                    "bucket_counts": [1, 2, 3],
                    "count": 6,
                    "sum": 12.0,
                },
                "temporality": "delta",
                "start_time": "1970-01-01T00:00:00Z",
            }
        })
    );
    assert_eq!(
        serde_json::to_value(SpanKind::Client).unwrap(),
        json!("client")
    );
    assert_eq!(
        serde_json::to_value(SpanKind::Consumer).unwrap(),
        json!("consumer")
    );
    assert_eq!(
        serde_json::to_value(SpanKind::Internal).unwrap(),
        json!("internal")
    );
    assert_eq!(
        serde_json::to_value(SpanKind::Producer).unwrap(),
        json!("producer")
    );
    assert_eq!(
        serde_json::to_value(SpanKind::Server).unwrap(),
        json!("server")
    );
    let mut value = serde_json::to_value(&record).unwrap();
    assert_eq!(
        value,
        json!({
            "timestamp": "1970-01-01T00:00:01Z",
            "service": "demo",
            "name": "latency",
            "value": {
                "kind": "histogram",
                "data": {
                    "point": {
                        "explicit_bounds": [1.0, 2.0],
                        "bucket_counts": [1, 2, 3],
                        "count": 6,
                        "sum": 12.0,
                    },
                    "temporality": "delta",
                    "start_time": "1970-01-01T00:00:00Z",
                }
            },
            "unit": null,
            "attributes": {},
        })
    );
    let decorated_record = MetricRecord::try_new(
        end,
        sc_observability_types::ServiceName::new("demo").unwrap(),
        sc_observability_types::MetricName::new("latency").unwrap(),
        MetricValue::Gauge(finite(1.5)),
    )
    .unwrap()
    .with_unit(Some(MetricUnit::new("ms").unwrap()))
    .with_attributes(Attributes::from([(
        "region".to_string(),
        AttributeValue::String("us-west".to_string()),
    )]));
    let decorated_value = serde_json::to_value(decorated_record).unwrap();
    assert_eq!(decorated_value["unit"], json!("ms"));
    assert_eq!(
        decorated_value["attributes"],
        json!({"region": {"kind": "string", "data": "us-west"}})
    );
    assert_eq!(
        serde_json::from_value::<MetricRecord>(value.clone()).unwrap(),
        record
    );
    value["timestamp"] = json!(start);
    assert!(serde_json::from_value::<MetricRecord>(value).is_err());
}
#[test]
fn span_kind_links_flags_and_typestate_survive_export() {
    use sc_observability_types::{ActionName, ServiceName, SpanId, TraceId};
    let trace = TraceId::new("0123456789abcdef0123456789abcdef").unwrap();
    let span = SpanId::new("0123456789abcdef").unwrap();
    let flags = TraceFlags::new(0x83);
    assert!(flags.sampled());
    assert_eq!(
        serde_json::from_str::<TraceFlags>("131").unwrap().bits(),
        0x83
    );
    let link = SpanLink::new(trace.clone(), span.clone(), flags, Attributes::new());
    let record = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        ServiceName::new("demo").unwrap(),
        ActionName::new("run").unwrap(),
        TraceContext::new(trace, span, flags),
        Attributes::new(),
    )
    .with_kind(SpanKind::Server)
    .with_links(vec![link.clone()])
    .end(SpanStatus::Ok, 42u64.into());
    assert_eq!(record.kind(), SpanKind::Server);
    assert_eq!(record.links(), &[link]);
    assert_eq!(record.duration_ms().as_u64(), 42);
    let wire = serde_json::to_value(SpanSignal::Ended(record)).unwrap();
    assert_eq!(wire["Ended"]["trace"]["flags"], 131);
    assert_eq!(wire["Ended"]["kind"], "server");
    assert!(wire["Ended"]["links"][0].get("trace").is_none());
    assert!(
        serde_json::from_value::<TraceContext>(
            json!({"trace_id":"bad","span_id":"bad","flags":1,"parent_span_id":null})
        )
        .is_err()
    );
}
