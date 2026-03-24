//! Typed observation routing layered on top of `sc-observability`.
//!
//! This crate owns construction-time subscriber/projector registration,
//! per-type routing, and top-level observability health aggregation while
//! remaining independent of OTLP transport details.

pub mod constants;
pub mod error_codes;

use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability::RotationPolicy;
use sc_observability_types::{
    EnvPrefix, ErrorContext, FlushError, InitError, ObservabilityHealthReport, Observable,
    Observation, ObservationError, ProjectionRegistration, Remediation, ServiceName, ShutdownError,
    SubscriberRegistration, ToolName,
};

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub tool_name: ToolName,
    pub log_root: PathBuf,
    pub env_prefix: EnvPrefix,
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
}

impl ObservabilityConfig {
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
}

pub struct Observability {
    shutdown: AtomicBool,
    _config: ObservabilityConfig,
}

pub struct ObservabilityBuilder {
    config: ObservabilityConfig,
}

impl Observability {
    pub fn new(config: ObservabilityConfig) -> Result<Self, InitError> {
        Ok(Self {
            shutdown: AtomicBool::new(false),
            _config: config,
        })
    }

    pub fn builder(config: ObservabilityConfig) -> ObservabilityBuilder {
        ObservabilityBuilder { config }
    }

    pub fn emit<T>(&self, _observation: Observation<T>) -> Result<(), ObservationError>
    where
        T: Observable,
    {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(ObservationError::Shutdown);
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<(), FlushError> {
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn health(&self) -> ObservabilityHealthReport {
        ObservabilityHealthReport {
            state: sc_observability_types::ObservationHealthState::Healthy,
            dropped_observations_total: 0,
            subscriber_failures_total: 0,
            projection_failures_total: 0,
            logging: None,
            telemetry: None,
            last_error: None,
        }
    }
}

impl ObservabilityBuilder {
    pub fn register_subscriber<T>(self, _registration: SubscriberRegistration<T>) -> Self
    where
        T: Observable,
    {
        self
    }

    pub fn register_projection<T>(self, _registration: ProjectionRegistration<T>) -> Self
    where
        T: Observable,
    {
        self
    }

    pub fn build(self) -> Result<Observability, InitError> {
        Observability::new(self.config)
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

struct _TypeWitness<T>(PhantomData<T>);
