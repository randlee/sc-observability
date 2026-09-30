//! Released 1.x typed-helper adapters.
//!
//! These inherent implementations preserve the released root method paths
//! while delegating directly to the canonical facade. They own no runtime,
//! routing, logger, span, or lifecycle state.

use std::path::PathBuf;

use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
#[allow(
    deprecated,
    reason = "root methods retain the released compatibility error wrappers"
)]
use sc_observability_types::{
    FlushError as LegacyFlushError, InitError as LegacyInitError,
    ShutdownError as LegacyShutdownError,
};
use sc_observability_types::{ServiceName, ToolName};

use crate::{Observability, ObservabilityBuilder, ObservabilityConfig};

fn init_failure(error: InitError) -> InitFailure {
    InitFailure::from_context(error.into_context())
}

#[allow(
    deprecated,
    reason = "root methods retain the released compatibility error wrappers"
)]
fn legacy_init_error(error: InitError) -> LegacyInitError {
    LegacyInitError(error.into_context())
}

fn flush_failure(error: FlushError) -> FlushFailure {
    FlushFailure::from_context(error.into_context())
}

#[allow(
    deprecated,
    reason = "root methods retain the released compatibility error wrappers"
)]
fn legacy_flush_error(error: FlushError) -> LegacyFlushError {
    LegacyFlushError(error.into_context())
}

fn shutdown_failure(error: ShutdownError) -> ShutdownFailure {
    ShutdownFailure::from_context(error.into_context())
}

#[allow(
    deprecated,
    reason = "root methods retain the released compatibility error wrappers"
)]
fn legacy_shutdown_error(error: ShutdownError) -> LegacyShutdownError {
    LegacyShutdownError(error.into_context())
}

impl ObservabilityConfig {
    /// Builds v1 defaults while retaining the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn default_for(tool_name: ToolName, log_root: PathBuf) -> Result<Self, LegacyInitError> {
        Self::default_for_v2(tool_name, log_root).map_err(legacy_init_error)
    }

    /// Derives a service name while retaining the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn service_name(&self) -> Result<ServiceName, LegacyInitError> {
        self.service_name_v2().map_err(legacy_init_error)
    }

    /// Builds v1 defaults while retaining the released typed failure contract.
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitFailure> {
        Self::default_for_v2(tool_name, log_root).map_err(init_failure)
    }

    /// Derives a service name while retaining the released typed failure contract.
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure> {
        self.service_name_v2().map_err(init_failure)
    }
}

impl Observability {
    /// Constructs the shared runtime with the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn new(config: ObservabilityConfig) -> Result<Self, LegacyInitError> {
        Self::new_v2(config).map_err(legacy_init_error)
    }

    /// Flushes the shared runtime with the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn flush(&self) -> Result<(), LegacyFlushError> {
        self.flush_v2().map_err(legacy_flush_error)
    }

    /// Shuts down the shared runtime with the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn shutdown(&self) -> Result<(), LegacyShutdownError> {
        self.shutdown_v2().map_err(legacy_shutdown_error)
    }

    /// Constructs the existing runtime with the released typed failure contract.
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure> {
        Self::new_v2(config).map_err(init_failure)
    }

    /// Flushes the existing runtime with the released typed failure contract.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush_v2().map_err(flush_failure)
    }

    /// Shuts down the existing runtime with the released typed failure contract.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        self.shutdown_v2().map_err(shutdown_failure)
    }
}

impl ObservabilityBuilder {
    /// Finalizes the shared builder with the released root failure contract.
    #[allow(
        deprecated,
        reason = "root methods retain the released compatibility error wrappers"
    )]
    pub fn build(self) -> Result<Observability, LegacyInitError> {
        self.build_v2().map_err(legacy_init_error)
    }

    /// Finalizes the existing builder with the released typed failure contract.
    pub fn build_typed(self) -> Result<Observability, InitFailure> {
        self.build_v2().map_err(init_failure)
    }
}
