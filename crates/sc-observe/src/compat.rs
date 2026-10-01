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

use sc_observability::LogError;
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
use sc_observability_types::{DiagnosticInfo, DiagnosticSummary, LogEvent, ServiceName, ToolName};
use sc_observability_types::{
    FlushError as LegacyFlushError, InitError as LegacyInitError,
    ShutdownError as LegacyShutdownError,
};

use crate::{
    Observability, ObservabilityBuilder, ObservabilityConfig, RunningFlushError, RunningLogger,
};

fn legacy_init_error(error: InitError) -> LegacyInitError {
    LegacyInitError(error.into_context())
}

fn legacy_flush_error(error: FlushError) -> LegacyFlushError {
    LegacyFlushError(error.into_context())
}

fn legacy_running_flush_error(error: RunningFlushError) -> LegacyFlushError {
    legacy_flush_error(error.into_canonical())
}

fn released_log_error_summary(error: LogError) -> DiagnosticSummary {
    match error {
        LogError::InvalidEvent(error) => DiagnosticSummary::from(error.diagnostic()),
        LogError::WriterDegraded(error) | LogError::ShutdownTimedOut(error) => {
            DiagnosticSummary::from(error.diagnostic())
        }
    }
}

impl RunningLogger {
    pub(crate) fn log(&self, event: LogEvent) -> Result<(), DiagnosticSummary> {
        match self {
            Self::Canonical(logger) => logger
                .log(event)
                .map_err(|error| crate::canonical_log_error_summary(&error)),
            Self::Released(logger) => logger.log(event).map_err(released_log_error_summary),
        }
    }
}

fn legacy_shutdown_error(error: ShutdownError) -> LegacyShutdownError {
    LegacyShutdownError(error.into_context())
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
        note = "Use ObservabilityConfig::default_for_typed(); see migrate-error-api.md."
    )]
    pub fn default_for(tool_name: ToolName, log_root: PathBuf) -> Result<Self, LegacyInitError> {
        Self::default_for_v2(tool_name, log_root).map_err(legacy_init_error)
    }

    /// Derives a service name while retaining the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use ObservabilityConfig::service_name_typed(); see migrate-error-api.md."
    )]
    pub fn service_name(&self) -> Result<ServiceName, LegacyInitError> {
        self.service_name_v2().map_err(legacy_init_error)
    }

    /// Builds v1 defaults while retaining the released typed failure contract.
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf) -> Result<Self, InitFailure> {
        Self::default_for_v2(tool_name, log_root).map_err(InitFailure::from)
    }

    /// Derives a service name while retaining the released typed failure contract.
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure> {
        self.service_name_v2().map_err(InitFailure::from)
    }
}

impl Observability {
    /// Constructs the shared runtime with the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use Observability::new_typed(); see migrate-error-api.md."
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
        note = "Use Observability::flush_typed(); see migrate-error-api.md."
    )]
    pub fn flush(&self) -> Result<(), LegacyFlushError> {
        self.flush_running().map_err(legacy_running_flush_error)
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
        note = "Use Observability::shutdown_typed(); see migrate-error-api.md."
    )]
    pub fn shutdown(&self) -> Result<(), LegacyShutdownError> {
        self.shutdown_v2().map_err(legacy_shutdown_error)
    }

    /// Constructs the existing runtime with the released typed failure contract.
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure> {
        Self::new_released(config).map_err(InitFailure::from)
    }

    /// Flushes the existing runtime with the released typed failure contract.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush_running()
            .map_err(RunningFlushError::into_released)
    }

    /// Shuts down the existing runtime with the released typed failure contract.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        self.shutdown_v2().map_err(ShutdownFailure::from)
    }
}

impl ObservabilityBuilder {
    /// Finalizes the shared builder with the released root failure contract.
    #[deprecated(
        since = "1.4.0",
        note = "Use ObservabilityBuilder::build_typed(); see migrate-error-api.md."
    )]
    pub fn build(self) -> Result<Observability, LegacyInitError> {
        self.build_released().map_err(legacy_init_error)
    }

    /// Finalizes the existing builder with the released typed failure contract.
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
    fn direct_root_error_adapters_preserve_canonical_context_and_source() {
        let init = legacy_init_error(InitError::Runtime {
            context: context("root init native source"),
        });
        assert_legacy_root_error(&init, "root init native source");

        let flush = legacy_flush_error(FlushError::Drain {
            context: context("root flush native source"),
        });
        assert_legacy_root_error(&flush, "root flush native source");

        let shutdown = legacy_shutdown_error(ShutdownError::Drain {
            context: context("root shutdown native source"),
        });
        assert_legacy_root_error(&shutdown, "root shutdown native source");
    }
    #[test]
    fn legacy_running_flush_error_preserves_context_and_source_identity() {
        // The proof begins at the facade input; it does not claim end-to-end
        // sink-source identity.
        fn identity(context: &ErrorContext) -> (*const (), *const ()) {
            let source = std::error::Error::source(context).expect("context keeps its source");
            (
                std::ptr::from_ref(context).cast::<()>(),
                std::ptr::from_ref(source).cast::<()>(),
            )
        }

        let boxed = context("root flush arm native source");
        let before = identity(&boxed);
        let legacy = legacy_running_flush_error(RunningFlushError::Released(
            FlushFailure::from_context(boxed),
        ));
        assert_eq!(identity(&legacy.0), before);

        let boxed = context("root canonical arm native source");
        let before = identity(&boxed);
        let legacy = legacy_running_flush_error(RunningFlushError::Canonical(FlushError::Drain {
            context: boxed,
        }));
        assert_eq!(identity(&legacy.0), before);
    }
}
