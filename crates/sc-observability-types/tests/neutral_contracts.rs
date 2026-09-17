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
    fn project_logs(&self, _: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionFailure> {
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
    fn project_spans(&self, _: &Observation<String>) -> Result<Vec<SpanSignal>, ProjectionFailure> {
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
    fn project_spans(&self, _: &Observation<String>) -> Result<Vec<SpanSignal>, ProjectionError> {
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
    fn project_logs(&self, _: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionFailure> {
        Ok(vec![log_event()])
    }
}

impl TypedSpanProjector<String> for TypedSuccess {
    fn project_spans(&self, _: &Observation<String>) -> Result<Vec<SpanSignal>, ProjectionFailure> {
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
    fn project_spans(&self, _: &Observation<String>) -> Result<Vec<SpanSignal>, ProjectionError> {
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
