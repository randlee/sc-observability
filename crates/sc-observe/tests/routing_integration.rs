#![cfg(feature = "v1")]
#![allow(
    deprecated,
    reason = "routing integration compatibility fixtures exercise the retained trait errors"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability::LoggerConfig;
use sc_observability::v2::LogSink;
use sc_observability_types::v2::{
    LogProjector, ObservationSubscriber, ProjectionError, ProjectionRegistration, SubscriberError,
    SubscriberRegistration,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, ErrorContext, Level, LogEvent,
    LogProjector as LegacyLogProjector, Observation,
    ObservationSubscriber as LegacyObservationSubscriber, OutcomeLabel, ProcessIdentity,
    ProjectionRegistration as LegacyProjectionRegistration, Remediation, SchemaVersion,
    ServiceName, SinkHealth, SinkHealthState, SinkName, SpanId,
    SubscriberRegistration as LegacySubscriberRegistration, TargetCategory, Timestamp,
    TraceContext as LegacyTraceContext, TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};
use serde_json::Map;

#[derive(Debug, Clone)]
struct AgentEvent {
    kind: &'static str,
}

struct RecordingLogSubscriber {
    deliveries: Arc<AtomicU64>,
}

impl ObservationSubscriber<LogEvent> for RecordingLogSubscriber {
    fn observe(&self, _observation: &Observation<LogEvent>) -> Result<(), SubscriberError> {
        self.deliveries.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct ForwardingLogSink {
    consumer: Arc<sc_observe::v2::Observability>,
}

impl sc_observability::v2::LogSink for ForwardingLogSink {
    fn write(&self, event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
        self.consumer
            .emit(Observation::new(event.service.clone(), event.clone()))
            .map_err(|source| sc_observability_types::v2::LogSinkError::Write {
                context: Box::new(
                    ErrorContext::new(
                        sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE,
                        "test-local forwarding consumer rejected the event",
                        Remediation::not_recoverable("rebuild the closed observation consumer"),
                    )
                    .source(Box::new(source)),
                ),
            })
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("routing-integration-forwarder").expect("valid sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
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

impl LegacyObservationSubscriber<AgentEvent> for RecordingSubscriber {
    fn observe(&self, observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
        <Self as ObservationSubscriber<AgentEvent>>::observe(self, observation)
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

impl LegacyLogProjector<AgentEvent> for RecordingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        <Self as LogProjector<AgentEvent>>::project_logs(self, observation)
    }
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

impl LegacyLogProjector<AgentEvent> for FailingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<AgentEvent>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        <Self as LogProjector<AgentEvent>>::project_logs(self, observation)
    }
}

fn tool_name() -> sc_observability_types::ToolName {
    sc_observability_types::ToolName::new("obs-app").expect("valid tool name")
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
fn one_observation_can_fan_out_to_subscribers_and_logs() {
    let subscriber_calls = Arc::new(Mutex::new(Vec::new()));
    let log_calls = Arc::new(Mutex::new(Vec::new()));
    let legacy_root = temp_path("fanout-legacy");
    let legacy_config =
        ObservabilityConfig::default_for(tool_name(), legacy_root.clone()).expect("config");
    let legacy = Observability::builder(legacy_config)
        .register_subscriber(LegacySubscriberRegistration::new(Arc::new(
            RecordingSubscriber {
                id: "subscriber",
                calls: subscriber_calls.clone(),
            },
        )))
        .register_projection(
            LegacyProjectionRegistration::new().with_log_projector(Arc::new(
                RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                },
            )),
        )
        .build()
        .expect("legacy runtime");

    let typed_root = temp_path("fanout-typed");
    let typed_config =
        sc_observe::v2::ObservabilityConfig::default_for(tool_name(), typed_root.clone())
            .expect("config");
    let typed = sc_observe::v2::Observability::builder(typed_config)
        .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
            id: "subscriber",
            calls: subscriber_calls.clone(),
        })))
        .register_projection(ProjectionRegistration::new().with_log_projector(Arc::new(
            RecordingLogProjector {
                calls: log_calls.clone(),
                id: "log",
            },
        )))
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

impl LegacyObservationSubscriber<AgentEvent> for FailingSubscriber {
    fn observe(&self, observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
        <Self as ObservationSubscriber<AgentEvent>>::observe(self, observation)
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

fn legacy_failing_log_projection(cause: &'static str) -> LegacyProjectionRegistration<AgentEvent> {
    LegacyProjectionRegistration::new().with_log_projector(Arc::new(FailingLogProjector {
        context: Mutex::new(Some(routing_failure_context(cause))),
    }))
}

fn delivering_subscriber() -> SubscriberRegistration<AgentEvent> {
    SubscriberRegistration::new(Arc::new(RecordingSubscriber {
        id: "delivered",
        calls: Arc::new(Mutex::new(Vec::new())),
    }))
}

fn legacy_delivering_subscriber() -> LegacySubscriberRegistration<AgentEvent> {
    LegacySubscriberRegistration::new(Arc::new(RecordingSubscriber {
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

fn forwarding_log_event() -> LogEvent {
    LogProjector::project_logs(
        &RecordingLogProjector {
            calls: Arc::new(Mutex::new(Vec::new())),
            id: "forwarding",
        },
        &observation(),
    )
    .expect("forwarding log event")
    .into_iter()
    .next()
    .expect("one forwarding log event")
}

#[test]
fn forwarding_sink_delivery_and_closed_rejection_update_core_writer_health() {
    let deliveries = Arc::new(AtomicU64::new(0));
    let consumer_config = sc_observe::v2::ObservabilityConfig::default_for(
        tool_name(),
        temp_path("forwarding-consumer"),
    )
    .expect("consumer config");
    let consumer = Arc::new(
        sc_observe::v2::Observability::builder(consumer_config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(
                RecordingLogSubscriber {
                    deliveries: deliveries.clone(),
                },
            )))
            .build()
            .expect("forwarding consumer"),
    );

    let sink = Arc::new(ForwardingLogSink {
        consumer: consumer.clone(),
    });
    let mut core_config = LoggerConfig::default_for(
        ServiceName::new("obs-app").expect("valid core service"),
        temp_path("forwarding-core"),
    );
    core_config.enable_file_sink = false;
    core_config.enable_console_sink = false;
    let mut builder = sc_observability::v2::Logger::builder(core_config).expect("core builder");
    builder.register_sink(sc_observability::SinkRegistration::typed(sink.clone()));
    let logger = builder.build().expect("core logger");
    let event = forwarding_log_event();

    logger
        .log(event.clone())
        .expect("open forwarding admission");
    logger
        .flush_with_timeout(std::time::Duration::from_secs(5))
        .expect("open forwarding barrier");
    assert_eq!(deliveries.load(Ordering::SeqCst), 1);
    assert!(logger.health().last_writer_error.is_none());

    consumer.shutdown().expect("close forwarding consumer");
    let rejection = sink
        .write(&event)
        .expect_err("closed consumer rejects forwarding");
    assert!(matches!(
        std::error::Error::source(&rejection)
            .expect("closed rejection source")
            .downcast_ref::<sc_observability_types::ObservationError>(),
        Some(sc_observability_types::ObservationError::Shutdown)
    ));
    let expected = rejection.diagnostic().clone();

    logger
        .log(event)
        .expect("queue admission precedes sink failure");
    logger
        .flush_with_timeout(std::time::Duration::from_secs(5))
        .expect("closed forwarding barrier");
    let health = logger.health();
    assert_eq!(health.dropped_events_total, 1);
    assert_eq!(
        health
            .last_writer_error
            .expect("writer degraded by closed forwarding")
            .code,
        Some(sc_observability::error_codes::LOGGER_WRITER_DEGRADED)
    );
    let last_error = health.last_error.expect("typed forwarding summary");
    assert_eq!(
        last_error.code.as_ref().map(ErrorCode::as_str),
        Some(expected.code.as_str())
    );
    assert_eq!(last_error.message, expected.message);
    logger.shutdown().expect("core shutdown");
}

/// Released root-error implementations route through both facades with their
/// own diagnostics, and a canonical registration routes through the v2 facade.
#[test]
fn released_and_canonical_registrations_route_failures_through_both_facades() {
    let released_config =
        ObservabilityConfig::default_for(tool_name(), temp_path("released-trait-failures"))
            .expect("config");
    let released = Observability::builder(released_config)
        .register_subscriber(LegacySubscriberRegistration::new(failing_subscriber(
            "released",
        )))
        .register_subscriber(legacy_delivering_subscriber())
        .register_projection(legacy_failing_log_projection("released projector"))
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
