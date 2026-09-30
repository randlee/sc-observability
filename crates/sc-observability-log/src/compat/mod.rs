//! Released 1.x bridge facade over the canonical v2 implementation.
//!
//! These adapters preserve the error variants and root paths shipped before
//! the canonical operation errors were introduced. Runtime ownership remains
//! in the canonical bridge; adapters only translate its typed outcomes.

use std::ops::Deref;
use std::path::PathBuf;
use std::time::Duration;

use sc_observability_types::{ErrorCode, LevelFilter, Remediation};
use serde::{Deserialize, Serialize};

use crate::{control, error_codes};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum InitError {
    #[error("sc-observability-log is already initialized in this process")]
    AlreadyInitialized,
    #[error("another log::Log implementation is already installed")]
    ForeignLoggerInstalled,
    #[error("configured level {configured:?} exceeds available static level {available:?}")]
    UnsupportedLevel {
        configured: LevelFilter,
        available: LevelFilter,
    },
    #[error("process identity resolution failed: {diagnostic}")]
    IdentityResolution {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("sc-observability logger construction failed: {diagnostic}")]
    Logger {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("could not start bridge lifecycle coordination: {diagnostic}")]
    RuntimeStart {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FlushError {
    #[error("flush did not complete within {timeout:?}")]
    TimedOut { timeout: Duration },
    #[error("sc-observability flush failed: {diagnostic}")]
    Logger {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("could not start the flush helper thread: {diagnostic}")]
    HelperSpawn {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("the flush helper thread ended without a result: {diagnostic}")]
    HelperLost {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("a previous flush is still running; no new flush was started")]
    InProgress,
    #[error("the logger is not running: {phase:?}")]
    NotRunning { phase: crate::LifecyclePhase },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShutdownError {
    #[error("shutdown did not complete within {timeout:?}")]
    TimedOut { timeout: Duration },
    #[error("final flush failed; the logger was still shut down: {diagnostic}")]
    FinalFlush {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("could not start the shutdown helper thread: {diagnostic}")]
    HelperSpawn {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("the shutdown helper thread ended without a result: {diagnostic}")]
    HelperLost {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EmitError {
    #[error("invalid field {raw_key:?}: {reason}")]
    InvalidField {
        raw_key: String,
        reason: crate::FieldKeyError,
    },
    #[error("invalid event: {diagnostic}")]
    InvalidEvent {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("writer queue is full: {diagnostic}")]
    QueueFull {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("writer is degraded: {diagnostic}")]
    WriterDegraded {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("logger shutdown timed out: {diagnostic}")]
    ShutdownTimedOut {
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    #[error("logger is not running: {phase:?}")]
    NotRunning { phase: crate::LifecyclePhase },
    #[error("reentrant emission")]
    Reentrant,
    #[error("logger callback panicked")]
    Panicked,
}

impl InitError {
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::AlreadyInitialized => error_codes::SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED,
            Self::ForeignLoggerInstalled => {
                error_codes::SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED
            }
            Self::UnsupportedLevel { .. } => error_codes::SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL,
            Self::IdentityResolution { diagnostic } | Self::Logger { diagnostic } => {
                diagnostic.code.clone()
            }
            Self::RuntimeStart { .. } => error_codes::SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED,
        }
    }

    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::AlreadyInitialized => Remediation::not_recoverable(
                "the log facade logger cannot be replaced; keep the first LogGuard",
            ),
            Self::ForeignLoggerInstalled => {
                Remediation::not_recoverable("choose one application logger before startup")
            }
            Self::UnsupportedLevel { .. } => Remediation::not_recoverable(
                "rebuild without the static cap or choose a supported startup baseline",
            ),
            Self::IdentityResolution { diagnostic } | Self::Logger { diagnostic } => {
                diagnostic.remediation.clone()
            }
            Self::RuntimeStart { .. } => Remediation::recoverable(
                "inspect thread and resource availability, then retry initialization explicitly",
                std::iter::empty::<String>(),
            ),
        }
    }
}

impl FlushError {
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::TimedOut { .. } => error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT,
            Self::Logger { diagnostic } => diagnostic.code.clone(),
            Self::HelperSpawn { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            Self::HelperLost { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
            Self::NotRunning { .. } => error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
            Self::InProgress => error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS,
        }
    }

    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::TimedOut { .. } => Remediation::recoverable(
                "retry the flush later or raise the timeout",
                ["a shutdown before the detached flush returns reports ShutdownError::TimedOut"],
            ),
            Self::Logger { diagnostic } => diagnostic.remediation.clone(),
            Self::HelperSpawn { .. } => Remediation::not_recoverable(
                "inspect resource availability and retained logger health; completion is unconfirmed",
            ),
            Self::HelperLost { .. } => Remediation::not_recoverable(
                "inspect retained lifecycle and health; do not claim worker completion",
            ),
            Self::NotRunning { .. } => Remediation::not_recoverable(
                "the lifecycle owner has shut the logger down; the final shutdown flushed what was queued",
            ),
            Self::InProgress => Remediation::recoverable(
                "wait for the previous flush to finish, then retry",
                ["wait for the in-flight flush before making an explicit new request"],
            ),
        }
    }
}

impl ShutdownError {
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::TimedOut { .. } => error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            Self::FinalFlush { diagnostic } => diagnostic.code.clone(),
            Self::HelperSpawn { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            Self::HelperLost { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
        }
    }

    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::TimedOut { .. } => Remediation::recoverable(
                "use control.wait_stopped to observe the original shutdown",
                std::iter::empty::<String>(),
            ),
            Self::FinalFlush { diagnostic } => diagnostic.remediation.clone(),
            Self::HelperSpawn { .. } => Remediation::not_recoverable(
                "inspect resource availability and logger health; completion is unconfirmed",
            ),
            Self::HelperLost { .. } => Remediation::not_recoverable(
                "inspect saved lifecycle and health; do not claim worker completion",
            ),
        }
    }
}

impl EmitError {
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidField { .. } => error_codes::SC_OBSERVABILITY_LOG_INVALID_FIELD,
            Self::InvalidEvent { diagnostic }
            | Self::QueueFull { diagnostic }
            | Self::WriterDegraded { diagnostic }
            | Self::ShutdownTimedOut { diagnostic } => diagnostic.code.clone(),
            Self::NotRunning { .. } => error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
            Self::Reentrant => error_codes::SC_OBSERVABILITY_LOG_REENTRANT_EMIT,
            Self::Panicked => error_codes::SC_OBSERVABILITY_LOG_LOGGER_PANICKED,
        }
    }

    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::InvalidEvent { diagnostic }
            | Self::QueueFull { diagnostic }
            | Self::WriterDegraded { diagnostic }
            | Self::ShutdownTimedOut { diagnostic } => diagnostic.remediation.clone(),
            Self::InvalidField { .. } => {
                Remediation::recoverable("correct the field key", ["resubmit the event"])
            }
            Self::NotRunning { .. } => {
                Remediation::not_recoverable("the owner has stopped the bridge")
            }
            Self::Reentrant => Remediation::not_recoverable(
                "remove logging from formatter, redactor, or diagnostic callbacks",
            ),
            Self::Panicked => Remediation::not_recoverable(
                "repair the panicking callback; do not retry implicitly",
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogControl(control::LogControl);

impl LogControl {
    pub(crate) const fn new() -> Self {
        Self(control::LogControl::new())
    }

    /// Converts this released 1.x control into its canonical v2 facade.
    ///
    /// Both facades retain the same weak attachment reference and process
    /// runtime; this conversion does not create another owner or lifecycle.
    #[must_use]
    pub fn into_v2(self) -> crate::v2::LogControl {
        self.0
    }

    /// Requests a bounded flush using the released root error variants.
    ///
    /// # Errors
    ///
    /// Returns the legacy timeout, writer, helper, in-progress, or stopped variant.
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        self.0
            .flush(timeout)
            .map_err(|error| legacy_flush(&error, timeout))
    }

    /// Reads the retained bridge health snapshot.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::Unavailable` when no report is retained.
    pub fn health(&self) -> Result<crate::BridgeHealthReport, crate::ControlError> {
        self.0.health()
    }

    /// Returns the path captured during logger initialization.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::Unavailable` when no path snapshot is available.
    pub fn active_log_path(&self) -> Result<Option<PathBuf>, crate::ControlError> {
        self.0.active_log_path()
    }

    /// Snapshots exact-once dropped-event counters.
    #[must_use]
    pub fn dropped_events(&self) -> crate::DroppedEvents {
        self.0.dropped_events()
    }

    /// Waits for completion of the already-started owner shutdown.
    ///
    /// # Errors
    ///
    /// Returns `WaitError` when shutdown has not started, cannot be observed, or times out.
    pub fn wait_stopped(
        &self,
        timeout: Duration,
    ) -> Result<crate::ShutdownReport, crate::WaitError> {
        self.0.wait_stopped(timeout)
    }

    /// Submits a typed event through the process-wide bridge.
    ///
    /// # Errors
    ///
    /// Returns one of the released typed admission errors after exact-once accounting.
    pub fn try_log(&self, event: crate::BridgeEvent) -> Result<crate::EmitOutcome, EmitError> {
        self.0.try_log(event).map_err(legacy_emit)
    }

    /// Executes a typed core query.
    ///
    /// # Errors
    ///
    /// Returns `ControlError` when the lifecycle or query rejects the request.
    pub fn query(
        &self,
        query: &sc_observability_types::LogQuery,
    ) -> Result<sc_observability_types::LogSnapshot, crate::ControlError> {
        self.0.query(query)
    }
}

pub(crate) fn legacy_emit(error: crate::error::EmitError) -> EmitError {
    use crate::error::EmitError as Core;
    match error {
        Core::InvalidField { raw_key, reason } => EmitError::InvalidField { raw_key, reason },
        Core::InvalidEvent { diagnostic } => EmitError::InvalidEvent { diagnostic },
        Core::QueueFull { diagnostic } => EmitError::QueueFull { diagnostic },
        Core::WriterDegraded { diagnostic } => EmitError::WriterDegraded { diagnostic },
        Core::ShutdownTimedOut { diagnostic } => EmitError::ShutdownTimedOut { diagnostic },
        Core::NotRunning { phase } => EmitError::NotRunning { phase },
        Core::NotInstalled => EmitError::NotRunning {
            phase: crate::LifecyclePhase::Stopped,
        },
        Core::Reentrant => EmitError::Reentrant,
        Core::Panicked => EmitError::Panicked,
    }
}

impl Deref for LogControl {
    type Target = control::LogControl;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn legacy_init(
    error: sc_observability_types::v2::InitError,
    configured: LevelFilter,
    available: LevelFilter,
) -> InitError {
    use sc_observability_types::v2::InitError as Core;
    match error {
        Core::Configuration { context } => match context.diagnostic().code.as_str() {
            "SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED" => InitError::AlreadyInitialized,
            "SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED" => InitError::ForeignLoggerInstalled,
            "SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL" => InitError::UnsupportedLevel {
                configured,
                available,
            },
            _ => InitError::RuntimeStart {
                diagnostic: operation_diagnostic(context.diagnostic()),
            },
        },
        Core::Runtime { context } => match context.diagnostic().code.as_str() {
            "SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED" => InitError::IdentityResolution {
                diagnostic: operation_diagnostic(context.diagnostic()),
            },
            _ => InitError::Logger {
                diagnostic: operation_diagnostic(context.diagnostic()),
            },
        },
        _ => InitError::RuntimeStart {
            diagnostic: operation_diagnostic(error.diagnostic()),
        },
    }
}

pub(crate) fn legacy_flush(
    error: &sc_observability_types::v2::FlushError,
    timeout: Duration,
) -> FlushError {
    use sc_observability_types::v2::FlushError as Core;
    let diagnostic = operation_diagnostic(error.diagnostic());
    match error {
        Core::Drain { .. }
            if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT =>
        {
            FlushError::TimedOut { timeout }
        }
        Core::Drain { .. }
            if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED =>
        {
            FlushError::HelperSpawn { diagnostic }
        }
        Core::Drain { .. } if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST => {
            FlushError::HelperLost { diagnostic }
        }
        Core::Drain { .. }
            if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS =>
        {
            FlushError::InProgress
        }
        Core::Drain { .. }
            if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
                || diagnostic.code == error_codes::SC_LOG_DETACH_NOT_INSTALLED =>
        {
            FlushError::NotRunning {
                phase: crate::LifecyclePhase::Stopped,
            }
        }
        _ => FlushError::Logger { diagnostic },
    }
}

pub(crate) fn legacy_shutdown(
    error: sc_observability_types::v2::ShutdownError,
    timeout: Duration,
) -> ShutdownError {
    use sc_observability_types::v2::ShutdownError as Core;
    match error {
        Core::Timeout { .. } => ShutdownError::TimedOut { timeout },
        Core::Drain { context } => {
            let diagnostic = operation_diagnostic(context.diagnostic());
            if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED {
                ShutdownError::HelperSpawn { diagnostic }
            } else if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST {
                ShutdownError::HelperLost { diagnostic }
            } else {
                ShutdownError::FinalFlush { diagnostic }
            }
        }
        _ => ShutdownError::FinalFlush {
            diagnostic: operation_diagnostic(error.diagnostic()),
        },
    }
}

fn operation_diagnostic(
    value: &sc_observability_types::Diagnostic,
) -> sc_observability_types::OperationDiagnostic {
    sc_observability_types::OperationDiagnostic {
        code: value.code.clone(),
        message: value.message.clone(),
        remediation: value.remediation.clone(),
        at: value.timestamp,
    }
}
