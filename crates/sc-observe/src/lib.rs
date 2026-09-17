//! Typed observation routing layered on top of `sc-observability`.
//!
//! This crate owns construction-time subscriber/projector registration,
//! per-type routing, and top-level observability health aggregation while
//! remaining independent of OTLP transport details.
#![expect(
    clippy::missing_errors_doc,
    reason = "public routing-facade error behavior is documented centrally in workspace docs, and repeating boilerplate on every wrapper method adds low signal"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "builder and accessor methods intentionally avoid pervasive must_use boilerplate across the facade"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "builder-style chaining is explicit from the signatures and intentionally lightweight"
)]
#![expect(
    clippy::struct_field_names,
    reason = "the health-provider field uses the full domain term for clarity across builder/runtime structs"
)]
#![expect(
    clippy::needless_pass_by_value,
    reason = "Observation is the producer-facing owned emission contract, so emit intentionally takes ownership"
)]

pub mod constants;
pub mod error_codes;

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[allow(
    deprecated,
    reason = "the facade retains legacy error names in its public compatibility signatures"
)]
use sc_observability::{LogError, Logger, LoggerConfig, RetainedLogPolicy, Running, Stopped};
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
#[allow(
    deprecated,
    reason = "the facade retains legacy error names in its published compatibility signatures"
)]
use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, EnvPrefix, ErrorContext, FlushError, InitError,
    ObservabilityHealthProvider, Observable, Observation, ProjectionRegistration, Remediation,
    ServiceName, ShutdownError, SubscriberError, SubscriberRegistration, TelemetryHealthState,
    ToolName,
};
#[doc(inline)]
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
    /// Stable tool name used to derive service and log layout defaults.
    pub tool_name: ToolName,
    /// Root directory that owns the routing runtime log tree.
    pub log_root: PathBuf,
    /// Environment-variable prefix used by the owning application.
    pub env_prefix: EnvPrefix,
    /// Reserved for future async/backpressure implementation. Phase 1 execution is synchronous; this value is stored but not yet applied.
    pub queue_capacity: usize,
    /// Retained-log policy forwarded to the built-in logging layer.
    pub retained_log_policy: RetainedLogPolicy,
}

impl ObservabilityConfig {
    /// Builds the documented v1 defaults from a tool name and log root.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use sc_observability_types::ToolName;
    /// use sc_observe::ObservabilityConfig;
    ///
    /// let config = ObservabilityConfig::default_for(
    ///     ToolName::new("demo-tool").expect("valid tool"),
    ///     PathBuf::from("logs"),
    /// )
    /// .expect("valid config");
    ///
    /// assert_eq!(config.tool_name.as_str(), "demo-tool");
    /// ```
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility accessor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use ObservabilityConfig::default_for_typed(); see migrate-error-api.md."
    )]
    pub fn default_for(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitError> {
        Self::default_for_typed(tool_name, log_root).map_err(Into::into)
    }

    /// Builds the documented defaults with a typed construction failure.
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitFailure> {
        let env_prefix = EnvPrefix::new(
            tool_name
                .as_str()
                .replace(['-', '.'], "_")
                .to_ascii_uppercase(),
        )
        .map_err(|err| {
            InitFailure::observation_initialization(
                "failed to derive env prefix",
                Remediation::not_recoverable("use an explicit valid env prefix"),
            )
            .cause(err.to_string())
            .source(Box::new(err))
        })?;
        Ok(Self {
            tool_name,
            log_root,
            env_prefix,
            queue_capacity: constants::DEFAULT_OBSERVATION_QUEUE_CAPACITY,
            retained_log_policy: RetainedLogPolicy::default(),
        })
    }

    /// Derives the logging/telemetry service name from the configured tool.
    #[allow(
        deprecated,
        reason = "retained compatibility accessor keeps the published InitError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility accessor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use ObservabilityConfig::service_name_typed(); see migrate-error-api.md."
    )]
    pub fn service_name(&self) -> Result<ServiceName, InitError> {
        self.service_name_typed().map_err(Into::into)
    }

    /// Derives the logging/telemetry service name with a typed failure.
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure> {
        ServiceName::new(self.tool_name.as_str()).map_err(|err| {
            InitFailure::observation_initialization(
                "failed to derive service name",
                Remediation::not_recoverable("use a valid tool name"),
            )
            .cause(err.to_string())
            .source(Box::new(err))
        })
    }

    fn logger_config_typed(&self) -> Result<LoggerConfig, InitFailure> {
        let mut config =
            LoggerConfig::default_for(self.service_name_typed()?, self.log_root.clone());
        config.queue_capacity = self.queue_capacity;
        config.retained_log_policy = self.retained_log_policy;
        Ok(config)
    }
}

/// Builder for construction-time subscriber and projector registration.
#[expect(
    missing_debug_implementations,
    reason = "the builder stores type-erased routing closures and health providers whose internals are not part of the public debug contract"
)]
pub struct ObservabilityBuilder {
    config: ObservabilityConfig,
    subscribers: Vec<ErasedSubscriberRegistration>,
    projections: Vec<ErasedProjectionRegistration>,
    observability_health_provider: Option<Arc<dyn ObservabilityHealthProvider>>,
}

/// Producer-facing routing runtime for typed observations.
#[expect(
    missing_debug_implementations,
    reason = "the runtime owns atomic state, mutexes, and type-erased routes that do not have a useful stable Debug representation"
)]
pub struct Observability {
    // MUTEX: state transitions replace the logger handle atomically. Blocking
    // writer shutdown occurs after publishing `ShuttingDown`, so callers never
    // observe an absent handle while emit, flush, and health race shutdown.
    logger: Mutex<LoggerHandle>,
    logger_changed: Condvar,
    shutdown: AtomicBool,
    subscriber_registrations: Vec<ErasedSubscriberRegistration>,
    projection_registrations: Vec<ErasedProjectionRegistration>,
    observability_health_provider: Option<Arc<dyn ObservabilityHealthProvider>>,
    runtime: RuntimeState,
}

#[derive(Default)]
struct RuntimeState {
    dropped_observations_total: AtomicU64,
    subscriber_failures_total: AtomicU64,
    projection_failures_total: AtomicU64,
    // MUTEX: routing failures update the shared last_error summary from multiple subscriber and
    // projector call paths; Mutex keeps the optional summary coherent as one unit, and RwLock
    // adds no value because writes dominate error reporting.
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

enum LoggerHandle {
    Running(Logger<Running>),
    ShuttingDown,
    Stopped(Logger<Stopped>),
}

#[allow(
    deprecated,
    reason = "routing keeps the published SubscriberError callback boundary"
)]
type SubscriberDispatchFn =
    dyn Fn(&dyn Any) -> Result<DispatchMatch, SubscriberError> + Send + Sync + 'static;
type ProjectionDispatchFn =
    dyn Fn(&dyn Any, &Logger<Running>) -> ProjectionDispatchResult + Send + Sync + 'static;

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

fn log_error_summary(error: &LogError) -> DiagnosticSummary {
    match error {
        LogError::InvalidEvent(error) => DiagnosticSummary::from(error.diagnostic()),
        LogError::WriterDegraded(error) | LogError::ShutdownTimedOut(error) => {
            DiagnosticSummary::from(error.diagnostic())
        }
    }
}

impl Observability {
    /// Builds a runtime using the documented default logger integration.
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Observability::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(config: ObservabilityConfig) -> Result<Self, InitError> {
        Self::new_typed(config).map_err(Into::into)
    }

    /// Builds a runtime using typed construction and initialization failures.
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure> {
        Self::builder(config).build_typed()
    }

    /// Starts a construction-time builder for subscribers and projections.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use sc_observability_types::ToolName;
    /// use sc_observe::{Observability, ObservabilityConfig};
    ///
    /// let config = ObservabilityConfig::default_for(
    ///     ToolName::new("demo-tool").expect("valid tool"),
    ///     PathBuf::from("logs"),
    /// )
    /// .expect("valid config");
    ///
    /// let _builder = Observability::builder(config);
    /// ```
    pub fn builder(config: ObservabilityConfig) -> ObservabilityBuilder {
        ObservabilityBuilder {
            config,
            subscribers: Vec::new(),
            projections: Vec::new(),
            observability_health_provider: None,
        }
    }

    /// Routes one typed observation through the registered subscribers and projections.
    ///
    /// # Panics
    ///
    /// Panics if the internal last-error mutex has been poisoned while the
    /// runtime records a routing, subscriber, or projection failure summary.
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
            let logger = self.logger.lock().expect("observability logger poisoned");
            let LoggerHandle::Running(logger) = &*logger else {
                return Err(ObservationError::Shutdown);
            };
            let result = (registration.dispatch)(observation_any, logger);
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
    ///
    /// # Panics
    ///
    /// Panics if the attached logger encounters a poisoned internal mutex while
    /// flushing its registered sinks.
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published FlushError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published FlushError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Observability::flush_typed(); see migrate-error-api.md."
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_typed().map_err(Into::into)
    }

    /// Flushes the attached logger with a typed failure.
    ///
    /// # Panics
    ///
    /// Panics if the attached logger encounters a poisoned internal mutex while
    /// flushing its registered sinks.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        let mut logger = self.logger.lock().expect("observability logger poisoned");
        while matches!(&*logger, LoggerHandle::ShuttingDown) {
            logger = self
                .logger_changed
                .wait(logger)
                .expect("observability logger poisoned");
        }
        match &*logger {
            LoggerHandle::Running(logger) => logger.flush_typed(),
            LoggerHandle::ShuttingDown | LoggerHandle::Stopped(_) => Ok(()),
        }
    }

    /// Shuts down the routing runtime. Repeated calls are idempotent.
    ///
    /// # Panics
    ///
    /// Panics if the attached logger encounters a poisoned internal mutex while
    /// flushing sinks or updating query/follow health during shutdown.
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published ShutdownError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published ShutdownError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Observability::shutdown_typed(); see migrate-error-api.md."
    )]
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_typed().map_err(Into::into)
    }

    /// Shuts down the routing runtime with a typed failure. Repeated calls are
    /// idempotent and return success, matching the legacy lifecycle contract.
    ///
    /// # Panics
    ///
    /// Panics if the attached logger encounters a poisoned internal mutex while
    /// shutting down its writer runtime.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let handle = {
            let mut logger = self.logger.lock().expect("observability logger poisoned");
            std::mem::replace(&mut *logger, LoggerHandle::ShuttingDown)
        };
        self.complete_shutdown(|| match handle {
            LoggerHandle::Running(logger) => LoggerHandle::Stopped(logger.shutdown()),
            LoggerHandle::ShuttingDown => unreachable!("shutdown has one owner"),
            LoggerHandle::Stopped(logger) => LoggerHandle::Stopped(logger),
        });
        Ok(())
    }

    fn complete_shutdown(&self, shutdown: impl FnOnce() -> LoggerHandle) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(shutdown));
        let mut logger = self.logger.lock().expect("observability logger poisoned");
        self.logger_changed.notify_all();
        match result {
            Ok(stopped) => *logger = stopped,
            // Resume while holding the mutex so it is poisoned just as it was
            // when shutdown ran under the lock. Woken waiters observe that
            // poison instead of waiting forever on ShuttingDown after unwind.
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Returns the aggregate runtime health view.
    ///
    /// # Panics
    ///
    /// Panics if the internal last-error mutex has been poisoned.
    pub fn health(&self) -> ObservabilityHealthReport {
        let logging = {
            let mut logger = self.logger.lock().expect("observability logger poisoned");
            while matches!(&*logger, LoggerHandle::ShuttingDown) {
                logger = self
                    .logger_changed
                    .wait(logger)
                    .expect("observability logger poisoned");
            }
            match &*logger {
                LoggerHandle::Running(logger) => Some(logger.health()),
                LoggerHandle::ShuttingDown => unreachable!("waited for shutdown completion"),
                LoggerHandle::Stopped(logger) => Some(logger.health()),
            }
        };
        let telemetry = self
            .observability_health_provider
            .as_ref()
            .map(sc_observability_types::ObservabilityHealthProvider::telemetry_health);
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
            || logging.as_ref().is_some_and(|logging| {
                logging.state != sc_observability_types::LoggingHealthState::Healthy
            })
            || telemetry.as_ref().is_some_and(|health| {
                matches!(
                    health.state,
                    TelemetryHealthState::Degraded | TelemetryHealthState::Unavailable
                )
            })
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
            logging,
            telemetry,
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
    /// Attaches a generic telemetry health provider without introducing an
    /// OTLP crate dependency.
    #[expect(
        clippy::implied_bounds_in_impls,
        reason = "the public API intentionally spells out Send + Sync per QA-BP-IMC-007"
    )]
    pub fn with_observability_health_provider(
        mut self,
        provider: impl ObservabilityHealthProvider + Send + Sync + 'static,
    ) -> Self {
        self.observability_health_provider = Some(Arc::new(provider));
        self
    }

    /// Registers one typed observation subscriber at construction time.
    ///
    /// # Panics
    ///
    /// Panics if internal type-erased routing calls this registration with the
    /// wrong observation payload type.
    pub fn register_subscriber<T>(mut self, registration: SubscriberRegistration<T>) -> Self
    where
        T: Observable,
    {
        let (subscriber, filter) = registration.into_parts();
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

                subscriber.observe(observation)?;
                Ok(DispatchMatch::Delivered)
            }),
        });
        self
    }

    /// Registers one typed observation projection set at construction time.
    ///
    /// # Panics
    ///
    /// Panics if internal type-erased routing calls this registration with the
    /// wrong observation payload type.
    pub fn register_projection<T>(mut self, registration: ProjectionRegistration<T>) -> Self
    where
        T: Observable,
    {
        let (log_projector, span_projector, metric_projector, filter) = registration.into_parts();

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
                                if let Err(err) = logger.log_typed(event) {
                                    let err: LogError = err.into();
                                    record_failure(log_error_summary(&err));
                                }
                            }
                            if let Err(err) = logger.flush_typed() {
                                record_failure(DiagnosticSummary::from(err.diagnostic()));
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
    #[allow(
        deprecated,
        reason = "retained compatibility builder method keeps the published InitError signature"
    )]
    #[allow(
        deprecated,
        reason = "retained compatibility builder method keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use ObservabilityBuilder::build_typed(); see migrate-error-api.md."
    )]
    pub fn build(self) -> Result<Observability, InitError> {
        self.build_typed().map_err(Into::into)
    }

    /// Finalizes registration and constructs the runtime with typed failures.
    pub fn build_typed(self) -> Result<Observability, InitFailure> {
        if self.subscribers.is_empty() && self.projections.is_empty() {
            return Err(InitFailure::observation_initialization(
                "at least one subscriber or projector route must be registered",
                Remediation::recoverable(
                    "register a subscriber or projector before building observability",
                    ["add at least one route for the observation types you emit"],
                ),
            ));
        }
        let logger = Logger::new_typed(self.config.logger_config_typed()?)?;
        Ok(Observability {
            logger: Mutex::new(LoggerHandle::Running(logger)),
            logger_changed: Condvar::new(),
            shutdown: AtomicBool::new(false),
            subscriber_registrations: self.subscribers,
            projection_registrations: self.projections,
            observability_health_provider: self.observability_health_provider,
            runtime: RuntimeState::default(),
        })
    }
}

mod sealed_emitters {
    pub trait Sealed {}
}

/// `ObservationEmitter<T>` is intentionally per-type -- callers hold one handle
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
#[allow(
    deprecated,
    reason = "routing compatibility tests exercise retained legacy registrations and errors"
)]
mod tests {
    use super::*;
    use sc_observability::{
        LogFilter, LogSink, LoggerConfig, SinkHealth, SinkHealthState, SinkRegistration,
    };
    use sc_observability_types::typed::{
        ClassifiedError, FlushFailureKind, InitFailureKind, SubscriberFailure,
        TypedObservationSubscriber, legacy_subscriber,
    };
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, LogSinkError, MetricKind, MetricName,
        MetricRecord, MetricUnit, ObservationFilter, ObservationSubscriber, ProcessIdentity,
        ProjectionError, SpanId, SpanProjector, SpanRecord, SpanSignal, SpanStarted,
        SubscriberError, TargetCategory, TelemetryHealthReport, TelemetryHealthState, Timestamp,
        TraceContext, TraceId,
    };
    use serde_json::Map;

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
        fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
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
        fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberError> {
            Err(SubscriberError(Box::new(ErrorContext::new(
                error_codes::OBSERVATION_ROUTING_FAILURE,
                "subscriber failed",
                Remediation::not_recoverable("test subscriber intentionally fails"),
            ))))
        }
    }

    struct TypedRecordingSubscriber {
        calls: Arc<AtomicU64>,
    }

    impl TypedObservationSubscriber<AgentEvent> for TypedRecordingSubscriber {
        fn observe(&self, _observation: &Observation<AgentEvent>) -> Result<(), SubscriberFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
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
                Map::default(),
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
                unit: Some(MetricUnit::new("1").expect("valid metric unit")),
                attributes: Map::default(),
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

    struct FakeTelemetryProvider {
        state: TelemetryHealthState,
    }

    impl sc_observability_types::telemetry_health_provider_sealed::Sealed for FakeTelemetryProvider {
        fn token(&self) -> sc_observability_types::telemetry_health_provider_sealed::Token {
            sc_observability_types::telemetry_health_provider_sealed::workspace_token()
        }
    }

    impl ObservabilityHealthProvider for FakeTelemetryProvider {
        fn telemetry_health(&self) -> TelemetryHealthReport {
            TelemetryHealthReport {
                state: self.state,
                dropped_exports_total: 0,
                malformed_spans_total: 0,
                exporter_statuses: Vec::new(),
                last_error: None,
            }
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

    fn schema_version() -> sc_observability_types::SchemaVersion {
        sc_observability_types::SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version")
    }

    fn outcome_label(value: &str) -> sc_observability_types::OutcomeLabel {
        sc_observability_types::OutcomeLabel::new(value).expect("valid outcome label")
    }

    fn sink_name(value: &str) -> sc_observability_types::SinkName {
        sc_observability_types::SinkName::new(value).expect("valid sink name")
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
            version: schema_version(),
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
            outcome: Some(outcome_label("ok")),
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
        }
    }

    #[test]
    fn registration_order_routing_is_deterministic() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let root = temp_path("order");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "first",
                calls: calls.clone(),
            })))
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "second",
                calls: calls.clone(),
            })))
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
            .register_subscriber(
                SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                    id: "allowed",
                    calls: calls.clone(),
                }))
                .with_filter(Arc::new(AllowFlagFilter)),
            )
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
            .register_subscriber(SubscriberRegistration::new(Arc::new(FailingSubscriber)))
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "still-runs",
                calls: calls.clone(),
            })))
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
            .register_projection(
                ProjectionRegistration::new()
                    .with_log_projector(Arc::new(FailingProjector))
                    .with_span_projector(Arc::new(RecordingSpanProjector {
                        count: span_count.clone(),
                    }))
                    .with_metric_projector(Arc::new(RecordingMetricProjector {
                        count: metric_count.clone(),
                    })),
            )
            .register_projection(ProjectionRegistration::new().with_log_projector(Arc::new(
                RecordingLogProjector {
                    calls: log_calls.clone(),
                    id: "log",
                },
            )))
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
            .register_subscriber(
                SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                    id: "filtered",
                    calls: Arc::new(Mutex::new(Vec::new())),
                }))
                .with_filter(Arc::new(AllowFlagFilter)),
            )
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
            .register_projection(
                ProjectionRegistration::new().with_log_projector(Arc::new(FailingProjector)),
            )
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
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "shutdown",
                calls: Arc::new(Mutex::new(Vec::new())),
            })))
            .build()
            .expect("runtime");

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
            .register_projection(
                ProjectionRegistration::new().with_log_projector(Arc::new(FailingProjector)),
            )
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
    fn top_level_health_exposes_attached_telemetry_provider() {
        let root = temp_path("telemetry-health");
        let config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "telemetry-health",
                calls: Arc::new(Mutex::new(Vec::new())),
            })))
            .with_observability_health_provider(Arc::new(FakeTelemetryProvider {
                state: TelemetryHealthState::Degraded,
            }))
            .build()
            .expect("runtime");

        let health = runtime.health();

        assert_eq!(health.state, ObservationHealthState::Degraded);
        assert_eq!(
            health.telemetry.expect("telemetry health").state,
            TelemetryHealthState::Degraded
        );
    }

    #[test]
    fn queue_capacity_override_propagates_to_logger_config() {
        let root = temp_path("queue-capacity");
        let mut config = ObservabilityConfig::default_for(tool_name(), root).expect("config");
        config.queue_capacity = 2048;

        let logger_config = config.logger_config_typed().expect("logger config");

        assert_eq!(logger_config.queue_capacity, 2048);
    }

    #[test]
    fn typed_builder_uses_real_adapters_and_preserves_lifecycle_contract() {
        let root = temp_path("typed-lifecycle");
        let calls = Arc::new(AtomicU64::new(0));
        let config =
            ObservabilityConfig::default_for_typed(tool_name(), root).expect("typed config");
        assert_eq!(
            config.service_name_typed().expect("typed service").as_str(),
            "obs-app"
        );

        let runtime = Observability::builder(config)
            .register_subscriber(SubscriberRegistration::new(legacy_subscriber(Arc::new(
                TypedRecordingSubscriber {
                    calls: calls.clone(),
                },
            ))))
            .build_typed()
            .expect("typed runtime");

        runtime.emit(observation(true)).expect("typed emit");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        runtime.flush_typed().expect("typed flush");
        runtime.shutdown_typed().expect("typed shutdown");
        runtime.shutdown_typed().expect("repeated typed shutdown");
        assert!(matches!(
            runtime.emit(observation(true)),
            Err(ObservationError::Shutdown)
        ));
        runtime.flush_typed().expect("flush after shutdown");
    }

    #[test]
    fn typed_builder_reports_empty_routes_and_logger_startup_failures() {
        let Err(empty) = Observability::builder(
            ObservabilityConfig::default_for_typed(tool_name(), temp_path("typed-empty"))
                .expect("typed config"),
        )
        .build_typed() else {
            panic!("empty routes must fail");
        };
        assert_eq!(empty.kind(), InitFailureKind::ObservationInitialization);

        let mut config =
            ObservabilityConfig::default_for_typed(tool_name(), temp_path("typed-init-failure"))
                .expect("typed config");
        config.queue_capacity = 0;
        let Err(error) = Observability::builder(config)
            .register_subscriber(SubscriberRegistration::new(legacy_subscriber(Arc::new(
                TypedRecordingSubscriber {
                    calls: Arc::new(AtomicU64::new(0)),
                },
            ))))
            .build_typed()
        else {
            panic!("zero queue capacity must fail");
        };
        assert_eq!(error.kind(), InitFailureKind::LoggerInitialization);
    }

    #[test]
    fn concurrent_typed_shutdown_is_idempotent() {
        let runtime = Arc::new(
            Observability::builder(
                ObservabilityConfig::default_for_typed(tool_name(), temp_path("typed-concurrent"))
                    .expect("typed config"),
            )
            .register_subscriber(SubscriberRegistration::new(legacy_subscriber(Arc::new(
                TypedRecordingSubscriber {
                    calls: Arc::new(AtomicU64::new(0)),
                },
            ))))
            .build_typed()
            .expect("typed runtime"),
        );

        let (completed_tx, completed_rx) = std::sync::mpsc::channel();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let runtime = runtime.clone();
                let completed_tx = completed_tx.clone();
                std::thread::spawn(move || {
                    let result = runtime.shutdown_typed();
                    completed_tx
                        .send(result)
                        .expect("shutdown completion receiver");
                })
            })
            .collect();
        drop(completed_tx);
        for _ in 0..8 {
            completed_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("bounded shutdown completion")
                .expect("shutdown");
        }
        for handle in handles {
            handle.join().expect("shutdown thread");
        }
        assert_eq!(runtime.health().state, ObservationHealthState::Unavailable);
    }

    #[test]
    fn in_flight_shutdown_preserves_flush_health_and_repeated_shutdown() {
        use std::sync::mpsc;
        use std::time::Duration;

        struct BlockingFlushSink {
            armed: Arc<AtomicBool>,
            entered: mpsc::Sender<()>,
            // MUTEX: LogSink is Sync; the sole writer owns receives on this
            // test-control channel. A timeout/disconnect releases failed tests.
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl LogSink for BlockingFlushSink {
            fn write(&self, _: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }
            fn flush(&self) -> Result<(), LogSinkError> {
                if self.armed.swap(false, Ordering::SeqCst) {
                    let _ = self.entered.send(());
                    let _ = self
                        .release
                        .lock()
                        .expect("release lock")
                        .recv_timeout(Duration::from_secs(5));
                }
                Err(LogSinkError(Box::new(ErrorContext::new(
                    sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                    "controlled flush failure",
                    Remediation::not_recoverable("test fixture"),
                ))))
            }
            fn health(&self) -> SinkHealth {
                SinkHealth {
                    name: sink_name("controlled-flush"),
                    state: SinkHealthState::DegradedDropping,
                    last_error: None,
                }
            }
        }
        for legacy in [false, true] {
            let armed = Arc::new(AtomicBool::new(false));
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let mut config = LoggerConfig::default_for(
                ServiceName::new("obs-app").expect("service"),
                temp_path("controlled-shutdown"),
            );
            config.enable_file_sink = false;
            config.enable_console_sink = false;
            let mut builder = Logger::builder(config).expect("logger builder");
            builder.register_sink(SinkRegistration::new(Arc::new(BlockingFlushSink {
                armed: armed.clone(),
                entered: entered_tx,
                release: Mutex::new(release_rx),
            })));
            let logger = builder.build();
            logger
                .flush_typed()
                .expect_err("seed logging failure counter");
            let before = logger.health();
            assert_eq!(before.flush_errors_total, 1);
            assert!(before.last_error.is_some());
            let runtime = Arc::new(Observability {
                logger: Mutex::new(LoggerHandle::Running(logger)),
                logger_changed: Condvar::new(),
                shutdown: AtomicBool::new(false),
                subscriber_registrations: Vec::new(),
                projection_registrations: Vec::new(),
                observability_health_provider: None,
                runtime: RuntimeState::default(),
            });
            armed.store(true, Ordering::SeqCst);
            let (shutdown_tx, shutdown_rx) = mpsc::channel();
            let shutdown_runtime = runtime.clone();
            let shutdown = std::thread::spawn(move || {
                let result = if legacy {
                    shutdown_runtime.shutdown().map_err(ShutdownFailure::from)
                } else {
                    shutdown_runtime.shutdown_typed()
                };
                let _ = shutdown_tx.send(result);
            });
            entered_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("writer inside controlled final flush");
            assert!(matches!(
                *runtime
                    .logger
                    .lock()
                    .expect("logger unlocked during shutdown"),
                LoggerHandle::ShuttingDown
            ));
            assert!(matches!(
                runtime.emit(observation(true)),
                Err(ObservationError::Shutdown)
            ));
            let (repeat_tx, repeat_rx) = mpsc::channel();
            let repeated_runtime = runtime.clone();
            let repeated = std::thread::spawn(move || {
                let _ = repeat_tx.send(repeated_runtime.shutdown_typed());
            });
            repeat_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("repeated shutdown is immediate")
                .expect("success");
            repeated.join().expect("repeated shutdown thread");

            let (started_tx, started_rx) = mpsc::channel();
            let (flush_tx, flush_rx) = mpsc::channel();
            let flush_runtime = runtime.clone();
            let flush_started = started_tx.clone();
            let flush = std::thread::spawn(move || {
                let _ = flush_started.send(());
                let result = if legacy {
                    flush_runtime.flush().map_err(FlushFailure::from)
                } else {
                    flush_runtime.flush_typed()
                };
                let _ = flush_tx.send(result);
            });
            let (health_tx, health_rx) = mpsc::channel();
            let health_runtime = runtime.clone();
            let health = std::thread::spawn(move || {
                let _ = started_tx.send(());
                let _ = health_tx.send(health_runtime.health());
            });
            for _ in 0..2 {
                started_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("waiter started");
            }
            assert!(
                matches!(
                    flush_rx.recv_timeout(Duration::from_millis(50)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ),
                "flush must wait for original writer"
            );
            assert!(
                matches!(
                    health_rx.recv_timeout(Duration::from_millis(50)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ),
                "health must wait for retained stopped snapshot"
            );
            assert!(matches!(
                shutdown_rx.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            release_tx.send(()).expect("release original writer");
            shutdown_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("bounded original shutdown")
                .expect("shutdown success");
            flush_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("bounded flush completion")
                .expect("stopped flush succeeds");
            let report = health_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("bounded health completion");
            assert_eq!(report.state, ObservationHealthState::Unavailable);
            let after = report.logging.expect("logging health retained");
            assert_eq!(after.flush_errors_total, before.flush_errors_total);
            assert_eq!(after.dropped_events_total, before.dropped_events_total);
            assert_eq!(after.queue_capacity, before.queue_capacity);
            assert_eq!(
                after.last_error.as_ref().expect("retained diagnostic").code,
                before
                    .last_writer_error
                    .as_ref()
                    .expect("original writer diagnostic")
                    .code
            );
            let final_writer = after
                .last_writer_error
                .as_ref()
                .expect("final writer diagnostic");
            let original_writer = before
                .last_writer_error
                .as_ref()
                .expect("original writer diagnostic");
            assert_eq!(final_writer.code, original_writer.code);
            assert_eq!(final_writer.message, original_writer.message);
            assert!(final_writer.at >= original_writer.at);
            assert_eq!(after.sink_statuses, before.sink_statuses);
            for thread in [shutdown, flush, health] {
                thread.join().expect("completed worker");
            }
        }
    }

    #[test]
    fn shutdown_unwind_releases_condition_waiters() {
        use std::sync::mpsc;
        use std::time::Duration;
        let runtime = Arc::new(Observability {
            logger: Mutex::new(LoggerHandle::ShuttingDown),
            logger_changed: Condvar::new(),
            shutdown: AtomicBool::new(true),
            subscriber_registrations: Vec::new(),
            projection_registrations: Vec::new(),
            observability_health_provider: None,
            runtime: RuntimeState::default(),
        });
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let threads: Vec<_> = [false, true]
            .into_iter()
            .map(|health| {
                let runtime = runtime.clone();
                let started_tx = started_tx.clone();
                let done_tx = done_tx.clone();
                std::thread::spawn(move || {
                    let _ = started_tx.send(());
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if health {
                            let _ = runtime.health();
                        } else {
                            let _ = runtime.flush_typed();
                        }
                    }));
                    let _ = done_tx.send(result.is_err());
                })
            })
            .collect();
        for _ in 0..2 {
            started_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("waiter started");
        }
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.complete_shutdown(|| panic!("injected shutdown unwind"));
        }));
        assert!(result.is_err());
        for _ in 0..2 {
            assert!(
                done_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("waiter observes poison")
            );
        }
        for thread in threads {
            thread.join().expect("bounded waiter");
        }
    }

    #[test]
    fn flush_forwards_logger_flush_behavior_directly() {
        struct PassthroughFilter;

        impl LogFilter for PassthroughFilter {
            fn accepts(&self, _event: &LogEvent) -> bool {
                true
            }
        }

        struct FlushFailSink {
            flush_calls: Arc<AtomicU64>,
            second_flush_completed: std::sync::mpsc::Sender<()>,
        }

        impl LogSink for FlushFailSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }

            fn flush(&self) -> Result<(), LogSinkError> {
                let call = self.flush_calls.fetch_add(1, Ordering::SeqCst);
                let result = Err(LogSinkError(Box::new(ErrorContext::new(
                    sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                    "flush failed",
                    Remediation::not_recoverable("test sink intentionally fails flush"),
                ))));
                if call == 1 {
                    let _ = self.second_flush_completed.send(());
                }
                result
            }

            fn health(&self) -> SinkHealth {
                SinkHealth {
                    name: sink_name("flush-fail"),
                    state: SinkHealthState::DegradedDropping,
                    last_error: None,
                }
            }
        }

        let ok_root = temp_path("flush-ok");
        let ok_config =
            ObservabilityConfig::default_for(tool_name(), ok_root.clone()).expect("config");
        let ok_runtime = Observability::builder(ok_config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "flush-ok",
                calls: Arc::new(Mutex::new(Vec::new())),
            })))
            .build()
            .expect("runtime");
        assert!(ok_runtime.flush().is_ok());

        let build_failing_runtime = |name: &str| {
            let flush_calls = Arc::new(AtomicU64::new(0));
            let (second_flush_completed, second_flush_rx) = std::sync::mpsc::channel();
            let mut logger_config = LoggerConfig::default_for(
                ServiceName::new("obs-app").expect("service"),
                temp_path(name),
            );
            logger_config.enable_file_sink = false;
            logger_config.enable_console_sink = false;
            let mut builder =
                sc_observability::Logger::builder(logger_config).expect("logger builder");
            builder.register_sink(
                SinkRegistration::new(Arc::new(FlushFailSink {
                    flush_calls: flush_calls.clone(),
                    second_flush_completed,
                }))
                .with_filter(Arc::new(PassthroughFilter)),
            );
            let logger = builder.build();

            let runtime = Observability {
                logger: Mutex::new(LoggerHandle::Running(logger)),
                logger_changed: Condvar::new(),
                shutdown: AtomicBool::new(false),
                subscriber_registrations: Vec::new(),
                projection_registrations: Vec::new(),
                observability_health_provider: None,
                runtime: RuntimeState::default(),
            };
            (runtime, flush_calls, second_flush_rx)
        };

        let (legacy_runtime, legacy_flush_calls, legacy_second_flush_rx) =
            build_failing_runtime("flush-legacy");
        let (typed_runtime, typed_flush_calls, typed_second_flush_rx) =
            build_failing_runtime("flush-typed");
        let Err(legacy_error) = legacy_runtime.flush() else {
            panic!("legacy flush must report sink failure");
        };
        legacy_second_flush_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("bounded legacy second flush completion");
        let Err(typed_error) = typed_runtime.flush_typed() else {
            panic!("typed flush must report sink failure");
        };
        assert_eq!(legacy_error.kind(), FlushFailureKind::LoggerFlush);
        assert_eq!(typed_error.kind(), FlushFailureKind::LoggerFlush);
        typed_second_flush_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("bounded typed second flush completion");
        assert_eq!(legacy_flush_calls.load(Ordering::SeqCst), 2);
        assert_eq!(typed_flush_calls.load(Ordering::SeqCst), 2);
        for runtime in [&legacy_runtime, &typed_runtime] {
            let logging = runtime.health().logging.expect("logging health");
            assert_eq!(logging.flush_errors_total, 1);
            assert!(logging.last_error.is_some());
        }
    }
}
