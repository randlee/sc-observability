//! Typed observation routing layered on top of `sc-observability`.
//!
//! This crate owns construction-time subscriber/projector registration,
//! per-type routing, and top-level observability health aggregation while
//! remaining independent of OTLP transport details.

pub mod constants;
pub mod error_codes;

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability::{Logger, LoggerConfig, RotationPolicy};
use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, EnvPrefix, ErrorContext, FlushError, InitError, Observable,
    Observation, ProjectionRegistration, Remediation, ServiceName, ShutdownError, SubscriberError,
    SubscriberRegistration, ToolName,
};
pub use sc_observability_types::{
    ObservabilityHealthReport, ObservationError, ObservationHealthState,
};

/// Top-level configuration for the observation routing runtime.
///
/// Routing owns tool identity, log-root selection, env-prefix derivation, and
/// queue capacity. Logging-specific level, retention, and redaction behavior
/// stay owned by `LoggerConfig` in `sc-observability` and are intentionally not
/// overridable at the `ObservabilityConfig` layer.
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub tool_name: ToolName,
    pub log_root: PathBuf,
    pub env_prefix: EnvPrefix,
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
}

impl ObservabilityConfig {
    /// Builds the documented v1 defaults from a tool name and log root.
    pub fn default_for(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitError> {
        let env_prefix = EnvPrefix::new(
            tool_name
                .as_str()
                .replace(['-', '.'], "_")
                .to_ascii_uppercase(),
        )
        .map_err(|err| {
            InitError(Box::new(
                ErrorContext::new(
                    error_codes::OBSERVABILITY_INIT_FAILED,
                    "failed to derive env prefix",
                    Remediation::not_recoverable("use an explicit valid env prefix"),
                )
                .cause(err.to_string()),
            ))
        })?;
        Ok(Self {
            tool_name,
            log_root,
            env_prefix,
            queue_capacity: constants::DEFAULT_OBSERVATION_QUEUE_CAPACITY,
            rotation: RotationPolicy::default(),
        })
    }

    /// Derives the logging/telemetry service name from the configured tool.
    pub fn service_name(&self) -> Result<ServiceName, InitError> {
        ServiceName::new(self.tool_name.as_str()).map_err(|err| {
            InitError(Box::new(
                ErrorContext::new(
                    error_codes::OBSERVABILITY_INIT_FAILED,
                    "failed to derive service name",
                    Remediation::not_recoverable("use a valid tool name"),
                )
                .cause(err.to_string()),
            ))
        })
    }

    fn logger_config(&self) -> Result<LoggerConfig, InitError> {
        let mut config = LoggerConfig::default_for(self.service_name()?, self.log_root.clone());
        config.queue_capacity = self.queue_capacity;
        config.rotation = self.rotation;
        Ok(config)
    }
}

/// Builder for construction-time subscriber and projector registration.
pub struct ObservabilityBuilder {
    config: ObservabilityConfig,
    subscribers: Vec<ErasedSubscriberRegistration>,
    projections: Vec<ErasedProjectionRegistration>,
}

/// Producer-facing routing runtime for typed observations.
pub struct Observability {
    logger: Logger,
    shutdown: AtomicBool,
    subscriber_registrations: Vec<ErasedSubscriberRegistration>,
    projection_registrations: Vec<ErasedProjectionRegistration>,
    runtime: RuntimeState,
}

#[derive(Default)]
struct RuntimeState {
    dropped_observations_total: AtomicU64,
    subscriber_failures_total: AtomicU64,
    projection_failures_total: AtomicU64,
    last_error: Mutex<Option<DiagnosticSummary>>,
}

struct ErasedSubscriberRegistration {
    type_id: TypeId,
    dispatch: Arc<SubscriberDispatchFn>,
}

struct ErasedProjectionRegistration {
    type_id: TypeId,
    dispatch: Arc<ProjectionDispatchFn>,
}

type SubscriberDispatchFn =
    dyn Fn(&dyn Any) -> Result<DispatchMatch, SubscriberError> + Send + Sync + 'static;
type ProjectionDispatchFn =
    dyn Fn(&dyn Any, &Logger) -> ProjectionDispatchResult + Send + Sync + 'static;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchMatch {
    Skipped,
    Delivered,
}

#[derive(Debug, Default, Clone, PartialEq)]
struct ProjectionDispatchResult {
    matched: bool,
    failure_count: u64,
    last_error: Option<DiagnosticSummary>,
}

impl Observability {
    /// Builds a runtime using the documented default logger integration.
    pub fn new(config: ObservabilityConfig) -> Result<Self, InitError> {
        Self::builder(config).build()
    }

    /// Starts a construction-time builder for subscribers and projections.
    pub fn builder(config: ObservabilityConfig) -> ObservabilityBuilder {
        ObservabilityBuilder {
            config,
            subscribers: Vec::new(),
            projections: Vec::new(),
        }
    }

    /// Routes one typed observation through the registered subscribers and projections.
    pub fn emit<T>(&self, observation: Observation<T>) -> Result<(), ObservationError>
    where
        T: Observable,
    {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(ObservationError::Shutdown);
        }

        let observation_any = &observation as &dyn Any;
        let type_id = TypeId::of::<T>();
        let mut matched = false;

        for registration in self
            .subscriber_registrations
            .iter()
            .filter(|entry| entry.type_id == type_id)
        {
            match (registration.dispatch)(observation_any) {
                Ok(DispatchMatch::Delivered) => matched = true,
                Ok(DispatchMatch::Skipped) => {}
                Err(err) => {
                    self.runtime
                        .subscriber_failures_total
                        .fetch_add(1, Ordering::SeqCst);
                    self.record_last_error(DiagnosticSummary::from(err.diagnostic()));
                }
            }
        }

        for registration in self
            .projection_registrations
            .iter()
            .filter(|entry| entry.type_id == type_id)
        {
            let result = (registration.dispatch)(observation_any, &self.logger);
            matched |= result.matched;
            if result.failure_count > 0 {
                self.runtime
                    .projection_failures_total
                    .fetch_add(result.failure_count, Ordering::SeqCst);
                if let Some(summary) = result.last_error {
                    self.record_last_error(summary);
                }
            }
        }

        if !matched {
            self.runtime
                .dropped_observations_total
                .fetch_add(1, Ordering::SeqCst);
            // Failing subscribers do not count as active paths; RoutingFailure
            // is correct per OBS-009/OBS-010.
            let context = ErrorContext::new(
                error_codes::OBSERVATION_ROUTING_FAILURE,
                "no eligible subscriber or projector path matched the observation",
                Remediation::recoverable(
                    "register at least one matching subscriber or projector",
                    ["ensure filters allow the emitted observation type"],
                ),
            );
            self.record_last_error(DiagnosticSummary::from(context.diagnostic()));
            return Err(ObservationError::RoutingFailure(Box::new(context)));
        }

        Ok(())
    }

    /// Flushes the attached logger. Routing itself does not keep an async queue in v1.
    pub fn flush(&self) -> Result<(), FlushError> {
        self.logger.flush()
    }

    /// Shuts down the routing runtime. Repeated calls are idempotent.
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        self.logger.shutdown()
    }

    /// Returns the aggregate runtime health view.
    pub fn health(&self) -> ObservabilityHealthReport {
        let logging = self.logger.health();
        let subscriber_failures = self
            .runtime
            .subscriber_failures_total
            .load(Ordering::SeqCst);
        let projection_failures = self
            .runtime
            .projection_failures_total
            .load(Ordering::SeqCst);
        let dropped = self
            .runtime
            .dropped_observations_total
            .load(Ordering::SeqCst);

        let state = if self.shutdown.load(Ordering::SeqCst) {
            ObservationHealthState::Unavailable
        } else if dropped > 0
            || subscriber_failures > 0
            || projection_failures > 0
            || logging.state != sc_observability_types::LoggingHealthState::Healthy
        {
            ObservationHealthState::Degraded
        } else {
            ObservationHealthState::Healthy
        };

        ObservabilityHealthReport {
            state,
            dropped_observations_total: dropped,
            subscriber_failures_total: subscriber_failures,
            projection_failures_total: projection_failures,
            logging: Some(logging),
            telemetry: None,
            last_error: self
                .runtime
                .last_error
                .lock()
                .expect("observability last_error poisoned")
                .clone(),
        }
    }

    fn record_last_error(&self, summary: DiagnosticSummary) {
        *self
            .runtime
            .last_error
            .lock()
            .expect("observability last_error poisoned") = Some(summary);
    }
}

impl ObservabilityBuilder {
    /// Registers one typed observation subscriber at construction time.
    pub fn register_subscriber<T>(mut self, registration: SubscriberRegistration<T>) -> Self
    where
        T: Observable,
    {
        let subscriber = registration.subscriber;
        let filter = registration.filter;
        self.subscribers.push(ErasedSubscriberRegistration {
            type_id: TypeId::of::<T>(),
            dispatch: Arc::new(move |observation_any| {
                let observation = observation_any
                    .downcast_ref::<Observation<T>>()
                    .expect("type-erased routing matched wrong observation type");

                if filter
                    .as_ref()
                    .is_some_and(|filter| !filter.accepts(observation))
                {
                    return Ok(DispatchMatch::Skipped);
                }

                subscriber.handle(observation)?;
                Ok(DispatchMatch::Delivered)
            }),
        });
        self
    }

    /// Registers one typed observation projection set at construction time.
    pub fn register_projection<T>(mut self, registration: ProjectionRegistration<T>) -> Self
    where
        T: Observable,
    {
        let filter = registration.filter;
        let log_projector = registration.log_projector;
        let span_projector = registration.span_projector;
        let metric_projector = registration.metric_projector;

        self.projections.push(ErasedProjectionRegistration {
            type_id: TypeId::of::<T>(),
            dispatch: Arc::new(move |observation_any, logger| {
                let observation = observation_any
                    .downcast_ref::<Observation<T>>()
                    .expect("type-erased routing matched wrong observation type");

                if filter
                    .as_ref()
                    .is_some_and(|filter| !filter.accepts(observation))
                {
                    return ProjectionDispatchResult::default();
                }

                let mut result = ProjectionDispatchResult::default();
                let mut record_failure = |summary: DiagnosticSummary| {
                    result.failure_count += 1;
                    result.last_error = Some(summary);
                };

                if let Some(projector) = &log_projector {
                    match projector.project_logs(observation) {
                        Ok(events) => {
                            result.matched = true;
                            for event in events {
                                if let Err(err) = logger.emit(event) {
                                    record_failure(DiagnosticSummary::from(err.diagnostic()));
                                }
                            }
                        }
                        Err(err) => record_failure(DiagnosticSummary::from(err.diagnostic())),
                    }
                }

                if let Some(projector) = &span_projector {
                    match projector.project_spans(observation) {
                        Ok(_) => result.matched = true,
                        Err(err) => record_failure(DiagnosticSummary::from(err.diagnostic())),
                    }
                }

                if let Some(projector) = &metric_projector {
                    match projector.project_metrics(observation) {
                        Ok(_) => result.matched = true,
                        Err(err) => record_failure(DiagnosticSummary::from(err.diagnostic())),
                    }
                }

                result
            }),
        });
        self
    }

    /// Finalizes registration and constructs the routing runtime.
    pub fn build(self) -> Result<Observability, InitError> {
        let logger = Logger::new(self.config.logger_config()?)?;
        Ok(Observability {
            logger,
            shutdown: AtomicBool::new(false),
            subscriber_registrations: self.subscribers,
            projection_registrations: self.projections,
            runtime: RuntimeState::default(),
        })
    }
}

mod sealed_emitters {
    pub trait Sealed {}
}

/// ObservationEmitter<T> is intentionally per-type -- callers hold one handle
/// per observation type. A single type-erased emitter for heterogeneous events
/// is not supported by design.
#[expect(
    dead_code,
    reason = "crate-local observation emitter trait is intentionally retained for injection"
)]
pub(crate) trait ObservationEmitter<T>: sealed_emitters::Sealed + Send + Sync
where
    T: Observable,
{
    fn emit(&self, observation: Observation<T>) -> Result<(), ObservationError>;
}

impl sealed_emitters::Sealed for Observability {}

impl<T> ObservationEmitter<T> for Observability
where
    T: Observable,
{
    fn emit(&self, observation: Observation<T>) -> Result<(), ObservationError> {
        Observability::emit(self, observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricKind, MetricName, MetricRecord,
        ObservationFilter, ObservationSubscriber, ProcessIdentity, ProjectionError, SpanId,
        SpanProjector, SpanRecord, SpanSignal, SpanStarted, SubscriberError, TargetCategory,
        Timestamp, TraceContext, TraceId,
    };

    #[derive(Debug, Clone)]
    struct AgentEvent {
        kind: &'static str,
        allow: bool,
    }

    struct RecordingSubscriber {
        id: &'static str,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl ObservationSubscriber<AgentEvent> for RecordingSubscriber {
        fn handle(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
            self.calls.lock().expect("calls poisoned").push(self.id);
            Ok(())
        }
    }

    struct AllowFlagFilter;

    impl ObservationFilter<AgentEvent> for AllowFlagFilter {
        fn accepts(&self, observation: &Observation<AgentEvent>) -> bool {
            observation.payload.allow
        }
    }

    struct FailingSubscriber;

    impl ObservationSubscriber<AgentEvent> for FailingSubscriber {
        fn handle(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
            Err(SubscriberError(Box::new(ErrorContext::new(
                error_codes::OBSERVATION_ROUTING_FAILURE,
                "subscriber failed",
                Remediation::not_recoverable("test subscriber intentionally fails"),
            ))))
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
        ) -> Result<Vec<LogEvent>, ProjectionError> {
            self.calls.lock().expect("calls poisoned").push(self.id);
            Ok(vec![log_event(
                observation.service.clone(),
                observation.payload.kind,
            )])
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
                trace_context(),
                Default::default(),
            ))])
        }
    }

    struct RecordingMetricProjector {
        count: Arc<AtomicU64>,
    }

    impl sc_observability_types::MetricProjector<AgentEvent> for RecordingMetricProjector {
        fn project_metrics(
            &self,
            observation: &Observation<AgentEvent>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(vec![MetricRecord {
                timestamp: Timestamp::UNIX_EPOCH,
                service: observation.service.clone(),
                name: MetricName::new("obs.events_total").expect("valid metric"),
                kind: MetricKind::Counter,
                value: 1.0,
                unit: Some("1".to_string()),
                attributes: Default::default(),
            }])
        }
    }

    struct FailingProjector;

    impl sc_observability_types::LogProjector<AgentEvent> for FailingProjector {
        fn project_logs(
            &self,
            _observation: &Observation<AgentEvent>,
        ) -> Result<Vec<LogEvent>, ProjectionError> {
            Err(ProjectionError(Box::new(ErrorContext::new(
                error_codes::OBSERVATION_ROUTING_FAILURE,
                "projector failed",
                Remediation::not_recoverable("test projector intentionally fails"),
            ))))
        }
    }

    fn tool_name() -> ToolName {
        ToolName::new("obs-app").expect("valid tool name")
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sc-observe-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: None,
        }
    }

    fn observation(allow: bool) -> Observation<AgentEvent> {
        let mut observation = Observation::new(
            ServiceName::new("obs-app").expect("valid service"),
            AgentEvent {
                kind: "received",
                allow,
            },
        );
        observation.identity = ProcessIdentity::default();
        observation
    }

    fn log_event(service: ServiceName, message: &str) -> LogEvent {
        LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service,
            target: TargetCategory::new("observe.routing").expect("valid target"),
            action: ActionName::new("observation.received").expect("valid action"),
            message: Some(message.to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace_context()),
            request_id: None,
            correlation_id: None,
            outcome: Some("ok".to_string()),
            diagnostic: Some(Diagnostic {
                code: ErrorCode::new_static("SC_TEST"),
                message: "projected".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect log output"]),
                docs: None,
                details: Default::default(),
            }),
            state_transition: None,
            fields: Default::default(),
        }
    }

    #[test]
    fn registration_order_routing_is_deterministic() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let root = temp_path("order");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(RecordingSubscriber {
                    id: "first",
                    calls: calls.clone(),
                }),
                filter: None,
            })
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(RecordingSubscriber {
                    id: "second",
                    calls: calls.clone(),
                }),
                filter: None,
            })
            .build()
            .expect("runtime");

        runtime.emit(observation(true)).expect("emit");

        assert_eq!(
            *calls.lock().expect("calls poisoned"),
            vec!["first", "second"]
        );
    }

    #[test]
    fn filter_acceptance_and_rejection_are_respected() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let root = temp_path("filter");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(RecordingSubscriber {
                    id: "allowed",
                    calls: calls.clone(),
                }),
                filter: Some(Arc::new(AllowFlagFilter)),
            })
            .build()
            .expect("runtime");

        assert!(runtime.emit(observation(false)).is_err());
        runtime.emit(observation(true)).expect("emit");

        assert_eq!(*calls.lock().expect("calls poisoned"), vec!["allowed"]);
    }

    #[test]
    fn subscriber_failures_are_isolated() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let root = temp_path("subscriber-failure");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(FailingSubscriber),
                filter: None,
            })
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(RecordingSubscriber {
                    id: "still-runs",
                    calls: calls.clone(),
                }),
                filter: None,
            })
            .build()
            .expect("runtime");

        runtime.emit(observation(true)).expect("emit");

        let health = runtime.health();
        assert_eq!(health.subscriber_failures_total, 1);
        assert_eq!(*calls.lock().expect("calls poisoned"), vec!["still-runs"]);
        assert_eq!(health.state, ObservationHealthState::Degraded);
    }

    #[test]
    fn projector_failures_are_isolated() {
        let log_calls = Arc::new(Mutex::new(Vec::new()));
        let span_count = Arc::new(AtomicU64::new(0));
        let metric_count = Arc::new(AtomicU64::new(0));
        let root = temp_path("projector-failure");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_projection(ProjectionRegistration {
                log_projector: Some(Arc::new(FailingProjector)),
                span_projector: Some(Arc::new(RecordingSpanProjector {
                    count: span_count.clone(),
                })),
                metric_projector: Some(Arc::new(RecordingMetricProjector {
                    count: metric_count.clone(),
                })),
                filter: None,
            })
            .register_projection(ProjectionRegistration {
                log_projector: Some(Arc::new(RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                })),
                span_projector: None,
                metric_projector: None,
                filter: None,
            })
            .build()
            .expect("runtime");

        runtime.emit(observation(true)).expect("emit");

        let health = runtime.health();
        assert_eq!(health.projection_failures_total, 1);
        assert_eq!(span_count.load(Ordering::SeqCst), 1);
        assert_eq!(metric_count.load(Ordering::SeqCst), 1);
        assert_eq!(*log_calls.lock().expect("calls poisoned"), vec!["log"]);
    }

    #[test]
    fn routing_failure_occurs_when_no_eligible_path_remains() {
        let root = temp_path("routing-failure");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration {
                subscriber: Arc::new(RecordingSubscriber {
                    id: "filtered",
                    calls: Arc::new(Mutex::new(Vec::new())),
                }),
                filter: Some(Arc::new(AllowFlagFilter)),
            })
            .build()
            .expect("runtime");

        let result = runtime.emit(observation(false));

        assert!(matches!(result, Err(ObservationError::RoutingFailure(_))));
        assert_eq!(runtime.health().dropped_observations_total, 1);
    }

    #[test]
    fn routing_failure_occurs_when_all_projectors_fail() {
        let root = temp_path("projector-routing-failure");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_projection(ProjectionRegistration {
                log_projector: Some(Arc::new(FailingProjector)),
                span_projector: None,
                metric_projector: None,
                filter: None,
            })
            .build()
            .expect("runtime");

        let result = runtime.emit(observation(true));

        assert!(matches!(result, Err(ObservationError::RoutingFailure(_))));
        let health = runtime.health();
        assert_eq!(health.dropped_observations_total, 1);
        assert_eq!(health.projection_failures_total, 1);
    }

    #[test]
    fn post_shutdown_emission_returns_shutdown_error() {
        let root = temp_path("shutdown");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config).build().expect("runtime");

        runtime.shutdown().expect("shutdown");

        assert!(matches!(
            runtime.emit(observation(true)),
            Err(ObservationError::Shutdown)
        ));
    }

    #[test]
    fn top_level_health_aggregates_logging_and_routing_state() {
        let root = temp_path("health");
        let config = ObservabilityConfig::default_for(tool_name(), root.clone()).expect("config");
        let runtime = Observability::builder(config)
            .register_projection(ProjectionRegistration {
                log_projector: Some(Arc::new(FailingProjector)),
                span_projector: None,
                metric_projector: None,
                filter: None,
            })
            .build()
            .expect("runtime");

        let _ = runtime.emit(observation(true));
        let health = runtime.health();

        assert_eq!(health.state, ObservationHealthState::Degraded);
        assert_eq!(health.projection_failures_total, 1);
        assert!(health.logging.is_some());
        assert!(health.last_error.is_some());
        assert!(health.telemetry.is_none());
    }

    #[test]
    fn queue_capacity_override_propagates_to_logger_config() {
        let root = temp_path("queue-capacity");
        let mut config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        config.queue_capacity = 2048;

        let logger_config = config.logger_config().expect("logger config");

        assert_eq!(logger_config.queue_capacity, 2048);
    }
}
