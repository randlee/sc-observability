#![allow(
    deprecated,
    reason = "routing integration compatibility fixtures exercise the retained trait errors"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability_types::typed::{
    ClassifiedError, ProjectionFailureKind, SubscriberFailureKind, typed_log_projector,
    typed_metric_projector, typed_span_projector, typed_subscriber,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, ErrorContext, Level, LogEvent, MetricKind, MetricName,
    MetricUnit, Observation, ObservationSubscriber, OutcomeLabel, ProcessIdentity,
    ProjectionRegistration, Remediation, SchemaVersion, ServiceName, SpanId, SpanProjector,
    SpanRecord, SpanSignal, SpanStarted, SubscriberRegistration, TargetCategory, Timestamp,
    TraceContext, TraceId,
};
use sc_observability_types::{ProjectionError, SubscriberError};
use sc_observe::{Observability, ObservabilityConfig};
use serde_json::Map;

#[derive(Debug, Clone)]
struct AgentEvent {
    kind: &'static str,
}

struct RecordingSubscriber {
    id: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl ObservationSubscriber<AgentEvent> for RecordingSubscriber {
    fn observe(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<(), sc_observability_types::SubscriberError> {
        self.calls.lock().expect("calls poisoned").push(self.id);
        Ok(())
    }
}

struct RecordingLogProjector {
    calls: Arc<Mutex<Vec<&'static str>>>,
    id: &'static str,
}

impl sc_observability_types::LogProjector<AgentEvent> for RecordingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::ProjectionError> {
        self.calls.lock().expect("calls poisoned").push(self.id);
        Ok(vec![LogEvent {
            version: SchemaVersion::new(
                sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
            )
            .expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: observation.service.clone(),
            target: TargetCategory::new("observe.routing").expect("valid target"),
            action: ActionName::new("observation.received").expect("valid action"),
            message: Some(observation.payload.kind.to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace_context()),
            request_id: None,
            correlation_id: None,
            outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
            diagnostic: Some(Diagnostic {
                timestamp: Timestamp::UNIX_EPOCH,
                code: ErrorCode::new_static("SC_TEST"),
                message: "projected".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect log output"]),
                docs: None,
                details: Map::default(),
            }),
            state_transition: None,
            fields: Map::default(),
        }])
    }
}

struct RecordingSpanProjector {
    count: Arc<AtomicU64>,
}

impl SpanProjector<AgentEvent> for RecordingSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<SpanSignal>, sc_observability_types::ProjectionError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            observation.service.clone(),
            ActionName::new("span.started").expect("valid action"),
            trace_context(),
            Map::default(),
        ))])
    }
}

struct RecordingMetricProjector {
    count: Arc<AtomicU64>,
}

struct FailingSubscriber {
    context: Mutex<Option<Box<ErrorContext>>>,
}

impl ObservationSubscriber<AgentEvent> for FailingSubscriber {
    fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
        Err(SubscriberError(
            self.context
                .lock()
                .expect("subscriber context poisoned")
                .take()
                .expect("subscriber fixture invoked once"),
        ))
    }
}

struct FailingLogProjector {
    context: Mutex<Option<Box<ErrorContext>>>,
}

impl sc_observability_types::LogProjector<AgentEvent> for FailingLogProjector {
    fn project_logs(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        Err(ProjectionError(
            self.context
                .lock()
                .expect("log projector context poisoned")
                .take()
                .expect("log projector fixture invoked once"),
        ))
    }
}

struct FailingSpanProjector {
    context: Mutex<Option<Box<ErrorContext>>>,
}

impl SpanProjector<AgentEvent> for FailingSpanProjector {
    fn project_spans(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        Err(ProjectionError(
            self.context
                .lock()
                .expect("span projector context poisoned")
                .take()
                .expect("span projector fixture invoked once"),
        ))
    }
}

struct FailingMetricProjector {
    context: Mutex<Option<Box<ErrorContext>>>,
}

impl sc_observability_types::MetricProjector<AgentEvent> for FailingMetricProjector {
    fn project_metrics(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<Vec<sc_observability_types::MetricRecord>, ProjectionError> {
        Err(ProjectionError(
            self.context
                .lock()
                .expect("metric projector context poisoned")
                .take()
                .expect("metric projector fixture invoked once"),
        ))
    }
}

impl sc_observability_types::MetricProjector<AgentEvent> for RecordingMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<sc_observability_types::MetricRecord>, sc_observability_types::ProjectionError>
    {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![sc_observability_types::MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: observation.service.clone(),
            name: MetricName::new("obs.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some(MetricUnit::new("1").expect("valid metric unit")),
            attributes: Map::default(),
        }])
    }
}

fn tool_name() -> sc_observability_types::ToolName {
    sc_observability_types::ToolName::new("obs-app").expect("valid tool name")
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    }
}

fn observation() -> Observation<AgentEvent> {
    let mut observation = Observation::new(
        ServiceName::new("obs-app").expect("valid service"),
        AgentEvent { kind: "received" },
    );
    observation.identity = ProcessIdentity::default();
    observation
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sc-observe-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ))
}

fn routing_failure_context(cause: &'static str) -> Box<ErrorContext> {
    Box::new(
        ErrorContext::new(
            ErrorCode::new_static("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"),
            "routing fixture failed",
            Remediation::not_recoverable("inspect the routing fixture"),
        )
        .source(Box::new(std::io::Error::other(cause))),
    )
}

fn assert_routing_context(
    context: &ErrorContext,
    original_context: *const ErrorContext,
    original_source: *const std::io::Error,
    cause: &str,
) {
    assert_eq!(std::ptr::from_ref(context), original_context);
    assert_eq!(
        context.diagnostic().code.as_str(),
        "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
    );
    let source = std::error::Error::source(context)
        .expect("routing source")
        .downcast_ref::<std::io::Error>()
        .expect("routing io source");
    assert_eq!(std::ptr::from_ref(source), original_source);
    assert_eq!(source.to_string(), cause);
}

#[test]
fn one_observation_can_fan_out_to_subscribers_logs_spans_and_metrics() {
    let subscriber_calls = Arc::new(Mutex::new(Vec::new()));
    let log_calls = Arc::new(Mutex::new(Vec::new()));
    let span_count = Arc::new(AtomicU64::new(0));
    let metric_count = Arc::new(AtomicU64::new(0));
    let legacy_root = temp_path("fanout-legacy");
    let legacy_config =
        ObservabilityConfig::default_for(tool_name(), legacy_root.clone()).expect("config");
    let legacy = Observability::builder(legacy_config)
        .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
            id: "subscriber",
            calls: subscriber_calls.clone(),
        })))
        .register_projection(
            ProjectionRegistration::new()
                .with_log_projector(Arc::new(RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                }))
                .with_span_projector(Arc::new(RecordingSpanProjector {
                    count: span_count.clone(),
                }))
                .with_metric_projector(Arc::new(RecordingMetricProjector {
                    count: metric_count.clone(),
                })),
        )
        .build()
        .expect("legacy runtime");

    let typed_root = temp_path("fanout-typed");
    let typed_config =
        ObservabilityConfig::default_for(tool_name(), typed_root.clone()).expect("config");
    let typed = Observability::builder(typed_config)
        .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
            id: "subscriber",
            calls: subscriber_calls.clone(),
        })))
        .register_projection(
            ProjectionRegistration::new()
                .with_log_projector(Arc::new(RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                }))
                .with_span_projector(Arc::new(RecordingSpanProjector {
                    count: span_count.clone(),
                }))
                .with_metric_projector(Arc::new(RecordingMetricProjector {
                    count: metric_count.clone(),
                })),
        )
        .build()
        .expect("typed runtime");

    legacy.emit(observation()).expect("legacy emit");
    typed.emit(observation()).expect("typed emit");

    let legacy_log_path = legacy_root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "obs-app{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let typed_log_path = typed_root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "obs-app{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let legacy_contents =
        std::fs::read_to_string(legacy_log_path).expect("read legacy projected log file");
    let typed_contents =
        std::fs::read_to_string(typed_log_path).expect("read typed projected log file");

    assert_eq!(
        *subscriber_calls.lock().expect("subscriber calls poisoned"),
        vec!["subscriber", "subscriber"]
    );
    assert_eq!(
        *log_calls.lock().expect("log calls poisoned"),
        vec!["log", "log"]
    );
    assert_eq!(span_count.load(Ordering::SeqCst), 2);
    assert_eq!(metric_count.load(Ordering::SeqCst), 2);
    assert!(legacy_contents.contains("\"action\":\"observation.received\""));
    assert!(typed_contents.contains("\"action\":\"observation.received\""));
    legacy.shutdown().expect("legacy shutdown");
    typed.shutdown().expect("typed shutdown");
}

#[test]
fn typed_routing_adapters_preserve_canonical_variants_codes_and_sources() {
    let subscriber_context = routing_failure_context("subscriber source");
    let subscriber_context_ptr = std::ptr::from_ref(subscriber_context.as_ref());
    let subscriber_source_ptr = std::ptr::from_ref(
        std::error::Error::source(subscriber_context.as_ref())
            .expect("subscriber source")
            .downcast_ref::<std::io::Error>()
            .expect("subscriber io source"),
    );
    let subscriber = typed_subscriber(Arc::new(FailingSubscriber {
        context: Mutex::new(Some(subscriber_context)),
    }));
    let subscriber_error = subscriber
        .observe(&observation())
        .expect_err("subscriber should fail");
    assert_eq!(subscriber_error.kind(), SubscriberFailureKind::Routing);
    assert_routing_context(
        subscriber_error.context(),
        subscriber_context_ptr,
        subscriber_source_ptr,
        "subscriber source",
    );

    let log_context = routing_failure_context("log projector source");
    let log_context_ptr = std::ptr::from_ref(log_context.as_ref());
    let log_source_ptr = std::ptr::from_ref(
        std::error::Error::source(log_context.as_ref())
            .expect("log projector source")
            .downcast_ref::<std::io::Error>()
            .expect("log projector io source"),
    );
    let log_projector = typed_log_projector(Arc::new(FailingLogProjector {
        context: Mutex::new(Some(log_context)),
    }));
    let log_error = log_projector
        .project_logs(&observation())
        .expect_err("log projector should fail");
    assert_eq!(log_error.kind(), ProjectionFailureKind::Routing);
    assert_routing_context(
        log_error.context(),
        log_context_ptr,
        log_source_ptr,
        "log projector source",
    );

    let span_context = routing_failure_context("span projector source");
    let span_context_ptr = std::ptr::from_ref(span_context.as_ref());
    let span_source_ptr = std::ptr::from_ref(
        std::error::Error::source(span_context.as_ref())
            .expect("span projector source")
            .downcast_ref::<std::io::Error>()
            .expect("span projector io source"),
    );
    let span_projector = typed_span_projector(Arc::new(FailingSpanProjector {
        context: Mutex::new(Some(span_context)),
    }));
    let span_error = span_projector
        .project_spans(&observation())
        .expect_err("span projector should fail");
    assert_eq!(span_error.kind(), ProjectionFailureKind::Routing);
    assert_routing_context(
        span_error.context(),
        span_context_ptr,
        span_source_ptr,
        "span projector source",
    );

    let metric_context = routing_failure_context("metric projector source");
    let metric_context_ptr = std::ptr::from_ref(metric_context.as_ref());
    let metric_source_ptr = std::ptr::from_ref(
        std::error::Error::source(metric_context.as_ref())
            .expect("metric projector source")
            .downcast_ref::<std::io::Error>()
            .expect("metric projector io source"),
    );
    let metric_projector = typed_metric_projector(Arc::new(FailingMetricProjector {
        context: Mutex::new(Some(metric_context)),
    }));
    let metric_error = metric_projector
        .project_metrics(&observation())
        .expect_err("metric projector should fail");
    assert_eq!(metric_error.kind(), ProjectionFailureKind::Routing);
    assert_routing_context(
        metric_error.context(),
        metric_context_ptr,
        metric_source_ptr,
        "metric projector source",
    );
}

impl sc_observability_types::v2::ObservationSubscriber<AgentEvent> for FailingSubscriber {
    fn observe(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<(), sc_observability_types::v2::SubscriberError> {
        Err(sc_observability_types::v2::SubscriberError::Subscriber {
            context: self
                .context
                .lock()
                .expect("subscriber context poisoned")
                .take()
                .expect("subscriber fixture invoked once"),
        })
    }
}

fn failing_subscriber(cause: &'static str) -> Arc<FailingSubscriber> {
    Arc::new(FailingSubscriber {
        context: Mutex::new(Some(routing_failure_context(cause))),
    })
}

fn failing_log_projection(cause: &'static str) -> ProjectionRegistration<AgentEvent> {
    ProjectionRegistration::new().with_log_projector(Arc::new(FailingLogProjector {
        context: Mutex::new(Some(routing_failure_context(cause))),
    }))
}

fn delivering_subscriber() -> SubscriberRegistration<AgentEvent> {
    SubscriberRegistration::new(Arc::new(RecordingSubscriber {
        id: "delivered",
        calls: Arc::new(Mutex::new(Vec::new())),
    }))
}

fn assert_routed_failures(
    health: &sc_observe::ObservabilityHealthReport,
    subscriber_failures: u64,
) {
    assert_eq!(health.subscriber_failures_total, subscriber_failures);
    assert_eq!(health.projection_failures_total, 1);
    let last_error = health.last_error.as_ref().expect("routed failure summary");
    assert_eq!(
        last_error.code.as_ref().map(ErrorCode::as_str),
        Some("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE")
    );
    assert_eq!(last_error.message, "routing fixture failed");
}

/// Released root-error implementations route through both facades with their
/// own diagnostics, and a canonical registration routes through the v2 facade.
#[test]
fn released_and_canonical_registrations_route_failures_through_both_facades() {
    let released_config =
        ObservabilityConfig::default_for(tool_name(), temp_path("released-trait-failures"))
            .expect("config");
    let released = Observability::builder(released_config)
        .register_subscriber(SubscriberRegistration::new(failing_subscriber("released")))
        .register_subscriber(delivering_subscriber())
        .register_projection(failing_log_projection("released projector"))
        .build()
        .expect("released runtime");
    released.emit(observation()).expect("delivered");
    assert_routed_failures(&released.health(), 1);

    let canonical_config = sc_observe::v2::ObservabilityConfig::default_for(
        tool_name(),
        temp_path("canonical-trait-failures"),
    )
    .expect("config");
    let canonical_subscriber: Arc<
        dyn sc_observability_types::v2::ObservationSubscriber<AgentEvent>,
    > = failing_subscriber("canonical");
    let canonical = sc_observe::v2::Observability::builder(canonical_config)
        .register_subscriber(SubscriberRegistration::new(failing_subscriber("converted")).into())
        .register_subscriber(sc_observability_types::v2::SubscriberRegistration::new(
            canonical_subscriber,
        ))
        .register_subscriber(delivering_subscriber().into())
        .register_projection(failing_log_projection("converted projector").into())
        .build()
        .expect("canonical runtime");
    canonical.emit(observation()).expect("delivered");
    assert_routed_failures(&canonical.health(), 2);
}
