//! Released 1.x typed-helper adapters.
//!
//! These inherent implementations preserve the released root method paths
//! while delegating directly to the canonical facade. They own no runtime,
//! routing, logger, span, or lifecycle state.

#![expect(
    deprecated,
    reason = "this module preserves released 1.x compatibility adapters"
)]

use std::path::PathBuf;

use sc_observability_types::InitError as LegacyInitError;
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
use sc_observability_types::{ServiceName, ToolName};

use crate::{Observability, ObservabilityBuilder, ObservabilityConfig, RunningFlushError};

fn legacy_init_error(error: InitError) -> LegacyInitError {
    LegacyInitError(error.into_context())
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
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityConfig::default_for(); see migrate-error-api.md."
    )]
    pub fn default_for(tool_name: ToolName, log_root: PathBuf) -> Result<Self, LegacyInitError> {
        Self::default_for_v2(tool_name, log_root).map_err(legacy_init_error)
    }

    /// Derives a service name while retaining the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityConfig::service_name(); see migrate-error-api.md."
    )]
    pub fn service_name(&self) -> Result<ServiceName, LegacyInitError> {
        self.service_name_v2().map_err(legacy_init_error)
    }

    /// Builds v1 defaults while retaining the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityConfig::default_for(); see migrate-error-api.md."
    )]
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitFailure> {
        Self::default_for_v2(tool_name, log_root).map_err(InitFailure::from)
    }

    /// Derives a service name while retaining the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityConfig::service_name(); see migrate-error-api.md."
    )]
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure> {
        self.service_name_v2().map_err(InitFailure::from)
    }
}

impl Observability {
    /// Constructs the shared runtime with the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::new(); see migrate-error-api.md."
    )]
    pub fn new(config: ObservabilityConfig) -> Result<Self, LegacyInitError> {
        Self::new_released(config).map_err(legacy_init_error)
    }

    /// Flushes the shared runtime with the released root failure contract.
    ///
    /// # Panics
    ///
    /// Panics if the runtime's internal logger-state mutex is poisoned, including
    /// while waiting for an in-progress shutdown to finish.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::flush(); see migrate-error-api.md."
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_running()
            .map_err(RunningFlushError::into_canonical)
    }

    /// Shuts down the shared runtime with the released root failure contract.
    ///
    /// # Panics
    ///
    /// Panics if the runtime's internal logger-state mutex is poisoned. It also
    /// resumes panics from logger shutdown, including panics from poisoned
    /// writer-snapshot or query-health mutexes.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::shutdown(); see migrate-error-api.md."
    )]
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_v2()
    }

    /// Constructs the existing runtime with the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::new(); see migrate-error-api.md."
    )]
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure> {
        Self::new_released(config).map_err(InitFailure::from)
    }

    /// Flushes the existing runtime with the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::flush(); see migrate-error-api.md."
    )]
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush_running()
            .map_err(RunningFlushError::into_released)
    }

    /// Shuts down the existing runtime with the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::Observability::shutdown(); see migrate-error-api.md."
    )]
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        self.shutdown_v2().map_err(ShutdownFailure::from)
    }
}

impl ObservabilityBuilder {
    /// Finalizes the shared builder with the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityBuilder::build(); see migrate-error-api.md."
    )]
    pub fn build(self) -> Result<Observability, LegacyInitError> {
        self.build_released().map_err(legacy_init_error)
    }

    /// Finalizes the existing builder with the released typed failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observe::v2::ObservabilityBuilder::build(); see migrate-error-api.md."
    )]
    pub fn build_typed(self) -> Result<Observability, InitFailure> {
        self.build_released().map_err(InitFailure::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::{DiagnosticInfo, ErrorCode, ErrorContext, Remediation};

    fn context(native_source: &'static str) -> Box<ErrorContext> {
        Box::new(
            ErrorContext::new(
                ErrorCode::new_static("SC_OBSERVE_ROOT_CONTEXT_FIXTURE"),
                "canonical root error fixture",
                Remediation::not_recoverable("inspect the root adapter fixture"),
            )
            .source(Box::new(std::io::Error::other(native_source))),
        )
    }

    fn assert_legacy_root_error(
        error: &(impl std::error::Error + DiagnosticInfo),
        native_source: &str,
    ) {
        assert_eq!(
            error.diagnostic().code.as_str(),
            "SC_OBSERVE_ROOT_CONTEXT_FIXTURE"
        );
        let context = std::error::Error::source(error)
            .expect("released root wrapper retains the canonical context");
        assert_eq!(
            std::error::Error::source(context)
                .map(ToString::to_string)
                .as_deref(),
            Some(native_source),
            "released root wrapper retains the native canonical source"
        );
    }

    #[test]
    fn legacy_init_error_preserves_canonical_context_and_source() {
        let init = legacy_init_error(InitError::Runtime {
            context: context("root init native source"),
        });
        assert_legacy_root_error(&init, "root init native source");
    }
}
