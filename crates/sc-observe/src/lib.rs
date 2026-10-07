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
#[cfg(feature = "v1")]
mod v1;

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
)]
use sc_observability::LogError;
#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
)]
use sc_observability::Logger as ReleasedLogger;
#[cfg(feature = "v1")]
use sc_observability::Running;
use sc_observability::v2::Logger;
use sc_observability::{LoggerConfig, RetainedLogPolicy};
#[cfg(feature = "v1")]
use sc_observability_types::DiagnosticInfo;
#[cfg(feature = "v1")]
#[allow(deprecated)]
use sc_observability_types::typed::FlushFailure;
use sc_observability_types::v2::{
    FlushError as CanonicalFlushError, InitError as CanonicalInitError, ObservationFilter,
    ProjectionRegistration as CanonicalProjectionRegistration,
    ShutdownError as CanonicalShutdownError, SubscriberError,
    SubscriberRegistration as CanonicalSubscriberRegistration,
};
use sc_observability_types::{
    DiagnosticSummary, EnvPrefix, ErrorContext, FailureClassification, LogEvent,
    ObservabilityHealthProvider, Observable, Observation, Remediation, ServiceName,
    TelemetryHealthState, ToolName,
};
#[doc(inline)]
pub use sc_observability_types::{
    ObservabilityHealthReport, ObservationError, ObservationHealthState,
};
#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "released root registration signatures remain available only through v1"
)]
use sc_observability_types::{
    ProjectionRegistration as LegacyProjectionRegistration,
    SubscriberRegistration as LegacySubscriberRegistration,
};

/// Canonical observation facade for the compatible 1.x transition.
pub mod v2 {
    #[doc(inline)]
    pub use crate::canonical::{Observability, ObservabilityBuilder, ObservabilityConfig};
    #[doc(inline)]
    pub use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
    #[doc(inline)]
    pub use sc_observability_types::v2::{ProjectionRegistration, SubscriberRegistration};
    #[doc(inline)]
    pub use sc_observability_types::{
        ObservabilityHealthProvider, ObservabilityHealthReport, Observable, Observation,
        ObservationError, ServiceName, ToolName,
    };
}

mod canonical {
    use super::{
        Any, Arc, AtomicBool, AtomicU64, CanonicalFlushError, CanonicalInitError,
        CanonicalProjectionRegistration, CanonicalShutdownError, CanonicalSubscriberRegistration,
        Condvar, DiagnosticSummary, Duration, EnvPrefix, ErrorContext, FailureClassification,
        LogEvent, Logger, LoggerConfig, Mutex, ObservabilityHealthProvider,
        ObservabilityHealthReport, Observable, Observation, ObservationError, ObservationFilter,
        ObservationHealthState, Ordering, PathBuf, Remediation, RetainedLogPolicy, Running,
        ServiceName, SubscriberError, TelemetryHealthState, ToolName, TypeId, constants,
        error_codes,
    };
    #[cfg(feature = "v1")]
    #[allow(
        deprecated,
        reason = "the released logger and registration contracts remain isolated in the v1 facade"
    )]
    use super::{
        DiagnosticInfo, FlushFailure, LegacyProjectionRegistration, LegacySubscriberRegistration,
        LogError, ReleasedLogger,
    };

    /// Top-level configuration for the canonical observation routing runtime.
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
        /// Builds defaults with canonical initialization errors.
        pub fn default_for(
            tool_name: ToolName,
            log_root: PathBuf,
        ) -> Result<Self, CanonicalInitError> {
            let env_prefix = EnvPrefix::new(
                tool_name
                    .as_str()
                    .replace(['-', '.'], "_")
                    .to_ascii_uppercase(),
            )
            .map_err(|err| CanonicalInitError::Configuration {
                context: Box::new(
                    ErrorContext::new(
                        error_codes::OBSERVABILITY_INIT_FAILED,
                        "failed to derive env prefix",
                        Remediation::not_recoverable("use an explicit valid env prefix"),
                    )
                    .cause(err.to_string())
                    .source(Box::new(err)),
                ),
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
        pub fn service_name(&self) -> Result<ServiceName, CanonicalInitError> {
            ServiceName::new(self.tool_name.as_str()).map_err(|err| {
                CanonicalInitError::Configuration {
                    context: Box::new(
                        ErrorContext::new(
                            error_codes::OBSERVABILITY_INIT_FAILED,
                            "failed to derive service name",
                            Remediation::not_recoverable("use a valid tool name"),
                        )
                        .cause(err.to_string())
                        .source(Box::new(err)),
                    ),
                }
            })
        }

        pub(crate) fn logger_config(&self) -> Result<LoggerConfig, CanonicalInitError> {
            let mut config = LoggerConfig::default_for(self.service_name()?, self.log_root.clone());
            if self.queue_capacity == 0 {
                return Err(CanonicalInitError::Runtime {
                    context: Box::new(ErrorContext::new(
                        sc_observability::error_codes::LOGGER_INIT_FAILED,
                        "queue capacity must be greater than zero",
                        Remediation::recoverable(
                            "set the observation queue capacity to a positive value",
                            ["increase queue_capacity to at least 1"],
                        ),
                    )),
                });
            }
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
        pub(crate) logger: Mutex<LoggerHandle>,
        pub(crate) logger_changed: Condvar,
        #[cfg(test)]
        pub(crate) logger_waiting: Option<std::sync::mpsc::Sender<&'static str>>,
        pub(crate) shutdown: AtomicBool,
        pub(crate) subscriber_registrations: Vec<ErasedSubscriberRegistration>,
        pub(crate) projection_registrations: Vec<ErasedProjectionRegistration>,
        pub(crate) observability_health_provider: Option<Arc<dyn ObservabilityHealthProvider>>,
        pub(crate) runtime: RuntimeState,
    }

    #[derive(Default)]
    pub(crate) struct RuntimeState {
        dropped_observations_total: AtomicU64,
        subscriber_failures_total: AtomicU64,
        projection_failures_total: AtomicU64,
        // MUTEX: routing failures update the shared last_error summary from multiple subscriber and
        // projector call paths; Mutex keeps the optional summary coherent as one unit, and RwLock
        // adds no value because writes dominate error reporting.
        last_error: Mutex<Option<DiagnosticSummary>>,
    }

    pub(crate) struct ErasedSubscriberRegistration {
        type_id: TypeId,
        dispatch: Arc<SubscriberDispatchFn>,
    }

    type ProjectLogsFn<T> =
        dyn Fn(&Observation<T>) -> Result<Vec<LogEvent>, DiagnosticSummary> + Send + Sync + 'static;
    type ProjectFn<T> =
        dyn Fn(&Observation<T>) -> Result<(), DiagnosticSummary> + Send + Sync + 'static;

    /// One projector family's routes, erased to the shared dispatch shape.
    struct ProjectionRoutes<T: Observable> {
        logs: Option<Arc<ProjectLogsFn<T>>>,
        spans: Option<Arc<ProjectFn<T>>>,
        metrics: Option<Arc<ProjectFn<T>>>,
        filter: Option<Arc<dyn ObservationFilter<T>>>,
    }

    pub(crate) struct ErasedProjectionRegistration {
        type_id: TypeId,
        dispatch: Arc<ProjectionDispatchFn>,
    }

    pub(crate) enum LoggerHandle {
        Running(RunningLogger),
        ShuttingDown,
        Stopped(sc_observability_types::LoggingHealthReport),
    }

    type SubscriberDispatchFn =
        dyn Fn(&dyn Any) -> Result<DispatchMatch, SubscriberError> + Send + Sync + 'static;
    type ProjectionDispatchFn =
        dyn Fn(&dyn Any, &RunningLogger) -> ProjectionDispatchResult + Send + Sync + 'static;

    /// Admission strictness pinned when a runtime is constructed.
    ///
    /// The v2 facade builds `Canonical`; the released root facade builds
    /// `Released`. No public selector or bypass exists.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RuntimeAdmission {
        Canonical,
        #[cfg(feature = "v1")]
        Released,
    }

    /// Running logger pinned to the admission mode of the facade that built it.
    pub(crate) enum RunningLogger {
        Canonical(Logger),
        #[cfg(feature = "v1")]
        #[allow(
            deprecated,
            reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
        )]
        Released(ReleasedLogger<Running>),
    }

    /// Flush failure carrying the native error shape of the owning facade.
    pub(crate) enum RunningFlushError {
        Canonical(CanonicalFlushError),
        #[cfg(feature = "v1")]
        #[allow(deprecated)]
        Released(FlushFailure),
    }

    fn shutdown_flush_error() -> RunningFlushError {
        RunningFlushError::Canonical(CanonicalFlushError::classified_drain(
            Box::new(ErrorContext::new(
                sc_observability::error_codes::LOGGER_WRITER_DEGRADED,
                "observability is shut down; construct a new runtime before flushing",
                Remediation::not_recoverable("create a new observability runtime"),
            )),
            FailureClassification::Closed,
        ))
    }

    #[cfg(feature = "v1")]
    #[allow(
        deprecated,
        reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
    )]
    fn released_log_error_summary(error: LogError) -> DiagnosticSummary {
        match error {
            LogError::InvalidEvent(error) => DiagnosticSummary::from(error.diagnostic()),
            LogError::WriterDegraded(error) | LogError::ShutdownTimedOut(error) => {
                DiagnosticSummary::from(error.diagnostic())
            }
        }
    }

    #[allow(deprecated)]
    impl RunningFlushError {
        pub(crate) fn summary(&self) -> DiagnosticSummary {
            match self {
                Self::Canonical(error) => DiagnosticSummary::from(error.diagnostic()),
                #[cfg(feature = "v1")]
                Self::Released(failure) => DiagnosticSummary::from(failure.diagnostic()),
            }
        }

        pub(crate) fn into_canonical(self) -> CanonicalFlushError {
            match self {
                Self::Canonical(error) => error,
                #[cfg(feature = "v1")]
                Self::Released(failure) => CanonicalFlushError::Drain {
                    context: failure.into_context(),
                },
            }
        }

        #[cfg(feature = "v1")]
        pub(crate) fn into_released(self) -> FlushFailure {
            match self {
                Self::Canonical(error) => FlushFailure::from(error),
                Self::Released(failure) => failure,
            }
        }
    }

    impl RunningLogger {
        fn flush(&self) -> Result<(), RunningFlushError> {
            match self {
                Self::Canonical(logger) => logger.flush().map_err(RunningFlushError::Canonical),
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => logger.flush_typed().map_err(RunningFlushError::Released),
            }
        }

        fn flush_with_timeout(&self, timeout: Duration) -> Result<(), RunningFlushError> {
            match self {
                Self::Canonical(logger) => logger
                    .flush_with_timeout(timeout)
                    .map_err(RunningFlushError::Canonical),
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => logger.flush_typed().map_err(RunningFlushError::Released),
            }
        }

        fn health(&self) -> sc_observability_types::LoggingHealthReport {
            match self {
                Self::Canonical(logger) => logger.health(),
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => logger.health(),
            }
        }

        fn shutdown(
            self,
        ) -> (
            sc_observability_types::LoggingHealthReport,
            Result<(), CanonicalShutdownError>,
        ) {
            match self {
                Self::Canonical(logger) => {
                    let result = logger.shutdown();
                    (logger.health(), result)
                }
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => (logger.shutdown().health(), Ok(())),
            }
        }

        fn shutdown_with_timeout(
            self,
            timeout: Duration,
        ) -> (
            sc_observability_types::LoggingHealthReport,
            Result<(), CanonicalShutdownError>,
        ) {
            match self {
                Self::Canonical(logger) => {
                    let result = logger.shutdown_with_timeout(timeout);
                    (logger.health(), result)
                }
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => (logger.shutdown().health(), Ok(())),
            }
        }

        fn log(&self, event: LogEvent) -> Result<(), DiagnosticSummary> {
            match self {
                Self::Canonical(logger) => logger
                    .log(event)
                    .map_err(|error| DiagnosticSummary::from(error.diagnostic())),
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission is isolated behind v1 in obs-f-8"
                )]
                Self::Released(logger) => logger.log(event).map_err(released_log_error_summary),
            }
        }
    }

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
        pub fn new(config: ObservabilityConfig) -> Result<Self, CanonicalInitError> {
            Self::builder(config).build()
        }

        /// Starts a construction-time builder for subscribers and projections.
        ///
        /// # Examples
        ///
        /// ```
        /// use std::path::PathBuf;
        /// use sc_observability_types::ToolName;
        /// use sc_observe::v2::{Observability, ObservabilityConfig};
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
        pub fn flush(&self) -> Result<(), CanonicalFlushError> {
            self.flush_running()
                .map_err(RunningFlushError::into_canonical)
        }

        /// Flushes the shared runtime within an explicit caller-provided timeout.
        pub fn flush_with_timeout(&self, timeout: Duration) -> Result<(), CanonicalFlushError> {
            self.flush_running_with_timeout(timeout)
                .map_err(RunningFlushError::into_canonical)
        }

        #[cfg(test)]
        pub(crate) fn flush_v2(&self) -> Result<(), CanonicalFlushError> {
            self.flush()
        }

        /// Flushes the attached logger, keeping the owning facade's error shape.
        pub(crate) fn flush_running(&self) -> Result<(), RunningFlushError> {
            let mut logger = self.logger.lock().expect("observability logger poisoned");
            while matches!(&*logger, LoggerHandle::ShuttingDown) {
                #[cfg(test)]
                if let Some(waiting) = &self.logger_waiting {
                    let _ = waiting.send("flush");
                }
                logger = self
                    .logger_changed
                    .wait(logger)
                    .expect("observability logger poisoned");
            }
            match &*logger {
                LoggerHandle::Running(logger) => logger.flush(),
                LoggerHandle::ShuttingDown | LoggerHandle::Stopped(_) => {
                    Err(shutdown_flush_error())
                }
            }
        }

        fn flush_running_with_timeout(&self, timeout: Duration) -> Result<(), RunningFlushError> {
            let mut logger = self.logger.lock().expect("observability logger poisoned");
            while matches!(&*logger, LoggerHandle::ShuttingDown) {
                logger = self
                    .logger_changed
                    .wait(logger)
                    .expect("observability logger poisoned");
            }
            match &*logger {
                LoggerHandle::Running(logger) => logger.flush_with_timeout(timeout),
                LoggerHandle::ShuttingDown | LoggerHandle::Stopped(_) => {
                    Err(shutdown_flush_error())
                }
            }
        }

        /// Shuts down the routing runtime. Repeated calls are idempotent.
        /// # Panics
        ///
        /// Panics if the internal logger-state mutex is poisoned. It also
        /// resumes panics from logger shutdown, including panics from poisoned
        /// writer-snapshot or query-health mutexes.
        pub fn shutdown(&self) -> Result<(), CanonicalShutdownError> {
            self.shutdown_inner(None)
        }

        /// Shuts down the shared runtime within an explicit caller-provided timeout.
        pub fn shutdown_with_timeout(
            &self,
            timeout: Duration,
        ) -> Result<(), CanonicalShutdownError> {
            self.shutdown_inner(Some(timeout))
        }

        fn shutdown_inner(&self, timeout: Option<Duration>) -> Result<(), CanonicalShutdownError> {
            if self.shutdown.swap(true, Ordering::SeqCst) {
                return Ok(());
            }
            let handle = {
                let mut logger = self.logger.lock().expect("observability logger poisoned");
                std::mem::replace(&mut *logger, LoggerHandle::ShuttingDown)
            };
            let mut shutdown_result = Ok(());
            self.complete_shutdown(|| match handle {
                LoggerHandle::Running(logger) => {
                    let (health, result) = match timeout {
                        Some(timeout) => logger.shutdown_with_timeout(timeout),
                        None => logger.shutdown(),
                    };
                    shutdown_result = result;
                    LoggerHandle::Stopped(health)
                }
                LoggerHandle::ShuttingDown => unreachable!("shutdown has one owner"),
                LoggerHandle::Stopped(health) => LoggerHandle::Stopped(health),
            });
            shutdown_result
        }

        pub(crate) fn complete_shutdown(&self, shutdown: impl FnOnce() -> LoggerHandle) {
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
                    #[cfg(test)]
                    if let Some(waiting) = &self.logger_waiting {
                        let _ = waiting.send("health");
                    }
                    logger = self
                        .logger_changed
                        .wait(logger)
                        .expect("observability logger poisoned");
                }
                match &*logger {
                    LoggerHandle::Running(logger) => Some(logger.health()),
                    LoggerHandle::ShuttingDown => unreachable!("waited for shutdown completion"),
                    LoggerHandle::Stopped(health) => Some(health.clone()),
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
        #[cfg(feature = "v1")]
        #[allow(
            deprecated,
            reason = "the released registrations remain isolated in the v1 facade"
        )]
        pub(crate) fn register_released_subscriber<T>(
            self,
            registration: LegacySubscriberRegistration<T>,
        ) -> Self
        where
            T: Observable,
        {
            let (subscriber, filter) = registration.into_parts();
            let mut builder = self;
            builder.subscribers.push(ErasedSubscriberRegistration {
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
            builder
        }

        /// Registers one canonical typed observation subscriber at construction time.
        ///
        /// # Panics
        ///
        /// Panics if internal type-erased routing calls this registration with
        /// the wrong observation payload type.
        pub fn register_subscriber<T>(
            mut self,
            registration: CanonicalSubscriberRegistration<T>,
        ) -> Self
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
        #[cfg(feature = "v1")]
        #[allow(
            deprecated,
            reason = "the released registrations remain isolated in the v1 facade"
        )]
        pub(crate) fn register_released_projection<T>(
            self,
            registration: LegacyProjectionRegistration<T>,
        ) -> Self
        where
            T: Observable,
        {
            // Released projectors route natively: converting their root span and
            // metric models to the canonical family would turn values it cannot
            // hold, such as scalar histograms, into routing failures.
            let (log_projector, span_projector, metric_projector, filter) =
                registration.into_parts();
            self.register_projection_routes(ProjectionRoutes {
                logs: log_projector.map(|projector| -> Arc<ProjectLogsFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_logs(observation)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                spans: span_projector.map(|projector| -> Arc<ProjectFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_spans(observation)
                            .map(drop)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                metrics: metric_projector.map(|projector| -> Arc<ProjectFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_metrics(observation)
                            .map(drop)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                filter,
            })
        }

        /// Registers one canonical typed observation projection set at construction time.
        pub fn register_projection<T>(
            self,
            registration: CanonicalProjectionRegistration<T>,
        ) -> Self
        where
            T: Observable,
        {
            let (log_projector, span_projector, metric_projector, filter) =
                registration.into_parts();
            self.register_projection_routes(ProjectionRoutes {
                logs: log_projector.map(|projector| -> Arc<ProjectLogsFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_logs(observation)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                spans: span_projector.map(|projector| -> Arc<ProjectFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_spans(observation)
                            .map(drop)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                metrics: metric_projector.map(|projector| -> Arc<ProjectFn<T>> {
                    Arc::new(move |observation| {
                        projector
                            .project_metrics(observation)
                            .map(drop)
                            .map_err(|err| DiagnosticSummary::from(err.diagnostic()))
                    })
                }),
                filter,
            })
        }

        /// Registers the type-erased dispatch shared by both projector families.
        fn register_projection_routes<T>(mut self, routes: ProjectionRoutes<T>) -> Self
        where
            T: Observable,
        {
            let ProjectionRoutes {
                logs,
                spans,
                metrics,
                filter,
            } = routes;

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

                    if let Some(project) = &logs {
                        match project(observation) {
                            Ok(events) => {
                                result.matched = true;
                                for event in events {
                                    if let Err(summary) = logger.log(event) {
                                        record_failure(summary);
                                    }
                                }
                                if let Err(err) = logger.flush() {
                                    record_failure(err.summary());
                                }
                            }
                            Err(summary) => record_failure(summary),
                        }
                    }

                    for project in [&spans, &metrics].into_iter().flatten() {
                        match project(observation) {
                            Ok(()) => result.matched = true,
                            Err(summary) => record_failure(summary),
                        }
                    }

                    result
                }),
            });
            self
        }

        /// Finalizes registration and constructs the routing runtime.
        pub fn build(self) -> Result<Observability, CanonicalInitError> {
            self.build_with(RuntimeAdmission::Canonical)
        }

        /// Finalizes registration for the released root facade.
        #[cfg(feature = "v1")]
        pub(crate) fn build_released(self) -> Result<Observability, CanonicalInitError> {
            self.build_with(RuntimeAdmission::Released)
        }

        fn build_with(self, mode: RuntimeAdmission) -> Result<Observability, CanonicalInitError> {
            if self.subscribers.is_empty() && self.projections.is_empty() {
                return Err(CanonicalInitError::Configuration {
                    context: Box::new(ErrorContext::new(
                        error_codes::OBSERVABILITY_INIT_FAILED,
                        "at least one subscriber or projector route must be registered",
                        Remediation::recoverable(
                            "register a subscriber or projector before building observability",
                            ["add at least one route for the observation types you emit"],
                        ),
                    )),
                });
            }
            let logger = Logger::new(self.config.logger_config()?)?;
            let logger = match mode {
                RuntimeAdmission::Canonical => RunningLogger::Canonical(logger),
                #[cfg(feature = "v1")]
                #[allow(
                    deprecated,
                    reason = "sc-observe 1.x released admission; isolated behind v1 in obs-f-8"
                )]
                RuntimeAdmission::Released => RunningLogger::Released(ReleasedLogger::from(logger)),
            };
            Ok(Observability {
                logger: Mutex::new(LoggerHandle::Running(logger)),
                logger_changed: Condvar::new(),
                #[cfg(test)]
                logger_waiting: None,
                shutdown: AtomicBool::new(false),
                subscriber_registrations: self.subscribers,
                projection_registrations: self.projections,
                observability_health_provider: self.observability_health_provider,
                runtime: RuntimeState::default(),
            })
        }
    }
} // mod canonical

/// Released 1.x root configuration facade.
#[cfg(feature = "v1")]
#[derive(Debug, Clone)]
#[deprecated(
    since = "1.4.0",
    note = "use sc_observe::v2::ObservabilityConfig; see docs/migration/phase-f.md"
)]
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

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "converts the retained released 1.x configuration"
)]
impl From<ObservabilityConfig> for v2::ObservabilityConfig {
    fn from(config: ObservabilityConfig) -> Self {
        Self {
            tool_name: config.tool_name,
            log_root: config.log_root,
            env_prefix: config.env_prefix,
            queue_capacity: config.queue_capacity,
            retained_log_policy: config.retained_log_policy,
        }
    }
}

/// Released 1.x construction-time registration facade.
#[cfg(feature = "v1")]
#[repr(transparent)]
#[deprecated(
    since = "1.4.0",
    note = "use sc_observe::v2::ObservabilityBuilder; see docs/migration/phase-f.md"
)]
pub struct ObservabilityBuilder(v2::ObservabilityBuilder);

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "this implementation owns the released root forwarding facade"
)]
impl std::fmt::Debug for ObservabilityBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObservabilityBuilder")
            .finish_non_exhaustive()
    }
}

/// Released 1.x root runtime facade.
#[cfg(feature = "v1")]
#[repr(transparent)]
#[deprecated(
    since = "1.4.0",
    note = "use sc_observe::v2::Observability; see docs/migration/phase-f.md"
)]
pub struct Observability(v2::Observability);

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "this implementation owns the released root forwarding facade"
)]
impl std::fmt::Debug for Observability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Observability")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "this implementation owns the released root forwarding facade"
)]
impl Observability {
    /// Starts a released construction-time builder.
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observe::v2::Observability::builder; see docs/migration/phase-f.md"
    )]
    pub fn builder(config: ObservabilityConfig) -> ObservabilityBuilder {
        ObservabilityBuilder(v2::Observability::builder(config.into()))
    }

    /// Routes one typed observation through the released facade.
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observe::v2::Observability::emit; see docs/migration/phase-f.md"
    )]
    pub fn emit<T>(&self, observation: Observation<T>) -> Result<(), ObservationError>
    where
        T: Observable,
    {
        self.0.emit(observation)
    }

    /// Returns the released facade's aggregate runtime health.
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observe::v2::Observability::health; see docs/migration/phase-f.md"
    )]
    pub fn health(&self) -> ObservabilityHealthReport {
        self.0.health()
    }
}

#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "this implementation owns the released root forwarding facade"
)]
impl ObservabilityBuilder {
    /// Attaches a health provider through the released builder facade.
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observe::v2::ObservabilityBuilder::with_observability_health_provider; see docs/migration/phase-f.md"
    )]
    pub fn with_observability_health_provider(
        self,
        provider: impl ObservabilityHealthProvider + 'static,
    ) -> Self {
        Self(self.0.with_observability_health_provider(provider))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::{LoggerHandle, RunningLogger, RuntimeState};
    #[cfg(feature = "v1")]
    use crate::canonical::{RunningFlushError, RuntimeAdmission};
    use crate::v2::{
        Observability as CanonicalObservability,
        ObservabilityConfig as CanonicalObservabilityConfig,
    };
    use sc_observability::v2::LogSink;
    use sc_observability::{LoggerConfig, SinkHealth, SinkHealthState, SinkRegistration};
    use sc_observability_types::v2::{
        AggregationTemporality, Attributes, FiniteF64, LogProjector, MetricProjector, MetricRecord,
        MetricValue, ObservationFilter, ObservationSubscriber, ProjectionError,
        ProjectionRegistration, SpanProjector, SpanRecord, SpanSignal, SubscriberError,
        SubscriberRegistration, TraceContext, TraceFlags,
    };
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricName, MetricUnit,
        ProcessIdentity, SpanId, SpanStarted, TargetCategory, TelemetryHealthReport,
        TelemetryHealthState, Timestamp, TraceContext as LegacyTraceContext, TraceId,
    };
    use serde_json::Map;
    use std::sync::mpsc;
    use std::time::Duration;

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
            Err(SubscriberError::Subscriber {
                context: Box::new(ErrorContext::new(
                    error_codes::OBSERVATION_ROUTING_FAILURE,
                    "subscriber failed",
                    Remediation::not_recoverable("test subscriber intentionally fails"),
                )),
            })
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
                v2_trace_context(),
                Attributes::new(),
            ))])
        }
    }

    struct RecordingMetricProjector {
        count: Arc<AtomicU64>,
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

    struct FailingProjector;

    impl LogProjector<AgentEvent> for FailingProjector {
        fn project_logs(
            &self,
            _observation: &Observation<AgentEvent>,
        ) -> Result<Vec<LogEvent>, ProjectionError> {
            Err(ProjectionError::Projection {
                context: Box::new(ErrorContext::new(
                    error_codes::OBSERVATION_ROUTING_FAILURE,
                    "projector failed",
                    Remediation::not_recoverable("test projector intentionally fails"),
                )),
            })
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

    fn v2_trace_context() -> TraceContext {
        TraceContext::new(
            TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            SpanId::new("0123456789abcdef").expect("valid span id"),
            TraceFlags::new(0),
        )
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
            trace: Some(LegacyTraceContext {
                trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
                span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
                parent_span_id: None,
            }),
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config =
            CanonicalObservabilityConfig::default_for(tool_name(), root.clone()).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let config = CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        let runtime = CanonicalObservability::builder(config)
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
        let mut config =
            CanonicalObservabilityConfig::default_for(tool_name(), root).expect("config");
        config.queue_capacity = 2048;

        let logger_config = config.logger_config().expect("logger config");

        assert_eq!(logger_config.queue_capacity, 2048);
    }

    #[test]
    fn canonical_flush_error_preserves_its_source_context() {
        let canonical = CanonicalFlushError::Drain {
            context: Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("SC_OBSERVE_TEST_DRAIN"),
                    "canonical drain failed",
                    Remediation::not_recoverable("inspect the drain failure"),
                )
                .source(Box::new(std::io::Error::other(
                    "canonical drain fixture source",
                ))),
            ),
        };

        assert_eq!(
            canonical.diagnostic().code.as_str(),
            "SC_OBSERVE_TEST_DRAIN"
        );
        assert!(
            std::error::Error::source(&canonical)
                .expect("canonical error retains its source")
                .to_string()
                .contains("canonical drain fixture source")
        );
    }

    #[cfg(feature = "v1")]
    #[test]
    #[allow(
        deprecated,
        reason = "released compatibility behavior is intentionally tested only with v1"
    )]
    fn released_root_init_error_preserves_canonical_context_and_source() {
        let init_error = ObservabilityConfig::default_for(
            ToolName::new(".").expect("fixture tool name is an identifier"),
            temp_path("root-init-error"),
        )
        .expect_err("invalid derived environment prefix must fail root initialization");
        assert_eq!(
            init_error.diagnostic().code,
            error_codes::OBSERVABILITY_INIT_FAILED
        );
        let context = std::error::Error::source(&init_error)
            .expect("released root init error retains canonical context");
        assert_eq!(
            std::error::Error::source(context)
                .map(ToString::to_string)
                .as_deref(),
            Some("env prefix must not end with underscore")
        );
    }

    struct BlockingFlushSink {
        flush_calls: AtomicU64,
        seed_completed: mpsc::Sender<()>,
        armed: Arc<AtomicBool>,
        entered: mpsc::Sender<()>,
        // MUTEX: LogSink is Sync; the sole writer owns receives on this
        // test-control channel. A timeout/disconnect releases failed tests.
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl LogSink for BlockingFlushSink {
        fn write(&self, _: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
            Ok(())
        }
        fn flush(&self) -> Result<(), sc_observability_types::v2::LogSinkError> {
            if self.armed.swap(false, Ordering::SeqCst) {
                let _ = self.entered.send(());
                let _ = self
                    .release
                    .lock()
                    .expect("release lock")
                    .recv_timeout(Duration::from_secs(5));
            }
            // A Flush command makes one sink pass. Signal after that pass so
            // the seed cannot consume the later shutdown arm.
            if self.flush_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                let _ = self.seed_completed.send(());
            }
            Err(sc_observability_types::v2::LogSinkError::Flush {
                context: Box::new(ErrorContext::new(
                    sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                    "controlled flush failure",
                    Remediation::not_recoverable("test fixture"),
                )),
            })
        }
        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: sink_name("controlled-flush"),
                state: SinkHealthState::DegradedDropping,
                last_error: None,
            }
        }
    }
    struct ShutdownFixture {
        runtime: Arc<CanonicalObservability>,
        before: sc_observability_types::LoggingHealthReport,
        entered_rx: mpsc::Receiver<()>,
        release_tx: mpsc::Sender<()>,
        waiting_rx: mpsc::Receiver<&'static str>,
    }

    fn shutdown_fixture() -> ShutdownFixture {
        let armed = Arc::new(AtomicBool::new(false));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (seed_completed, seed_rx) = mpsc::channel();
        let mut config = LoggerConfig::default_for(
            ServiceName::new("obs-app").expect("service"),
            temp_path("controlled-shutdown"),
        );
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::typed(Arc::new(BlockingFlushSink {
            flush_calls: AtomicU64::new(0),
            seed_completed,
            armed: armed.clone(),
            entered: entered_tx,
            release: Mutex::new(release_rx),
        })));
        let logger = builder.build().expect("built logger");
        logger.flush().expect_err("seed logging failure counter");
        seed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("seed flush completed before shutdown is armed");
        let before = logger.health();
        assert_eq!(before.flush_errors_total, 1);
        assert!(before.last_error.is_some());
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let runtime = Arc::new(CanonicalObservability {
            logger: Mutex::new(LoggerHandle::Running(RunningLogger::Canonical(logger))),
            logger_changed: Condvar::new(),
            #[cfg(test)]
            logger_waiting: Some(waiting_tx),
            shutdown: AtomicBool::new(false),
            subscriber_registrations: Vec::new(),
            projection_registrations: Vec::new(),
            observability_health_provider: None,
            runtime: RuntimeState::default(),
        });
        armed.store(true, Ordering::SeqCst);
        ShutdownFixture {
            runtime,
            before,
            entered_rx,
            release_tx,
            waiting_rx,
        }
    }

    fn assert_logger_waiters(waiting: &mpsc::Receiver<&'static str>) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let (mut flush, mut health) = (false, false);
        while !(flush && health) {
            match waiting
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("both callers reached the logger condition wait")
            {
                "flush" => flush = true,
                "health" => health = true,
                other => panic!("unexpected logger wait site: {other}"),
            }
        }
    }

    fn assert_retained_shutdown_health(
        report: ObservabilityHealthReport,
        before: &sc_observability_types::LoggingHealthReport,
    ) {
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
    }

    fn assert_immediate_repeated_shutdown(runtime: &Arc<CanonicalObservability>) {
        let (repeat_tx, repeat_rx) = mpsc::channel();
        let repeated_runtime = runtime.clone();
        let repeated = std::thread::spawn(move || {
            let _ = repeat_tx.send(repeated_runtime.shutdown());
        });
        repeat_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("repeated shutdown is immediate")
            .expect("success");
        repeated.join().expect("repeated shutdown thread");
    }

    #[test]
    fn in_flight_shutdown_preserves_flush_health_and_repeated_shutdown() {
        let ShutdownFixture {
            runtime,
            before,
            entered_rx,
            release_tx,
            waiting_rx,
        } = shutdown_fixture();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let shutdown_runtime = runtime.clone();
        let shutdown = std::thread::spawn(move || {
            let _ = shutdown_tx.send(shutdown_runtime.shutdown());
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
        assert_immediate_repeated_shutdown(&runtime);
        let (flush_tx, flush_rx) = mpsc::channel();
        let flush_runtime = runtime.clone();
        let flush = std::thread::spawn(move || {
            let _ = flush_tx.send(flush_runtime.flush_v2());
        });
        let (health_tx, health_rx) = mpsc::channel();
        let health_runtime = runtime.clone();
        let health = std::thread::spawn(move || {
            let _ = health_tx.send(health_runtime.health());
        });
        assert_logger_waiters(&waiting_rx);
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
        assert_retained_shutdown_health(report, &before);
        for thread in [shutdown, flush, health] {
            thread.join().expect("completed worker");
        }
    }

    #[test]
    fn shutdown_unwind_releases_condition_waiters() {
        struct ReleaseOnFailure(Arc<CanonicalObservability>);
        impl Drop for ReleaseOnFailure {
            fn drop(&mut self) {
                if !self.0.logger.is_poisoned() {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.0
                            .complete_shutdown(|| panic!("release failed unwind fixture"));
                    }));
                }
            }
        }
        use std::sync::mpsc;
        use std::time::Duration;
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let runtime = Arc::new(CanonicalObservability {
            logger: Mutex::new(LoggerHandle::ShuttingDown),
            logger_changed: Condvar::new(),
            #[cfg(test)]
            logger_waiting: Some(waiting_tx),
            shutdown: AtomicBool::new(true),
            subscriber_registrations: Vec::new(),
            projection_registrations: Vec::new(),
            observability_health_provider: None,
            runtime: RuntimeState::default(),
        });
        let _release_on_failure = ReleaseOnFailure(runtime.clone());
        let (done_tx, done_rx) = mpsc::channel();
        let threads: Vec<_> = [false, true]
            .into_iter()
            .map(|health| {
                let runtime = runtime.clone();
                let done_tx = done_tx.clone();
                std::thread::spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if health {
                            let _ = runtime.health();
                        } else {
                            let _ = runtime.flush_v2();
                        }
                    }));
                    let _ = done_tx.send(result.is_err());
                })
            })
            .collect();
        assert_logger_waiters(&waiting_rx);
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
        use sc_observability_types::v2::LogSinkError;

        struct FlushFailSink {
            flush_calls: Arc<AtomicU64>,
            flush_completed: std::sync::mpsc::Sender<()>,
        }

        impl LogSink for FlushFailSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }

            fn flush(&self) -> Result<(), LogSinkError> {
                let call = self.flush_calls.fetch_add(1, Ordering::SeqCst);
                let result = Err(LogSinkError::Flush {
                    context: Box::new(ErrorContext::new(
                        sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                        "flush failed",
                        Remediation::not_recoverable("test sink intentionally fails flush"),
                    )),
                });
                if call == 0 {
                    let _ = self.flush_completed.send(());
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
        let ok_config = CanonicalObservabilityConfig::default_for(tool_name(), ok_root.clone())
            .expect("config");
        let ok_runtime = CanonicalObservability::builder(ok_config)
            .register_subscriber(SubscriberRegistration::new(Arc::new(RecordingSubscriber {
                id: "flush-ok",
                calls: Arc::new(Mutex::new(Vec::new())),
            })))
            .build()
            .expect("runtime");
        assert!(ok_runtime.flush().is_ok());

        let build_failing_runtime = |name: &str| {
            let flush_calls = Arc::new(AtomicU64::new(0));
            let (flush_completed, flush_rx) = std::sync::mpsc::channel();
            let mut logger_config = LoggerConfig::default_for(
                ServiceName::new("obs-app").expect("service"),
                temp_path(name),
            );
            logger_config.enable_file_sink = false;
            logger_config.enable_console_sink = false;
            let mut builder =
                sc_observability::v2::Logger::builder(logger_config).expect("logger builder");
            builder.register_sink(SinkRegistration::typed(Arc::new(FlushFailSink {
                flush_calls: flush_calls.clone(),
                flush_completed,
            })));
            let logger = builder.build().expect("built logger");

            let runtime = CanonicalObservability {
                logger: Mutex::new(LoggerHandle::Running(RunningLogger::Canonical(logger))),
                logger_changed: Condvar::new(),
                #[cfg(test)]
                logger_waiting: None,
                shutdown: AtomicBool::new(false),
                subscriber_registrations: Vec::new(),
                projection_registrations: Vec::new(),
                observability_health_provider: None,
                runtime: RuntimeState::default(),
            };
            (runtime, flush_calls, flush_rx)
        };

        let (legacy_runtime, legacy_flush_calls, legacy_flush_rx) =
            build_failing_runtime("flush-legacy");
        let (typed_runtime, typed_flush_calls, typed_flush_rx) =
            build_failing_runtime("flush-typed");
        let Err(legacy_error) = legacy_runtime.flush_v2() else {
            panic!("legacy flush must report sink failure");
        };
        legacy_flush_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("bounded legacy flush completion");
        let Err(typed_error) = typed_runtime.flush_v2() else {
            panic!("typed flush must report sink failure");
        };
        assert!(matches!(&typed_error, CanonicalFlushError::Drain { .. }));
        assert_eq!(
            legacy_error.diagnostic().code,
            typed_error.diagnostic().code
        );
        typed_flush_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("bounded typed flush completion");
        assert_eq!(legacy_flush_calls.load(Ordering::SeqCst), 1);
        assert_eq!(typed_flush_calls.load(Ordering::SeqCst), 1);
        for runtime in [&legacy_runtime, &typed_runtime] {
            let logging = runtime.health().logging.expect("logging health");
            assert_eq!(logging.flush_errors_total, 1);
            assert!(logging.last_error.is_some());
        }
    }
    #[cfg(feature = "v1")]
    #[test]
    #[allow(
        deprecated,
        reason = "released compatibility behavior is intentionally tested only with v1"
    )]
    fn flush_failure_reports_the_same_diagnostic_per_facade() {
        use sc_observability_types::v2::LogSinkError;

        struct SourceFailSink;

        impl LogSink for SourceFailSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }

            fn flush(&self) -> Result<(), LogSinkError> {
                Err(LogSinkError::Flush {
                    context: Box::new(
                        ErrorContext::new(
                            sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                            "flush failed",
                            Remediation::not_recoverable("test sink intentionally fails flush"),
                        )
                        .source(Box::new(std::io::Error::other("native flush sink source"))),
                    ),
                })
            }

            fn health(&self) -> SinkHealth {
                SinkHealth {
                    name: sink_name("source-fail"),
                    state: SinkHealthState::DegradedDropping,
                    last_error: None,
                }
            }
        }

        fn failing_runtime(name: &str, mode: RuntimeAdmission) -> CanonicalObservability {
            let mut logger_config = LoggerConfig::default_for(
                ServiceName::new("obs-app").expect("service"),
                temp_path(name),
            );
            logger_config.enable_file_sink = false;
            logger_config.enable_console_sink = false;
            let mut builder =
                sc_observability::v2::Logger::builder(logger_config).expect("logger builder");
            builder.register_sink(SinkRegistration::typed(Arc::new(SourceFailSink)));
            let logger = builder.build().expect("built logger");
            let logger = match mode {
                RuntimeAdmission::Canonical => RunningLogger::Canonical(logger),
                RuntimeAdmission::Released => RunningLogger::Released(ReleasedLogger::from(logger)),
            };
            CanonicalObservability {
                logger: Mutex::new(LoggerHandle::Running(logger)),
                logger_changed: Condvar::new(),
                #[cfg(test)]
                logger_waiting: None,
                shutdown: AtomicBool::new(false),
                subscriber_registrations: Vec::new(),
                projection_registrations: Vec::new(),
                observability_health_provider: None,
                runtime: RuntimeState::default(),
            }
        }

        // Canonical (v2) facade: the canonical arm keeps the original context.
        let canonical = failing_runtime("flush-source-canonical", RuntimeAdmission::Canonical);
        let Err(error) = canonical.flush() else {
            panic!("canonical flush must report the sink failure");
        };
        assert!(matches!(error, CanonicalFlushError::Drain { .. }));
        let canonical_code = error.diagnostic().code.clone();
        assert_eq!(error.diagnostic().message, "writer flush failed");

        // Released (root) facade: the released arm keeps the FlushFailure context.
        let released = Observability(failing_runtime(
            "flush-source-released",
            RuntimeAdmission::Released,
        ));
        let Err(typed) = released.flush_typed() else {
            panic!("released typed flush must report the sink failure");
        };
        assert_eq!(typed.diagnostic().code, canonical_code);
        assert_eq!(typed.diagnostic().message, "writer flush failed");

        let released = Observability(failing_runtime(
            "flush-source-released-legacy",
            RuntimeAdmission::Released,
        ));
        let Err(legacy) = released.flush() else {
            panic!("released legacy flush must report the sink failure");
        };
        assert_eq!(legacy.diagnostic().code, canonical_code);
        assert_eq!(legacy.diagnostic().message, "writer flush failed");

        // A v2 flush over a released-pinned runtime still maps its own arm.
        let released = failing_runtime("flush-source-released-v2", RuntimeAdmission::Released);
        let Err(mapped) = released.flush() else {
            panic!("v2 flush must report the sink failure");
        };
        assert_eq!(mapped.diagnostic().message, "writer flush failed");
    }

    #[cfg(feature = "v1")]
    #[test]
    #[allow(deprecated)]
    fn running_flush_error_arms_preserve_context_and_source_identity() {
        fn context() -> Box<ErrorContext> {
            Box::new(
                ErrorContext::new(
                    sc_observability::error_codes::LOGGER_FLUSH_FAILED,
                    "arm fixture",
                    Remediation::not_recoverable("inspect the arm fixture"),
                )
                .source(Box::new(std::io::Error::other("native flush arm source"))),
            )
        }

        // The identity proof begins at the facade input (the flush error handed to
        // the facade conversion); it does not claim end-to-end sink-source identity.
        // Heap addresses of the boxed context and of its boxed source.
        fn identity(context: &ErrorContext) -> (*const (), *const ()) {
            let source = std::error::Error::source(context).expect("context keeps its source");
            (
                std::ptr::from_ref(context).cast::<()>(),
                std::ptr::from_ref(source).cast::<()>(),
            )
        }

        let canonical_error = || {
            let context = context();
            let before = identity(&context);
            (
                RunningFlushError::Canonical(CanonicalFlushError::Drain { context }),
                before,
            )
        };
        let released_error = || {
            let context = context();
            let before = identity(&context);
            (
                RunningFlushError::Released(FlushFailure::from_context(context)),
                before,
            )
        };

        let (error, before) = canonical_error();
        assert_eq!(error.summary().message, "arm fixture");
        assert_eq!(identity(&error.into_released().into_context()), before);

        let (error, before) = canonical_error();
        assert_eq!(identity(&error.into_canonical().into_context()), before);

        let (error, before) = released_error();
        assert_eq!(identity(&error.into_canonical().into_context()), before);

        let (error, before) = released_error();
        assert_eq!(identity(&error.into_released().into_context()), before);
    }
}
