//! Released 1.x typed-helper adapters.
//!
//! These inherent implementations preserve the released root method paths
//! while delegating directly to the canonical facade. They own no runtime,
//! routing, logger, span, or lifecycle state.

use std::path::PathBuf;

use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
use sc_observability_types::{ServiceName, ToolName};

use crate::{Observability, ObservabilityBuilder, ObservabilityConfig};

fn init_failure(error: InitError) -> InitFailure {
    InitFailure::from_context(error.into_context())
}

fn flush_failure(error: FlushError) -> FlushFailure {
    FlushFailure::from_context(error.into_context())
}

fn shutdown_failure(error: ShutdownError) -> ShutdownFailure {
    ShutdownFailure::from_context(error.into_context())
}

impl ObservabilityConfig {
    /// Builds v1 defaults while retaining the released typed failure contract.
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitFailure> {
        Self::default_for(tool_name, log_root).map_err(init_failure)
    }

    /// Derives a service name while retaining the released typed failure contract.
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure> {
        self.service_name().map_err(init_failure)
    }
}

impl Observability {
    /// Constructs the existing runtime with the released typed failure contract.
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure> {
        Self::new(config).map_err(init_failure)
    }

    /// Flushes the existing runtime with the released typed failure contract.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush().map_err(flush_failure)
    }

    /// Shuts down the existing runtime with the released typed failure contract.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        self.shutdown().map_err(shutdown_failure)
    }
}

impl ObservabilityBuilder {
    /// Finalizes the existing builder with the released typed failure contract.
    pub fn build_typed(self) -> Result<Observability, InitFailure> {
        self.build().map_err(init_failure)
    }
}
