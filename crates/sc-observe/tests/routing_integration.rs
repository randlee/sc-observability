#![cfg(feature = "v1")]
#![allow(
    deprecated,
    reason = "routing integration compatibility fixtures exercise the retained trait errors"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability_types::v2::{
    AggregationTemporality, Attributes, FiniteF64, LogProjector, MetricProjector, MetricRecord,
    MetricValue, ObservationSubscriber, ProjectionError, ProjectionRegistration, SpanProjector,
    SpanRecord, SpanSignal, SubscriberError, SubscriberRegistration, TraceContext, TraceFlags,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, ErrorContext, Level, LogEvent, MetricName, MetricUnit,
    Observation, OutcomeLabel, ProcessIdentity, Remediation, SchemaVersion, ServiceName, SpanId,
    SpanStarted, TargetCategory, Timestamp, TraceContext as LegacyTraceContext, TraceId,
};
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
    fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
        self.calls.lock().expect("calls poisoned").push(self.id);
        Ok(())
    }
}

struct RecordingLogProjector {
    calls: Arc<Mutex<Vec<&'static str>>>,
    id: &'static str,
}

impl LogProjector<AgentEvent> for RecordingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
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
            trace: Some(LegacyTraceContext {
                trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
                span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
                parent_span_id: None,
            }),
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
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            observation.service.clone(),
            ActionName::new("span.started").expect("valid action"),
            v2_trace_context(),
            Attributes::new(),
        ))])
    }
}

struct RecordingMetricProjector {
    count: Arc<AtomicU64>,
}

struct FailingSubscriber {
    context: Mutex<Option<Box<ErrorContext>>>,
}

struct FailingLogProjector {
    context: Mutex<Option<Box<ErrorContext>>>,
}

impl LogProjector<AgentEvent> for FailingLogProjector {
    fn project_logs(
        &self,
        _observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        Err(ProjectionError::Projection {
            context: self
                .context
                .lock()
                .expect("log projector context poisoned")
                .take()
                .expect("log projector fixture invoked once"),
        })
    }
}

impl MetricProjector<AgentEvent> for RecordingMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![
            MetricRecord::try_new(
                Timestamp::UNIX_EPOCH,
                observation.service.clone(),
                MetricName::new("obs.events_total").expect("valid metric"),
                MetricValue::Sum {
                    value: FiniteF64::new(1.0).expect("finite metric value"),
                    monotonic: true,
                    temporality: AggregationTemporality::Cumulative,
                    start_time: Timestamp::UNIX_EPOCH,
                },
            )
            .expect("valid cumulative metric")
            .with_unit(Some(MetricUnit::new("1").expect("valid metric unit"))),
        ])
    }
}

fn tool_name() -> sc_observability_types::ToolName {
    sc_observability_types::ToolName::new("obs-app").expect("valid tool name")
}

fn v2_trace_context() -> TraceContext {
    TraceContext::new(
        TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        SpanId::new("0123456789abcdef").expect("valid span id"),
        TraceFlags::new(0),
    )
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

impl ObservationSubscriber<AgentEvent> for FailingSubscriber {
    fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
        Err(SubscriberError::Subscriber {
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
        .register_subscriber(SubscriberRegistration::new(failing_subscriber("converted")))
        .register_subscriber(sc_observability_types::v2::SubscriberRegistration::new(
            canonical_subscriber,
        ))
        .register_subscriber(delivering_subscriber())
        .register_projection(failing_log_projection("converted projector"))
        .build()
        .expect("canonical runtime");
    canonical.emit(observation()).expect("delivered");
    assert_routed_failures(&canonical.health(), 2);
}
