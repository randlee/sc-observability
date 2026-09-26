//! Contract tests for consumers of the staged canonical surface.
use sc_observability_types::v2::*;
use sc_observability_types::{DiagnosticInfo, ErrorContext, Remediation, Timestamp, error_codes};
use serde_json::json;
use std::error::Error;

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
        ($name:ident::$variant:ident, $code:expr) => {{
            let original = context($code);
            let diagnostic = original.diagnostic().clone();
            let pointer = std::ptr::from_ref(&*original);
            let error = $name::$variant { context: original };
            assert_eq!(std::ptr::from_ref(error.context()), pointer);
            assert_eq!(DiagnosticInfo::diagnostic(&error), &diagnostic);
            let original_source = error
                .source()
                .unwrap()
                .source()
                .unwrap()
                .downcast_ref::<Sentinel>()
                .unwrap();
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
    check!(IdentityError::Process, error_codes::DIAGNOSTIC_INVALID);
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
    check!(ExportError::Transport, error_codes::DIAGNOSTIC_INVALID);
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
fn stable_failure_codes() {
    let mut seen = std::collections::HashSet::new();
    for code in error_codes::ALL {
        assert!(seen.insert(code.as_str()), "duplicate {code}");
    }
    assert_eq!(error_codes::otlp::ALL.len(), 26);
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
        context: context(error_codes::otlp::OTLP_QUEUE_FULL),
    }
    .into();
    assert_eq!(telemetry.code().as_str(), "OTLP_QUEUE_FULL");
    assert!(matches!(
        &telemetry,
        TelemetryError::ExportFailure(ExportError::QueueFull { .. })
    ));
    assert!(
        telemetry
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<Sentinel>()
    );
    assert_eq!(
        TelemetryError::Shutdown.code().as_str(),
        "OTLP_TELEMETRY_SHUTDOWN"
    );
}
fn finite(n: f64) -> FiniteF64 {
    FiniteF64::new(n).unwrap()
}
fn histogram() -> HistogramPoint {
    HistogramPoint::try_new(vec![1.0, 2.0], vec![1, 2, 3], 6, finite(12.0)).unwrap()
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
    assert!(HistogramPoint::try_new(vec![f64::INFINITY], vec![0, 0], 0, finite(0.0)).is_err());
    assert!(HistogramPoint::try_new(vec![], vec![0], 0, finite(1.0)).is_err());
    assert!(HistogramPoint::try_new(vec![], vec![0], 0, finite(0.0)).is_ok());
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteF64::new(value).is_err());
    }
}
#[test]
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
    let mut value = serde_json::to_value(&record).unwrap();
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
    assert_eq!(record.duration_ms().unwrap().as_u64(), 42);
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
