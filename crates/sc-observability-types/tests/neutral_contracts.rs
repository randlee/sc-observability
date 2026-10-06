#![cfg(any())]

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
    assert_eq!(error_codes::otlp::ALL.len(), 30);
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

#[cfg(feature = "v1")]
mod legacy_compatibility {
    //! External-consumer fixtures for the neutral typed failure contract.

    #![allow(
        deprecated,
        reason = "neutral compatibility tests exercise both legacy and typed adapter contracts"
    )]

    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use sc_observability_types::typed::{
        ClassifiedError, IdentityFailure, IdentityFailureKind, ProjectionFailure,
        ProjectionFailureKind, SubscriberFailure, SubscriberFailureKind, TypedLogProjector,
        TypedMetricProjector, TypedObservationSubscriber, TypedProcessIdentityResolver,
        TypedSpanProjector, legacy_identity, legacy_log_projector, legacy_metric_projector,
        legacy_span_projector, legacy_subscriber, typed_identity, typed_log_projector,
        typed_metric_projector, typed_span_projector, typed_subscriber,
    };
    use sc_observability_types::*;
    use serde_json::Map;

    fn remediation() -> Remediation {
        Remediation::not_recoverable("adapter test remediation")
    }

    fn context(code: &'static str) -> Box<ErrorContext> {
        Box::new(
            ErrorContext::new(
                ErrorCode::new_static(code),
                "adapter failure",
                remediation(),
            )
            .source(Box::new(std::io::Error::other("adapter source"))),
        )
    }

    fn observation() -> Observation<String> {
        Observation::new(
            ServiceName::new("adapter-test").expect("valid service"),
            "payload".to_string(),
        )
    }

    fn trace() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span"),
            parent_span_id: None,
        }
    }

    fn log_event() -> LogEvent {
        LogEvent {
            version: SchemaVersion::new("v1").expect("valid schema"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: ServiceName::new("adapter-test").expect("valid service"),
            target: TargetCategory::new("adapter").expect("valid target"),
            action: ActionName::new("adapter.success").expect("valid action"),
            message: Some("success".to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace()),
            request_id: None,
            correlation_id: None,
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields: Map::new(),
        }
    }

    fn span_signal() -> SpanSignal {
        SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: trace(),
            name: ActionName::new("adapter.success").expect("valid action"),
            attributes: Map::new(),
            diagnostic: None,
        })
    }

    fn metric_record() -> MetricRecord {
        MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: ServiceName::new("adapter-test").expect("valid service"),
            name: MetricName::new("adapter.success").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: None,
            attributes: Map::new(),
        }
    }

    fn assert_context(context: &ErrorContext, expected_pointer: usize) {
        assert_eq!(std::ptr::from_ref(context) as usize, expected_pointer);
        assert_eq!(
            context.to_string(),
            "adapter failure; caused by: adapter source"
        );
        let source = std::error::Error::source(context).expect("source error");
        assert_eq!(source.to_string(), "adapter source");
    }

    fn typed_projection_failure(code: &'static str) -> (ProjectionFailure, usize) {
        let context = context(code);
        let expected_pointer = std::ptr::from_ref(context.as_ref()) as usize;
        (ProjectionFailure::from_context(context), expected_pointer)
    }

    struct TypedIdentityError {
        calls: Arc<AtomicUsize>,
        // The mutex lets this `&self` fixture transfer its owned failure exactly once.
        failure: Mutex<Option<IdentityFailure>>,
        expected_pointer: usize,
    }

    impl TypedProcessIdentityResolver for TypedIdentityError {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .failure
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct LegacyIdentityError {
        calls: Arc<AtomicUsize>,
        // The mutex lets this `&self` fixture transfer its owned failure exactly once.
        failure: Mutex<Option<IdentityError>>,
        expected_pointer: usize,
    }

    impl ProcessIdentityResolver for LegacyIdentityError {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .failure
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct TypedSubscriberError {
        calls: Arc<AtomicUsize>,
        // The mutex lets this `&self` fixture transfer its owned failure exactly once.
        failure: Mutex<Option<SubscriberFailure>>,
        expected_pointer: usize,
    }

    impl TypedObservationSubscriber<String> for TypedSubscriberError {
        fn observe(&self, _: &Observation<String>) -> Result<(), SubscriberFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .failure
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct LegacySubscriberError {
        calls: Arc<AtomicUsize>,
        // The mutex lets this `&self` fixture transfer its owned failure exactly once.
        failure: Mutex<Option<SubscriberError>>,
        expected_pointer: usize,
    }

    impl ObservationSubscriber<String> for LegacySubscriberError {
        fn observe(&self, _: &Observation<String>) -> Result<(), SubscriberError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .failure
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct TypedProjectorErrors {
        log_calls: Arc<AtomicUsize>,
        span_calls: Arc<AtomicUsize>,
        metric_calls: Arc<AtomicUsize>,
        // These mutexes let `&self` fixtures transfer each owned failure exactly once.
        log: Mutex<Option<ProjectionFailure>>,
        span: Mutex<Option<ProjectionFailure>>,
        metric: Mutex<Option<ProjectionFailure>>,
        log_pointer: usize,
        span_pointer: usize,
        metric_pointer: usize,
    }

    impl TypedLogProjector<String> for TypedProjectorErrors {
        fn project_logs(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<LogEvent>, ProjectionFailure> {
            self.log_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .log
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    impl TypedSpanProjector<String> for TypedProjectorErrors {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionFailure> {
            self.span_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .span
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    impl TypedMetricProjector<String> for TypedProjectorErrors {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
            self.metric_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .metric
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct LegacyProjectorErrors {
        log_calls: Arc<AtomicUsize>,
        span_calls: Arc<AtomicUsize>,
        metric_calls: Arc<AtomicUsize>,
        // These mutexes let `&self` fixtures transfer each owned failure exactly once.
        log: Mutex<Option<ProjectionError>>,
        span: Mutex<Option<ProjectionError>>,
        metric: Mutex<Option<ProjectionError>>,
        log_pointer: usize,
        span_pointer: usize,
        metric_pointer: usize,
    }

    impl LogProjector<String> for LegacyProjectorErrors {
        fn project_logs(&self, _: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionError> {
            self.log_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .log
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    impl SpanProjector<String> for LegacyProjectorErrors {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionError> {
            self.span_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .span
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    impl MetricProjector<String> for LegacyProjectorErrors {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            self.metric_calls.fetch_add(1, Ordering::SeqCst);
            Err(self
                .metric
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("one call"))
        }
    }

    struct TypedSuccess;

    impl TypedProcessIdentityResolver for TypedSuccess {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure> {
            Ok(ProcessIdentity {
                hostname: Some("typed".to_string()),
                pid: Some(7),
            })
        }
    }

    impl TypedObservationSubscriber<String> for TypedSuccess {
        fn observe(&self, _: &Observation<String>) -> Result<(), SubscriberFailure> {
            Ok(())
        }
    }

    impl TypedLogProjector<String> for TypedSuccess {
        fn project_logs(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<LogEvent>, ProjectionFailure> {
            Ok(vec![log_event()])
        }
    }

    impl TypedSpanProjector<String> for TypedSuccess {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionFailure> {
            Ok(vec![span_signal()])
        }
    }

    impl TypedMetricProjector<String> for TypedSuccess {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
            Ok(vec![metric_record()])
        }
    }

    struct LegacySuccess;

    impl ProcessIdentityResolver for LegacySuccess {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            Ok(ProcessIdentity {
                hostname: Some("legacy".to_string()),
                pid: Some(8),
            })
        }
    }

    impl ObservationSubscriber<String> for LegacySuccess {
        fn observe(&self, _: &Observation<String>) -> Result<(), SubscriberError> {
            Ok(())
        }
    }

    impl LogProjector<String> for LegacySuccess {
        fn project_logs(&self, _: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionError> {
            Ok(vec![log_event()])
        }
    }

    impl SpanProjector<String> for LegacySuccess {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionError> {
            Ok(vec![span_signal()])
        }
    }

    impl MetricProjector<String> for LegacySuccess {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            Ok(vec![metric_record()])
        }
    }

    #[test]
    fn typed_to_legacy_adapters_preserve_errors_and_invoke_once() {
        let identity_context = context("SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED");
        let identity_pointer = std::ptr::from_ref(identity_context.as_ref()) as usize;
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let identity = Arc::new(TypedIdentityError {
            calls: Arc::clone(&identity_calls),
            failure: Mutex::new(Some(IdentityFailure::from_context(identity_context))),
            expected_pointer: identity_pointer,
        });
        let error = legacy_identity(identity.clone())
            .resolve()
            .expect_err("failure expected");
        assert_context(&error.0, identity.expected_pointer);
        assert_eq!(identity_calls.load(Ordering::SeqCst), 1);

        let subscriber_context = context("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
        let subscriber_pointer = std::ptr::from_ref(subscriber_context.as_ref()) as usize;
        let subscriber_calls = Arc::new(AtomicUsize::new(0));
        let subscriber = Arc::new(TypedSubscriberError {
            calls: Arc::clone(&subscriber_calls),
            failure: Mutex::new(Some(SubscriberFailure::from_context(subscriber_context))),
            expected_pointer: subscriber_pointer,
        });
        let error = legacy_subscriber(subscriber.clone())
            .observe(&observation())
            .expect_err("failure expected");
        assert_context(&error.0, subscriber.expected_pointer);
        assert_eq!(subscriber_calls.load(Ordering::SeqCst), 1);

        let (log_failure, log_pointer) =
            typed_projection_failure("SC_OBSERVABILITY_OTLP_EXPORT_FAILED");
        let (span_failure, span_pointer) =
            typed_projection_failure("SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED");
        let (metric_failure, metric_pointer) =
            typed_projection_failure("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
        let projectors = Arc::new(TypedProjectorErrors {
            log_calls: Arc::new(AtomicUsize::new(0)),
            span_calls: Arc::new(AtomicUsize::new(0)),
            metric_calls: Arc::new(AtomicUsize::new(0)),
            log: Mutex::new(Some(log_failure)),
            span: Mutex::new(Some(span_failure)),
            metric: Mutex::new(Some(metric_failure)),
            log_pointer,
            span_pointer,
            metric_pointer,
        });
        let error = legacy_log_projector(projectors.clone())
            .project_logs(&observation())
            .expect_err("failure expected");
        assert_context(&error.0, projectors.log_pointer);
        let error = legacy_span_projector(projectors.clone())
            .project_spans(&observation())
            .expect_err("failure expected");
        assert_context(&error.0, projectors.span_pointer);
        let error = legacy_metric_projector(projectors.clone())
            .project_metrics(&observation())
            .expect_err("failure expected");
        assert_context(&error.0, projectors.metric_pointer);
        assert_eq!(projectors.log_calls.load(Ordering::SeqCst), 1);
        assert_eq!(projectors.span_calls.load(Ordering::SeqCst), 1);
        assert_eq!(projectors.metric_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn legacy_to_typed_adapters_preserve_errors_and_invoke_once() {
        let identity_context = context("SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED");
        let identity_pointer = std::ptr::from_ref(identity_context.as_ref()) as usize;
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let identity = Arc::new(LegacyIdentityError {
            calls: Arc::clone(&identity_calls),
            failure: Mutex::new(Some(IdentityError(identity_context))),
            expected_pointer: identity_pointer,
        });
        let error = typed_identity(identity.clone())
            .resolve()
            .expect_err("failure expected");
        assert_context(error.context(), identity.expected_pointer);
        assert_eq!(error.kind(), IdentityFailureKind::ResolutionFailed);
        assert_eq!(identity_calls.load(Ordering::SeqCst), 1);

        let subscriber_context = context("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
        let subscriber_pointer = std::ptr::from_ref(subscriber_context.as_ref()) as usize;
        let subscriber_calls = Arc::new(AtomicUsize::new(0));
        let subscriber = Arc::new(LegacySubscriberError {
            calls: Arc::clone(&subscriber_calls),
            failure: Mutex::new(Some(SubscriberError(subscriber_context))),
            expected_pointer: subscriber_pointer,
        });
        let error = typed_subscriber(subscriber.clone())
            .observe(&observation())
            .expect_err("failure expected");
        assert_context(error.context(), subscriber.expected_pointer);
        assert_eq!(error.kind(), SubscriberFailureKind::Routing);
        assert_eq!(subscriber_calls.load(Ordering::SeqCst), 1);

        let log_context = context("SC_OBSERVABILITY_OTLP_EXPORT_FAILED");
        let log_pointer = std::ptr::from_ref(log_context.as_ref()) as usize;
        let span_context = context("SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED");
        let span_pointer = std::ptr::from_ref(span_context.as_ref()) as usize;
        let metric_context = context("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
        let metric_pointer = std::ptr::from_ref(metric_context.as_ref()) as usize;
        let projectors = Arc::new(LegacyProjectorErrors {
            log_calls: Arc::new(AtomicUsize::new(0)),
            span_calls: Arc::new(AtomicUsize::new(0)),
            metric_calls: Arc::new(AtomicUsize::new(0)),
            log: Mutex::new(Some(ProjectionError(log_context))),
            span: Mutex::new(Some(ProjectionError(span_context))),
            metric: Mutex::new(Some(ProjectionError(metric_context))),
            log_pointer,
            span_pointer,
            metric_pointer,
        });
        let error = typed_log_projector(projectors.clone())
            .project_logs(&observation())
            .expect_err("failure expected");
        assert_context(error.context(), projectors.log_pointer);
        assert_eq!(error.kind(), ProjectionFailureKind::TelemetryExport);
        let error = typed_span_projector(projectors.clone())
            .project_spans(&observation())
            .expect_err("failure expected");
        assert_context(error.context(), projectors.span_pointer);
        assert_eq!(error.kind(), ProjectionFailureKind::SpanAssembly);
        let error = typed_metric_projector(projectors.clone())
            .project_metrics(&observation())
            .expect_err("failure expected");
        assert_context(error.context(), projectors.metric_pointer);
        assert_eq!(error.kind(), ProjectionFailureKind::Routing);
        assert_eq!(projectors.log_calls.load(Ordering::SeqCst), 1);
        assert_eq!(projectors.span_calls.load(Ordering::SeqCst), 1);
        assert_eq!(projectors.metric_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn all_ten_adapters_preserve_success_values() {
        let identity = ProcessIdentity {
            hostname: Some("typed".to_string()),
            pid: Some(7),
        };
        assert_eq!(
            legacy_identity(Arc::new(TypedSuccess))
                .resolve()
                .expect("success"),
            identity
        );
        let identity = ProcessIdentity {
            hostname: Some("legacy".to_string()),
            pid: Some(8),
        };
        assert_eq!(
            typed_identity(Arc::new(LegacySuccess))
                .resolve()
                .expect("success"),
            identity
        );

        let expected_observation = observation();
        assert_eq!(
            legacy_subscriber::<String>(Arc::new(TypedSuccess))
                .observe(&expected_observation)
                .expect("success"),
            ()
        );
        assert_eq!(
            typed_subscriber::<String>(Arc::new(LegacySuccess))
                .observe(&expected_observation)
                .expect("success"),
            ()
        );
        assert_eq!(
            legacy_log_projector::<String>(Arc::new(TypedSuccess))
                .project_logs(&expected_observation)
                .expect("success"),
            vec![log_event()]
        );
        assert_eq!(
            typed_log_projector::<String>(Arc::new(LegacySuccess))
                .project_logs(&expected_observation)
                .expect("success"),
            vec![log_event()]
        );
        assert_eq!(
            legacy_span_projector::<String>(Arc::new(TypedSuccess))
                .project_spans(&expected_observation)
                .expect("success"),
            vec![span_signal()]
        );
        assert_eq!(
            typed_span_projector::<String>(Arc::new(LegacySuccess))
                .project_spans(&expected_observation)
                .expect("success"),
            vec![span_signal()]
        );
        assert_eq!(
            legacy_metric_projector::<String>(Arc::new(TypedSuccess))
                .project_metrics(&expected_observation)
                .expect("success"),
            vec![metric_record()]
        );
        assert_eq!(
            typed_metric_projector::<String>(Arc::new(LegacySuccess))
                .project_metrics(&expected_observation)
                .expect("success"),
            vec![metric_record()]
        );
    }

    #[test]
    fn unchanged_root_glob_consumer_compiles_alongside_explicit_typed_imports() {
        let _: Arc<dyn ProcessIdentityResolver> = Arc::new(LegacySuccess);
        let _: Arc<dyn ObservationSubscriber<String>> = Arc::new(LegacySuccess);
        let _: Arc<dyn LogProjector<String>> = Arc::new(LegacySuccess);
        let _: Arc<dyn SpanProjector<String>> = Arc::new(LegacySuccess);
        let _: Arc<dyn MetricProjector<String>> = Arc::new(LegacySuccess);
    }
}

#[cfg(feature = "v1")]
mod released_canonical_conversion {
    //! Released root registrations keep the caller's objects; conversions to and
    //! from the canonical `v2` family move the original error context.

    #![allow(
        deprecated,
        reason = "the released root traits report the retained root errors"
    )]

    use std::sync::{Arc, Mutex};

    use sc_observability_types::v2;
    use sc_observability_types::*;

    fn context() -> Box<ErrorContext> {
        Box::new(
            ErrorContext::new(
                ErrorCode::new_static("CONVERSION_TEST_FAILED"),
                "conversion failure",
                Remediation::not_recoverable("conversion test remediation"),
            )
            .source(Box::new(std::io::Error::other("conversion source"))),
        )
    }

    fn observation() -> Observation<String> {
        Observation::new(
            ServiceName::new("conversion-test").expect("valid service"),
            "payload".to_string(),
        )
    }

    /// The addresses a moved context must keep: the context and its source.
    fn identity(context: &ErrorContext) -> (usize, usize) {
        let source = std::error::Error::source(context).expect("context source");
        (
            std::ptr::from_ref(context) as usize,
            std::ptr::from_ref(source).cast::<()>() as usize,
        )
    }

    /// Returns its one boxed context exactly once, from either family.
    struct Once(Mutex<Option<Box<ErrorContext>>>);

    impl Once {
        fn new() -> (Arc<Self>, (usize, usize)) {
            let context = context();
            let expected = identity(&context);
            (Arc::new(Self(Mutex::new(Some(context)))), expected)
        }

        fn take(&self) -> Box<ErrorContext> {
            self.0
                .lock()
                .expect("unpoisoned")
                .take()
                .expect("fixture invoked once")
        }
    }

    impl ProcessIdentityResolver for Once {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            Err(IdentityError(self.take()))
        }
    }

    impl ObservationSubscriber<String> for Once {
        fn observe(&self, _: &Observation<String>) -> Result<(), SubscriberError> {
            Err(SubscriberError(self.take()))
        }
    }

    impl LogProjector<String> for Once {
        fn project_logs(&self, _: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionError> {
            Err(ProjectionError(self.take()))
        }
    }

    impl SpanProjector<String> for Once {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionError> {
            Err(ProjectionError(self.take()))
        }
    }

    impl MetricProjector<String> for Once {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            Err(ProjectionError(self.take()))
        }
    }

    /// Returns its one boxed context exactly once through the `v2` traits.
    struct CanonicalOnce(Once);

    impl CanonicalOnce {
        fn new() -> (Arc<Self>, (usize, usize)) {
            let context = context();
            let expected = identity(&context);
            (Arc::new(Self(Once(Mutex::new(Some(context))))), expected)
        }
    }

    impl v2::ProcessIdentityResolver for CanonicalOnce {
        fn resolve(&self) -> Result<ProcessIdentity, v2::IdentityError> {
            Err(v2::IdentityError::Process {
                context: self.0.take(),
            })
        }
    }

    impl v2::ObservationSubscriber<String> for CanonicalOnce {
        fn observe(&self, _: &Observation<String>) -> Result<(), v2::SubscriberError> {
            Err(v2::SubscriberError::Subscriber {
                context: self.0.take(),
            })
        }
    }

    impl v2::LogProjector<String> for CanonicalOnce {
        fn project_logs(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<LogEvent>, v2::ProjectionError> {
            Err(v2::ProjectionError::Projection {
                context: self.0.take(),
            })
        }
    }

    struct AcceptAll;

    impl ObservationFilter<String> for AcceptAll {
        fn accepts(&self, _: &Observation<String>) -> bool {
            true
        }
    }

    #[test]
    fn released_registrations_return_the_supplied_objects() {
        let (subscriber, _) = Once::new();
        let subscriber: Arc<dyn ObservationSubscriber<String>> = subscriber;
        let filter: Arc<dyn ObservationFilter<String>> = Arc::new(AcceptAll);
        let (returned, returned_filter) = SubscriberRegistration::new(Arc::clone(&subscriber))
            .with_filter(Arc::clone(&filter))
            .into_parts();
        assert!(Arc::ptr_eq(&returned, &subscriber));
        assert!(Arc::ptr_eq(&returned_filter.expect("filter"), &filter));

        let (projector, _) = Once::new();
        let log: Arc<dyn LogProjector<String>> = projector.clone();
        let span: Arc<dyn SpanProjector<String>> = projector.clone();
        let metric: Arc<dyn MetricProjector<String>> = projector;
        let (returned_log, returned_span, returned_metric, returned_filter) =
            ProjectionRegistration::new()
                .with_log_projector(Arc::clone(&log))
                .with_span_projector(Arc::clone(&span))
                .with_metric_projector(Arc::clone(&metric))
                .with_filter(Arc::clone(&filter))
                .into_parts();
        assert!(Arc::ptr_eq(&returned_log.expect("log"), &log));
        assert!(Arc::ptr_eq(&returned_span.expect("span"), &span));
        assert!(Arc::ptr_eq(&returned_metric.expect("metric"), &metric));
        assert!(Arc::ptr_eq(&returned_filter.expect("filter"), &filter));
    }

    #[test]
    fn released_to_canonical_conversion_moves_every_context() {
        let (subscriber, expected) = Once::new();
        let filter: Arc<dyn ObservationFilter<String>> = Arc::new(AcceptAll);
        let converted: v2::SubscriberRegistration<String> = SubscriberRegistration::new(subscriber)
            .with_filter(Arc::clone(&filter))
            .into();
        let (converted, converted_filter) = converted.into_parts();
        assert!(Arc::ptr_eq(&converted_filter.expect("filter"), &filter));
        let error = converted
            .observe(&observation())
            .expect_err("subscriber failure");
        assert!(matches!(error, v2::SubscriberError::Subscriber { .. }));
        assert_eq!(identity(error.context()), expected);
        assert_eq!(error.diagnostic().code.as_str(), "CONVERSION_TEST_FAILED");

        for signal in ["log", "span", "metric"] {
            let (projector, expected) = Once::new();
            let registration = match signal {
                "log" => ProjectionRegistration::new().with_log_projector(projector),
                "span" => ProjectionRegistration::new().with_span_projector(projector),
                _ => ProjectionRegistration::new().with_metric_projector(projector),
            };
            let (log, span, metric, _) =
                v2::ProjectionRegistration::from(registration).into_parts();
            let error = match signal {
                "log" => log.expect("log").project_logs(&observation()).map(drop),
                "span" => span.expect("span").project_spans(&observation()).map(drop),
                _ => metric
                    .expect("metric")
                    .project_metrics(&observation())
                    .map(drop),
            }
            .expect_err("projection failure");
            assert_eq!(identity(error.context()), expected, "{signal}");
            assert_eq!(error.diagnostic().message, "conversion failure", "{signal}");
        }
    }

    #[test]
    fn canonical_to_released_and_back_moves_the_context() {
        let (subscriber, expected) = CanonicalOnce::new();
        let released: SubscriberRegistration<String> =
            v2::SubscriberRegistration::new(subscriber).into();
        let round_trip = v2::SubscriberRegistration::from(released);
        let error = round_trip
            .into_parts()
            .0
            .observe(&observation())
            .expect_err("subscriber failure");
        assert_eq!(identity(error.context()), expected);

        let (projector, expected) = CanonicalOnce::new();
        let released: ProjectionRegistration<String> = v2::ProjectionRegistration::new()
            .with_log_projector(projector)
            .into();
        let error = released
            .into_parts()
            .0
            .expect("log")
            .project_logs(&observation())
            .expect_err("projection failure");
        assert_eq!(identity(&error.0), expected);
    }
}

#[cfg(feature = "v1")]
mod released_canonical_model_conversion {
    //! Registration conversion maps span and metric models field by field and
    //! returns a projection error for a value the other family cannot hold.

    #![allow(
        deprecated,
        reason = "the released root traits report the retained root errors"
    )]

    use std::sync::Arc;

    use sc_observability_types::v2;
    use sc_observability_types::*;
    use serde_json::{Map, json};

    const TRACE: &str = "0123456789abcdef0123456789abcdef";
    const SPAN: &str = "0123456789abcdef";
    const PARENT: &str = "fedcba9876543210";

    fn service() -> ServiceName {
        ServiceName::new("model-conversion").expect("valid service")
    }

    fn observation() -> Observation<String> {
        Observation::new(service(), "payload".to_string())
    }

    fn later() -> Timestamp {
        serde_json::from_str("\"1970-01-01T00:00:05Z\"").expect("valid timestamp")
    }

    fn released_trace() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new(TRACE).expect("trace"),
            span_id: SpanId::new(SPAN).expect("span"),
            parent_span_id: Some(SpanId::new(PARENT).expect("parent")),
        }
    }

    fn canonical_trace(flags: u8) -> v2::TraceContext {
        v2::TraceContext::new(
            TraceId::new(TRACE).expect("trace"),
            SpanId::new(SPAN).expect("span"),
            v2::TraceFlags::new(flags),
        )
        .with_parent(SpanId::new(PARENT).expect("parent"))
    }

    fn released_metric(kind: MetricKind, value: f64) -> MetricRecord {
        MetricRecord {
            timestamp: later(),
            service: service(),
            name: MetricName::new("conversion.value").expect("metric"),
            kind,
            value,
            unit: Some(MetricUnit::new("ms").expect("unit")),
            attributes: Map::from_iter([("count".to_owned(), json!(3))]),
        }
    }

    struct Released(Vec<SpanSignal>, Vec<MetricRecord>);

    impl SpanProjector<String> for Released {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<SpanSignal>, ProjectionError> {
            Ok(self.0.clone())
        }
    }

    impl MetricProjector<String> for Released {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            Ok(self.1.clone())
        }
    }

    struct Canonical(Vec<v2::SpanSignal>, Vec<v2::MetricRecord>);

    impl v2::SpanProjector<String> for Canonical {
        fn project_spans(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<v2::SpanSignal>, v2::ProjectionError> {
            Ok(self.0.clone())
        }
    }

    impl v2::MetricProjector<String> for Canonical {
        fn project_metrics(
            &self,
            _: &Observation<String>,
        ) -> Result<Vec<v2::MetricRecord>, v2::ProjectionError> {
            Ok(self.1.clone())
        }
    }

    fn to_canonical(
        released: Released,
    ) -> (
        Arc<dyn v2::SpanProjector<String>>,
        Arc<dyn v2::MetricProjector<String>>,
    ) {
        let released = Arc::new(released);
        let (_, span, metric, _) = v2::ProjectionRegistration::from(
            ProjectionRegistration::new()
                .with_span_projector(released.clone())
                .with_metric_projector(released),
        )
        .into_parts();
        (span.expect("span"), metric.expect("metric"))
    }

    fn to_released(
        canonical: Canonical,
    ) -> (
        Arc<dyn SpanProjector<String>>,
        Arc<dyn MetricProjector<String>>,
    ) {
        let canonical = Arc::new(canonical);
        let (_, span, metric, _) = ProjectionRegistration::from(
            v2::ProjectionRegistration::new()
                .with_span_projector(canonical.clone())
                .with_metric_projector(canonical),
        )
        .into_parts();
        (span.expect("span"), metric.expect("metric"))
    }

    fn canonical_started(flags: u8) -> v2::SpanRecord<SpanStarted> {
        v2::SpanRecord::new(
            Timestamp::UNIX_EPOCH,
            service(),
            ActionName::new("conversion.run").expect("action"),
            canonical_trace(flags),
            v2::Attributes::from([("count".to_owned(), v2::AttributeValue::Int(3))]),
        )
    }

    fn canonical_metric(value: v2::MetricValue) -> v2::MetricRecord {
        v2::MetricRecord::try_new(
            later(),
            service(),
            MetricName::new("conversion.value").expect("metric"),
            value,
        )
        .expect("valid canonical metric")
        .with_unit(Some(MetricUnit::new("ms").expect("unit")))
        .with_attributes(v2::Attributes::from([(
            "count".to_owned(),
            v2::AttributeValue::Int(3),
        )]))
    }

    fn finite(value: f64) -> v2::FiniteF64 {
        v2::FiniteF64::new(value).expect("finite")
    }

    #[test]
    fn released_models_convert_to_canonical_with_every_field() {
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service(),
            ActionName::new("conversion.run").expect("action"),
            released_trace(),
            Map::from_iter([("count".to_owned(), json!(3))]),
        );
        let ended = started.clone().end(SpanStatus::Error, DurationMs::from(12));
        let event = SpanEvent {
            timestamp: later(),
            trace: released_trace(),
            name: ActionName::new("conversion.event").expect("event"),
            attributes: Map::from_iter([("flag".to_owned(), json!(true))]),
            diagnostic: None,
        };
        let (span, metric) = to_canonical(Released(
            vec![
                SpanSignal::Started(started),
                SpanSignal::Event(event),
                SpanSignal::Ended(ended),
            ],
            vec![
                released_metric(MetricKind::Counter, 4.0),
                released_metric(MetricKind::Gauge, -2.5),
            ],
        ));

        let expected_started = canonical_started(0);
        let expected_event = v2::SpanEvent {
            timestamp: later(),
            trace: canonical_trace(0),
            name: ActionName::new("conversion.event").expect("event"),
            attributes: v2::Attributes::from([("flag".to_owned(), v2::AttributeValue::Bool(true))]),
            diagnostic: None,
        };
        assert_eq!(
            span.project_spans(&observation())
                .expect("representable spans"),
            vec![
                v2::SpanSignal::Started(expected_started.clone()),
                v2::SpanSignal::Event(expected_event),
                v2::SpanSignal::Ended(
                    expected_started.end(SpanStatus::Error, DurationMs::from(12))
                ),
            ]
        );
        assert_eq!(
            metric
                .project_metrics(&observation())
                .expect("representable metrics"),
            vec![
                canonical_metric(v2::MetricValue::Sum {
                    value: finite(4.0),
                    monotonic: true,
                    temporality: v2::AggregationTemporality::Cumulative,
                    start_time: later(),
                }),
                canonical_metric(v2::MetricValue::Gauge(finite(-2.5))),
            ]
        );
    }

    #[test]
    fn released_scalar_histogram_and_invalid_values_fail_canonical_conversion() {
        for (metric, message) in [
            (
                released_metric(MetricKind::Histogram, 7.0),
                "released scalar histogram has no canonical bucket distribution",
            ),
            (
                released_metric(MetricKind::Gauge, f64::NAN),
                "released metric value is not finite",
            ),
            (
                released_metric(MetricKind::Counter, -1.0),
                "released metric violates the canonical metric contract",
            ),
        ] {
            let (_, projector) = to_canonical(Released(Vec::new(), vec![metric]));
            let error = projector
                .project_metrics(&observation())
                .expect_err("unrepresentable released metric");
            assert!(matches!(error, v2::ProjectionError::Projection { .. }));
            assert_eq!(error.diagnostic().message, message);
            assert_eq!(
                error.diagnostic().code,
                error_codes::VALUE_VALIDATION_FAILED
            );
        }
    }

    #[test]
    fn canonical_models_convert_to_released_when_representable() {
        let started = canonical_started(0);
        let (span, metric) = to_released(Canonical(
            vec![
                v2::SpanSignal::Started(started.clone()),
                v2::SpanSignal::Ended(started.end(SpanStatus::Ok, DurationMs::from(9))),
            ],
            vec![canonical_metric(v2::MetricValue::Sum {
                value: finite(4.0),
                monotonic: true,
                temporality: v2::AggregationTemporality::Cumulative,
                start_time: later(),
            })],
        ));

        let spans = span.project_spans(&observation()).expect("representable");
        let SpanSignal::Ended(ended) = &spans[1] else {
            panic!("second signal is the ended span");
        };
        assert_eq!(ended.trace(), &released_trace());
        assert_eq!(ended.duration_ms(), Some(DurationMs::from(9)));
        assert_eq!(ended.attributes().get("count"), Some(&json!(3)));
        assert_eq!(
            metric
                .project_metrics(&observation())
                .expect("representable"),
            vec![released_metric(MetricKind::Counter, 4.0)]
        );
    }

    #[test]
    fn canonical_trace_fields_and_histograms_fail_released_conversion() {
        let link = v2::SpanLink::new(
            TraceId::new(TRACE).expect("trace"),
            SpanId::new(PARENT).expect("span"),
            v2::TraceFlags::new(1),
            v2::Attributes::new(),
        );
        for (signal, message) in [
            (
                canonical_started(1),
                "canonical trace flags have no released representation",
            ),
            (
                canonical_started(0).with_kind(v2::SpanKind::Server),
                "canonical span kind has no released representation",
            ),
            (
                canonical_started(0).with_links(vec![link]),
                "canonical span links have no released representation",
            ),
        ] {
            let (span, _) =
                to_released(Canonical(vec![v2::SpanSignal::Started(signal)], Vec::new()));
            let error = span
                .project_spans(&observation())
                .expect_err("unrepresentable canonical span");
            assert_eq!(error.0.diagnostic().message, message);
        }

        let histogram =
            v2::HistogramPoint::try_new(vec![finite(10.0)], vec![1, 2], 3, finite(42.0))
                .expect("valid histogram");
        for (value, message) in [
            (
                v2::MetricValue::Histogram {
                    point: histogram,
                    temporality: v2::AggregationTemporality::Delta,
                    start_time: Timestamp::UNIX_EPOCH,
                },
                "canonical histogram buckets have no released representation",
            ),
            (
                v2::MetricValue::Sum {
                    value: finite(4.0),
                    monotonic: true,
                    temporality: v2::AggregationTemporality::Delta,
                    start_time: Timestamp::UNIX_EPOCH,
                },
                "canonical sum interval has no released representation",
            ),
        ] {
            let (_, metric) = to_released(Canonical(Vec::new(), vec![canonical_metric(value)]));
            let error = metric
                .project_metrics(&observation())
                .expect_err("unrepresentable canonical metric");
            assert_eq!(error.0.diagnostic().message, message);
            assert_eq!(
                error.0.diagnostic().code,
                error_codes::VALUE_VALIDATION_FAILED
            );
        }
    }

    #[test]
    fn nested_released_attributes_convert_recursively_on_every_span_and_metric_path() {
        let nested = Map::from_iter([(
            "nested".to_owned(),
            json!({ "values": [1, u64::MAX, -2, 2.5, "text", null, true, { "deep": [3] }] }),
        )]);
        let expected = v2::Attributes::from([(
            "nested".to_owned(),
            v2::AttributeValue::Object(v2::Attributes::from([(
                "values".to_owned(),
                v2::AttributeValue::Array(vec![
                    v2::AttributeValue::Int(1),
                    v2::AttributeValue::UInt(u64::MAX),
                    v2::AttributeValue::Int(-2),
                    v2::AttributeValue::Float(v2::FiniteF64::new(2.5).expect("finite")),
                    v2::AttributeValue::String("text".to_owned()),
                    v2::AttributeValue::Null,
                    v2::AttributeValue::Bool(true),
                    v2::AttributeValue::Object(v2::Attributes::from([(
                        "deep".to_owned(),
                        v2::AttributeValue::Array(vec![v2::AttributeValue::Int(3)]),
                    )])),
                ]),
            )])),
        )]);
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service(),
            ActionName::new("conversion.run").expect("action"),
            released_trace(),
            nested.clone(),
        );
        let event = SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: released_trace(),
            name: ActionName::new("conversion.event").expect("event"),
            attributes: nested.clone(),
            diagnostic: None,
        };
        let ended = started.clone().end(SpanStatus::Ok, DurationMs::from(4));
        let mut metric = released_metric(MetricKind::Gauge, 1.0);
        metric.attributes = nested;
        let (spans, metrics) = to_canonical(Released(
            vec![
                SpanSignal::Started(started),
                SpanSignal::Event(event),
                SpanSignal::Ended(ended),
            ],
            vec![metric],
        ));

        let spans = spans.project_spans(&observation()).expect("spans convert");
        let [
            v2::SpanSignal::Started(started),
            v2::SpanSignal::Event(event),
            v2::SpanSignal::Ended(ended),
        ] = spans.as_slice()
        else {
            panic!("three converted span signals: {spans:?}");
        };
        assert_eq!(started.attributes(), &expected);
        assert_eq!(event.attributes, expected);
        assert_eq!(ended.attributes(), &expected);
        let metrics = metrics
            .project_metrics(&observation())
            .expect("metric converts");
        assert_eq!(metrics[0].attributes(), &expected);
    }
}
