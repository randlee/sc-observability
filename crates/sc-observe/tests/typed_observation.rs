#![allow(
    deprecated,
    reason = "paired routing fixtures exercise both legacy and typed construction paths"
)]

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use sc_observability_types::typed::{
    ClassifiedError, InitFailureKind, ProjectionFailure, SubscriberFailure, TypedLogProjector,
    TypedMetricProjector, TypedObservationSubscriber, TypedSpanProjector, legacy_log_projector,
    legacy_metric_projector, legacy_span_projector, legacy_subscriber, typed_subscriber,
};
use sc_observability_types::{
    ActionName, Diagnostic, DiagnosticInfo, ErrorCode, Level, LogEvent, MetricKind, MetricName,
    MetricRecord, MetricUnit, Observation, ObservationError, ObservationFilter, ProcessIdentity,
    ProjectionRegistration, Remediation, SchemaVersion, ServiceName, SpanId, SpanRecord,
    SpanSignal, SpanStarted, SubscriberRegistration, TargetCategory, Timestamp, ToolName,
    TraceContext, TraceId,
};
use sc_observe::Observability;
use serde_json::{Map, json};

#[derive(Debug, Clone)]
struct ObservationPayload {
    message: &'static str,
    allow: bool,
}

#[derive(Debug, Clone)]
struct OtherObservationPayload;

struct AllowFilter;

impl ObservationFilter<ObservationPayload> for AllowFilter {
    fn accepts(&self, observation: &Observation<ObservationPayload>) -> bool {
        observation.payload.allow
    }
}

struct OrderedSubscriber {
    id: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl TypedObservationSubscriber<ObservationPayload> for OrderedSubscriber {
    fn observe(
        &self,
        _observation: &Observation<ObservationPayload>,
    ) -> Result<(), SubscriberFailure> {
        self.calls.lock().expect("calls poisoned").push(self.id);
        Ok(())
    }
}

struct FailingSubscriber {
    code: ErrorCode,
    calls: Arc<AtomicUsize>,
}

impl TypedObservationSubscriber<ObservationPayload> for FailingSubscriber {
    fn observe(
        &self,
        _observation: &Observation<ObservationPayload>,
    ) -> Result<(), SubscriberFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(SubscriberFailure::from_context(Box::new(
            sc_observability_types::ErrorContext::new(
                self.code.clone(),
                "subscriber fixture failed",
                Remediation::not_recoverable("inspect the subscriber fixture"),
            )
            .source(Box::new(std::io::Error::other("subscriber fixture source"))),
        )))
    }
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
    observation_with(true)
}

fn observation_with(allow: bool) -> Observation<ObservationPayload> {
    let mut observation = Observation::new(
        ServiceName::new("typed-observe").expect("valid service"),
        ObservationPayload {
            message: "received",
            allow,
        },
    );
    observation.identity = ProcessIdentity::default();
    observation
}

fn other_observation() -> Observation<OtherObservationPayload> {
    Observation::new(
        ServiceName::new("typed-observe").expect("valid service"),
        OtherObservationPayload,
    )
}

fn config(name: &str) -> sc_observe::ObservabilityConfig {
    sc_observe::ObservabilityConfig::default_for(
        ToolName::new("typed-observe").expect("valid tool"),
        temp_path(name),
    )
    .expect("config")
}

fn paired_subscriber_runtimes(
    name: &str,
    registrations: Vec<SubscriberRegistration<ObservationPayload>>,
) -> (Observability, Observability) {
    let mut legacy_builder = Observability::builder(config(&format!("{name}-legacy")));
    let mut typed_builder = Observability::builder(config(&format!("{name}-typed")));
    for registration in registrations {
        legacy_builder = legacy_builder.register_subscriber(registration.clone());
        typed_builder = typed_builder.register_subscriber(registration);
    }
    let legacy = legacy_builder.build().expect("legacy runtime");
    let typed = typed_builder.build_typed().expect("typed runtime");
    (legacy, typed)
}

fn paired_projection_runtimes(
    name: &str,
    subscriber: SubscriberRegistration<ObservationPayload>,
    projection: ProjectionRegistration<ObservationPayload>,
) -> (Observability, Observability) {
    let legacy = Observability::builder(config(&format!("{name}-legacy")))
        .register_subscriber(subscriber.clone())
        .register_projection(projection.clone())
        .build()
        .expect("legacy runtime");
    let typed = Observability::builder(config(&format!("{name}-typed")))
        .register_subscriber(subscriber)
        .register_projection(projection)
        .build_typed()
        .expect("typed runtime");
    (legacy, typed)
}

#[test]
fn paired_filters_ordering_and_invocation_counts_match() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let registrations = vec![
        SubscriberRegistration::new(legacy_subscriber(Arc::new(OrderedSubscriber {
            id: "first",
            calls: order.clone(),
        })))
        .with_filter(Arc::new(AllowFilter)),
        SubscriberRegistration::new(legacy_subscriber(Arc::new(OrderedSubscriber {
            id: "second",
            calls: order.clone(),
        })))
        .with_filter(Arc::new(AllowFilter)),
    ];
    let (legacy, typed) = paired_subscriber_runtimes("filter-order", registrations);

    assert!(matches!(
        legacy.emit(observation_with(false)),
        Err(ObservationError::RoutingFailure(_))
    ));
    assert!(matches!(
        typed.emit(observation_with(false)),
        Err(ObservationError::RoutingFailure(_))
    ));
    legacy.emit(observation_with(true)).expect("legacy emit");
    typed.emit(observation_with(true)).expect("typed emit");

    assert_eq!(
        *order.lock().expect("calls poisoned"),
        vec!["first", "second", "first", "second"]
    );
    for runtime in [&legacy, &typed] {
        let health = runtime.health();
        assert_eq!(health.dropped_observations_total, 1);
        assert_eq!(health.subscriber_failures_total, 0);
    }
}

#[test]
fn paired_no_matching_route_and_failure_outcomes_are_classified() {
    let calls = Arc::new(AtomicUsize::new(0));
    let registration =
        SubscriberRegistration::new(legacy_subscriber(Arc::new(CountingSubscriber {
            calls: calls.clone(),
        })));
    let (legacy, typed) = paired_subscriber_runtimes("no-match", vec![registration]);

    assert!(matches!(
        legacy.emit(other_observation()),
        Err(ObservationError::RoutingFailure(_))
    ));
    assert!(matches!(
        typed.emit(other_observation()),
        Err(ObservationError::RoutingFailure(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for runtime in [&legacy, &typed] {
        assert_eq!(runtime.health().dropped_observations_total, 1);
    }

    let failed_calls = Arc::new(AtomicUsize::new(0));
    let delivered_calls = Arc::new(AtomicUsize::new(0));
    let mixed = vec![
        SubscriberRegistration::new(legacy_subscriber(Arc::new(FailingSubscriber {
            code: ErrorCode::new_static("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"),
            calls: failed_calls.clone(),
        }))),
        SubscriberRegistration::new(legacy_subscriber(Arc::new(CountingSubscriber {
            calls: delivered_calls.clone(),
        }))),
    ];
    let (legacy, typed) = paired_subscriber_runtimes("mixed-failure", mixed);
    legacy.emit(observation()).expect("legacy success route");
    typed.emit(observation()).expect("typed success route");
    assert_eq!(failed_calls.load(Ordering::SeqCst), 2);
    assert_eq!(delivered_calls.load(Ordering::SeqCst), 2);
    for runtime in [&legacy, &typed] {
        let health = runtime.health();
        assert_eq!(health.subscriber_failures_total, 1);
        assert_eq!(health.dropped_observations_total, 0);
    }

    let all_failed_calls = Arc::new(AtomicUsize::new(0));
    let all_failed = SubscriberRegistration::new(legacy_subscriber(Arc::new(FailingSubscriber {
        code: ErrorCode::new_static("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"),
        calls: all_failed_calls.clone(),
    })));
    let (legacy, typed) = paired_subscriber_runtimes("all-failure", vec![all_failed]);
    assert!(matches!(
        legacy.emit(observation()),
        Err(ObservationError::RoutingFailure(_))
    ));
    assert!(matches!(
        typed.emit(observation()),
        Err(ObservationError::RoutingFailure(_))
    ));
    assert_eq!(all_failed_calls.load(Ordering::SeqCst), 2);
    for runtime in [&legacy, &typed] {
        let health = runtime.health();
        assert_eq!(health.subscriber_failures_total, 1);
        assert_eq!(health.dropped_observations_total, 1);
        assert_eq!(
            health.last_error.expect("routing diagnostic").code,
            Some(ErrorCode::new_static(
                "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
            ))
        );
    }
}

#[test]
fn observation_adapter_preserves_custom_and_cross_family_context() {
    for code in [
        ErrorCode::new_static("SC_CUSTOM_OBSERVATION_FAILURE"),
        sc_observability::error_codes::LOGGER_FLUSH_FAILED,
    ] {
        let legacy = legacy_subscriber(Arc::new(FailingSubscriber {
            code: code.clone(),
            calls: Arc::new(AtomicUsize::new(0)),
        }));
        let legacy_error =
            sc_observability_types::ObservationSubscriber::observe(legacy.as_ref(), &observation())
                .expect_err("legacy adapter should preserve failure");
        assert_eq!(
            legacy_error.kind(),
            sc_observability_types::typed::SubscriberFailureKind::Unclassified
        );
        assert_eq!(legacy_error.context().diagnostic().code, code);

        let typed = typed_subscriber(legacy);
        let typed_error = typed
            .observe(&observation())
            .expect_err("typed adapter should preserve failure");
        assert_eq!(
            typed_error.kind(),
            sc_observability_types::typed::SubscriberFailureKind::Unclassified
        );
        assert_eq!(typed_error.context().diagnostic().code, code);
        assert_eq!(
            std::error::Error::source(typed_error.context())
                .expect("source context")
                .to_string(),
            "subscriber fixture source"
        );
    }
}

// Each fixture owns one context: consuming it twice fails, and pointer checks
// prove the adapters moved the original source/backtrace-bearing allocation.
struct FailingProjector {
    context: Mutex<Option<Box<sc_observability_types::ErrorContext>>>,
    calls: Arc<AtomicUsize>,
}

impl FailingProjector {
    fn fail(&self) -> Box<sc_observability_types::ErrorContext> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.context
            .lock()
            .expect("context lock")
            .take()
            .expect("one invocation")
    }
}

macro_rules! projector_context_case {
    ($test:ident, $legacy_trait:ident, $typed_trait:ident, $method:ident, $output:ty,
     $legacy_adapter:ident, $typed_adapter:ident) => {
        impl sc_observability_types::$legacy_trait<ObservationPayload> for FailingProjector {
            fn $method(
                &self,
                _: &Observation<ObservationPayload>,
            ) -> Result<Vec<$output>, sc_observability_types::ProjectionError> {
                Err(sc_observability_types::ProjectionError(self.fail()))
            }
        }
        impl sc_observability_types::typed::$typed_trait<ObservationPayload> for FailingProjector {
            fn $method(
                &self,
                _: &Observation<ObservationPayload>,
            ) -> Result<Vec<$output>, ProjectionFailure> {
                Err(ProjectionFailure::from_context(self.fail()))
            }
        }
        #[test]
        fn $test() {
            for code in [
                ErrorCode::new_static("SC_CUSTOM_PROJECTION_FAILURE"),
                sc_observability::error_codes::LOGGER_FLUSH_FAILED,
            ] {
                for typed_to_legacy in [false, true] {
                    let context = Box::new(
                        sc_observability_types::ErrorContext::new(
                            code.clone(),
                            "projector fixture failed",
                            Remediation::not_recoverable("inspect projector fixture"),
                        )
                        .cause("fixture cause")
                        .docs("https://example.test/projector")
                        .detail("family", json!(stringify!($method)))
                        .source(Box::new(std::io::Error::other("projector native source"))),
                    );
                    let original_context = std::ptr::from_ref(context.as_ref());
                    let diagnostic = context.diagnostic().clone();
                    let original_source = std::ptr::from_ref(
                        std::error::Error::source(context.as_ref())
                            .expect("native source")
                            .downcast_ref::<std::io::Error>()
                            .expect("io source"),
                    );
                    let calls = Arc::new(AtomicUsize::new(0));
                    let fixture = Arc::new(FailingProjector {
                        context: Mutex::new(Some(context)),
                        calls: calls.clone(),
                    });
                    let failure = if typed_to_legacy {
                        let adapter = sc_observability_types::typed::$legacy_adapter(fixture);
                        let error = adapter.$method(&observation()).expect_err("legacy failure");
                        assert_eq!(
                            error.kind(),
                            sc_observability_types::typed::ProjectionFailureKind::Unclassified
                        );
                        ProjectionFailure::from(error)
                    } else {
                        let adapter = sc_observability_types::typed::$typed_adapter(fixture);
                        adapter.$method(&observation()).expect_err("typed failure")
                    };
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        failure.kind(),
                        sc_observability_types::typed::ProjectionFailureKind::Unclassified
                    );
                    assert_eq!(failure.diagnostic().code, code);
                    assert_eq!(failure.diagnostic(), &diagnostic);
                    assert_eq!(std::ptr::from_ref(failure.context()), original_context);
                    let source = std::error::Error::source(failure.context())
                        .expect("retained native source")
                        .downcast_ref::<std::io::Error>()
                        .expect("retained native type");
                    assert_eq!(std::ptr::from_ref(source), original_source);
                    assert_eq!(source.to_string(), "projector native source");
                }
            }
        }
    };
}

projector_context_case!(
    log_adapters_retain_custom_and_wrong_family_context,
    LogProjector,
    TypedLogProjector,
    project_logs,
    LogEvent,
    legacy_log_projector,
    typed_log_projector
);
projector_context_case!(
    span_adapters_retain_custom_and_wrong_family_context,
    SpanProjector,
    TypedSpanProjector,
    project_spans,
    SpanSignal,
    legacy_span_projector,
    typed_span_projector
);
projector_context_case!(
    metric_adapters_retain_custom_and_wrong_family_context,
    MetricProjector,
    TypedMetricProjector,
    project_metrics,
    MetricRecord,
    legacy_metric_projector,
    typed_metric_projector
);

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
fn paired_projection_routes_preserve_output_family_invocation_counts() {
    let subscriber_calls = Arc::new(AtomicUsize::new(0));
    let log_calls = Arc::new(AtomicUsize::new(0));
    let span_calls = Arc::new(AtomicUsize::new(0));
    let metric_calls = Arc::new(AtomicUsize::new(0));
    let subscriber = SubscriberRegistration::new(legacy_subscriber(Arc::new(CountingSubscriber {
        calls: subscriber_calls.clone(),
    })));
    let projection = ProjectionRegistration::new()
        .with_log_projector(legacy_log_projector(Arc::new(CountingLogProjector {
            calls: log_calls.clone(),
        })))
        .with_span_projector(legacy_span_projector(Arc::new(CountingSpanProjector {
            calls: span_calls.clone(),
        })))
        .with_metric_projector(legacy_metric_projector(Arc::new(CountingMetricProjector {
            calls: metric_calls.clone(),
        })));
    let (legacy, typed) = paired_projection_runtimes("projection-families", subscriber, projection);

    legacy.emit(observation()).expect("legacy emit");
    legacy.flush().expect("legacy flush");
    typed.emit(observation()).expect("typed emit");
    typed.flush_typed().expect("typed flush");

    assert_eq!(subscriber_calls.load(Ordering::SeqCst), 2);
    assert_eq!(log_calls.load(Ordering::SeqCst), 2);
    assert_eq!(span_calls.load(Ordering::SeqCst), 2);
    assert_eq!(metric_calls.load(Ordering::SeqCst), 2);
    legacy.shutdown().expect("legacy shutdown");
    typed.shutdown_typed().expect("typed shutdown");
}

#[test]
fn invalid_deserialized_names_fail_paired_facade_checks() {
    assert!(ToolName::new("").is_err());
    assert!(ServiceName::new("").is_err());

    let invalid_tool: ToolName = serde_json::from_value(json!("bad/name"))
        .expect("derived deserialization intentionally admits the fixture");
    let legacy_default = sc_observe::ObservabilityConfig::default_for(
        invalid_tool.clone(),
        temp_path("invalid-default-legacy"),
    )
    .expect_err("legacy default must validate the derived env prefix");
    let typed_default = sc_observe::ObservabilityConfig::default_for_typed(
        invalid_tool.clone(),
        temp_path("invalid-default-typed"),
    )
    .expect_err("typed default must validate the derived env prefix");
    assert_eq!(
        legacy_default.kind(),
        InitFailureKind::ObservationInitialization
    );
    assert_eq!(
        typed_default.kind(),
        InitFailureKind::ObservationInitialization
    );
    assert_eq!(
        legacy_default.diagnostic().code,
        typed_default.diagnostic().code
    );
    assert_eq!(
        legacy_default.diagnostic().code,
        sc_observe::error_codes::OBSERVABILITY_INIT_FAILED
    );
    for source in [
        std::error::Error::source(&legacy_default).expect("legacy context source"),
        std::error::Error::source(typed_default.context()).expect("typed context source"),
    ] {
        assert!(source.to_string().contains("env prefix"));
    }

    let mut legacy_config = config("invalid-service-legacy");
    legacy_config.tool_name = invalid_tool.clone();
    let mut typed_config = config("invalid-service-typed");
    typed_config.tool_name = invalid_tool;
    let legacy_service = legacy_config
        .service_name()
        .expect_err("legacy service derivation must validate the tool name");
    let typed_service = typed_config
        .service_name_typed()
        .expect_err("typed service derivation must validate the tool name");
    assert_eq!(
        legacy_service.kind(),
        InitFailureKind::ObservationInitialization
    );
    assert_eq!(
        typed_service.kind(),
        InitFailureKind::ObservationInitialization
    );
    assert_eq!(
        legacy_service.diagnostic().code,
        typed_service.diagnostic().code
    );
    for source in [
        std::error::Error::source(&legacy_service).expect("legacy context source"),
        std::error::Error::source(typed_service.context()).expect("typed context source"),
    ] {
        assert!(source.to_string().contains("identifier"));
    }
}

#[test]
fn typed_and_legacy_construction_failures_classify_consistently() {
    let legacy_config = config("legacy-new-empty");
    let typed_config = config("typed-new-empty");
    let Err(legacy_new) = Observability::new(legacy_config) else {
        panic!("legacy new without routes must fail");
    };
    let Err(new_empty) = Observability::new_typed(typed_config) else {
        panic!("new_typed without routes must fail");
    };
    assert_eq!(
        legacy_new.kind(),
        InitFailureKind::ObservationInitialization
    );
    assert_eq!(new_empty.kind(), InitFailureKind::ObservationInitialization);
    assert_eq!(legacy_new.diagnostic().code, new_empty.diagnostic().code);

    let Err(empty) = Observability::builder(
        sc_observe::ObservabilityConfig::default_for_typed(
            ToolName::new("typed-observe").expect("valid tool"),
            temp_path("empty"),
        )
        .expect("typed config"),
    )
    .build_typed() else {
        panic!("empty routes must fail");
    };
    assert_eq!(empty.kind(), InitFailureKind::ObservationInitialization);

    let mut legacy_config = sc_observe::ObservabilityConfig::default_for(
        ToolName::new("typed-observe").expect("valid tool"),
        temp_path("legacy-logger-failure"),
    )
    .expect("legacy config");
    legacy_config.queue_capacity = 0;
    let mut typed_config = sc_observe::ObservabilityConfig::default_for_typed(
        ToolName::new("typed-observe").expect("valid tool"),
        temp_path("typed-logger-failure"),
    )
    .expect("typed config");
    typed_config.queue_capacity = 0;
    let registration =
        SubscriberRegistration::new(legacy_subscriber(Arc::new(CountingSubscriber {
            calls: Arc::new(AtomicUsize::new(0)),
        })));
    let Err(legacy_logger_failure) = Observability::builder(legacy_config)
        .register_subscriber(registration.clone())
        .build()
    else {
        panic!("legacy zero queue capacity must fail");
    };
    let Err(typed_logger_failure) = Observability::builder(typed_config)
        .register_subscriber(registration)
        .build_typed()
    else {
        panic!("typed zero queue capacity must fail");
    };
    assert_eq!(
        legacy_logger_failure.kind(),
        InitFailureKind::LoggerInitialization
    );
    assert_eq!(
        typed_logger_failure.kind(),
        InitFailureKind::LoggerInitialization
    );
    assert_eq!(
        legacy_logger_failure.diagnostic().code,
        typed_logger_failure.diagnostic().code
    );
    assert!(
        legacy_logger_failure
            .diagnostic()
            .message
            .contains("queue capacity")
    );
    assert!(
        typed_logger_failure
            .diagnostic()
            .message
            .contains("queue capacity")
    );
}
