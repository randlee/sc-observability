use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sc_observability_types::typed::{
    ClassifiedError, InitFailureKind, ProjectionFailure, SubscriberFailure, TypedLogProjector,
    TypedMetricProjector, TypedObservationSubscriber, TypedSpanProjector, legacy_log_projector,
    legacy_metric_projector, legacy_span_projector, legacy_subscriber,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricKind, MetricName, MetricRecord,
    MetricUnit, Observation, ProcessIdentity, ProjectionRegistration, Remediation, SchemaVersion,
    ServiceName, SpanId, SpanRecord, SpanSignal, SpanStarted, SubscriberRegistration,
    TargetCategory, Timestamp, ToolName, TraceContext, TraceId,
};
use sc_observe::Observability;
use serde_json::Map;

#[derive(Debug, Clone)]
struct ObservationPayload {
    message: &'static str,
}

struct CountingSubscriber {
    calls: Arc<AtomicUsize>,
}

impl TypedObservationSubscriber<ObservationPayload> for CountingSubscriber {
    fn observe(
        &self,
        _observation: &Observation<ObservationPayload>,
    ) -> Result<(), SubscriberFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct CountingLogProjector {
    calls: Arc<AtomicUsize>,
}

impl TypedLogProjector<ObservationPayload> for CountingLogProjector {
    fn project_logs(
        &self,
        observation: &Observation<ObservationPayload>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![log_event(observation, observation.payload.message)])
    }
}

struct CountingSpanProjector {
    calls: Arc<AtomicUsize>,
}

impl TypedSpanProjector<ObservationPayload> for CountingSpanProjector {
    fn project_spans(
        &self,
        observation: &Observation<ObservationPayload>,
    ) -> Result<Vec<SpanSignal>, ProjectionFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            observation.service.clone(),
            ActionName::new("observation.started").expect("valid action"),
            trace_context(),
            Map::new(),
        ))])
    }
}

struct CountingMetricProjector {
    calls: Arc<AtomicUsize>,
}

impl TypedMetricProjector<ObservationPayload> for CountingMetricProjector {
    fn project_metrics(
        &self,
        observation: &Observation<ObservationPayload>,
    ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: observation.service.clone(),
            name: MetricName::new("observation.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some(MetricUnit::new("1").expect("valid unit")),
            attributes: Map::new(),
        }])
    }
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "sc-observe-typed-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ))
}

fn observation() -> Observation<ObservationPayload> {
    let mut observation = Observation::new(
        ServiceName::new("typed-observe").expect("valid service"),
        ObservationPayload {
            message: "received",
        },
    );
    observation.identity = ProcessIdentity::default();
    observation
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    }
}

fn log_event(observation: &Observation<ObservationPayload>, message: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(sc_observability_types::OBSERVATION_ENVELOPE_VERSION)
            .expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: observation.service.clone(),
        target: TargetCategory::new("observe.routing").expect("valid target"),
        action: ActionName::new("observation.received").expect("valid action"),
        message: Some(message.to_owned()),
        identity: ProcessIdentity::default(),
        trace: Some(trace_context()),
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST_OBSERVATION"),
            message: "typed observation projected".to_owned(),
            cause: None,
            remediation: Remediation::recoverable(
                "inspect the projected event",
                Vec::<String>::new(),
            ),
            docs: None,
            details: Map::new(),
        }),
        state_transition: None,
        fields: Map::new(),
    }
}

#[test]
fn typed_routes_execute_real_subscriber_and_projector_adapters() {
    let root = temp_path("routes");
    let subscriber_calls = Arc::new(AtomicUsize::new(0));
    let log_calls = Arc::new(AtomicUsize::new(0));
    let span_calls = Arc::new(AtomicUsize::new(0));
    let metric_calls = Arc::new(AtomicUsize::new(0));
    let config = sc_observe::ObservabilityConfig::default_for_typed(
        ToolName::new("typed-observe").expect("valid tool"),
        root.clone(),
    )
    .expect("typed config");

    let runtime = Observability::builder(config)
        .register_subscriber(SubscriberRegistration::new(legacy_subscriber(Arc::new(
            CountingSubscriber {
                calls: subscriber_calls.clone(),
            },
        ))))
        .register_projection(
            ProjectionRegistration::new()
                .with_log_projector(legacy_log_projector(Arc::new(CountingLogProjector {
                    calls: log_calls.clone(),
                })))
                .with_span_projector(legacy_span_projector(Arc::new(CountingSpanProjector {
                    calls: span_calls.clone(),
                })))
                .with_metric_projector(legacy_metric_projector(Arc::new(
                    CountingMetricProjector {
                        calls: metric_calls.clone(),
                    },
                ))),
        )
        .build_typed()
        .expect("typed runtime");

    runtime.emit(observation()).expect("typed emit");
    runtime.flush_typed().expect("typed flush");
    assert_eq!(subscriber_calls.load(Ordering::SeqCst), 1);
    assert_eq!(log_calls.load(Ordering::SeqCst), 1);
    assert_eq!(span_calls.load(Ordering::SeqCst), 1);
    assert_eq!(metric_calls.load(Ordering::SeqCst), 1);

    let log_path = root
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "typed-observe{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let contents = std::fs::read_to_string(log_path).expect("read projected log");
    assert!(contents.contains("observation.received"));
    assert!(contents.contains("received"));

    runtime.shutdown_typed().expect("typed shutdown");
    runtime.shutdown_typed().expect("repeated typed shutdown");
}

#[test]
fn typed_and_legacy_construction_failures_classify_consistently() {
    let empty = match Observability::builder(
        sc_observe::ObservabilityConfig::default_for_typed(
            ToolName::new("typed-observe").expect("valid tool"),
            temp_path("empty"),
        )
        .expect("typed config"),
    )
    .build_typed()
    {
        Err(error) => error,
        Ok(_) => panic!("empty routes must fail"),
    };
    assert_eq!(empty.kind(), InitFailureKind::ObservationInitialization);

    let mut config = sc_observe::ObservabilityConfig::default_for_typed(
        ToolName::new("typed-observe").expect("valid tool"),
        temp_path("logger-failure"),
    )
    .expect("typed config");
    config.queue_capacity = 0;
    let logger_failure = match Observability::builder(config)
        .register_subscriber(SubscriberRegistration::new(legacy_subscriber(Arc::new(
            CountingSubscriber {
                calls: Arc::new(AtomicUsize::new(0)),
            },
        ))))
        .build_typed()
    {
        Err(error) => error,
        Ok(_) => panic!("zero queue capacity must fail"),
    };
    assert_eq!(logger_failure.kind(), InitFailureKind::LoggerInitialization);
}
