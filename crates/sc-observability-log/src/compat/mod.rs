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
/// Failure while initializing the released 1.x bridge facade.
pub enum InitError {
    /// The process-wide bridge was already initialized.
    #[error("sc-observability-log is already initialized in this process")]
    AlreadyInitialized,
    /// Another logger already owns the `log` facade.
    #[error("another log::Log implementation is already installed")]
    ForeignLoggerInstalled,
    /// The requested level exceeds the level compiled into this executable.
    #[error("configured level {configured:?} exceeds available static level {available:?}")]
    UnsupportedLevel {
        /// Requested runtime level.
        configured: LevelFilter,
        /// Most verbose level available in this build.
        available: LevelFilter,
    },
    /// Process identity could not be resolved.
    #[error("process identity resolution failed: {diagnostic}")]
    IdentityResolution {
        /// Details about the identity-resolution failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The underlying logger could not be constructed.
    #[error("sc-observability logger construction failed: {diagnostic}")]
    Logger {
        /// Details about the logger-construction failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// Required bridge lifecycle coordination could not be started.
    #[error("could not start bridge lifecycle coordination: {diagnostic}")]
    RuntimeStart {
        /// Details about the coordination startup failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
/// Failure while flushing the released 1.x bridge facade.
pub enum FlushError {
    /// The flush did not finish before the supplied deadline.
    #[error("flush did not complete within {timeout:?}")]
    TimedOut {
        /// Deadline that elapsed while waiting for the flush.
        timeout: Duration,
    },
    /// The underlying logger rejected or failed the flush.
    #[error("sc-observability flush failed: {diagnostic}")]
    Logger {
        /// Details about the logger flush failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// A helper thread could not be started for the bounded flush.
    #[error("could not start the flush helper thread: {diagnostic}")]
    HelperSpawn {
        /// Details about the helper startup failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The flush helper ended without publishing a result.
    #[error("the flush helper thread ended without a result: {diagnostic}")]
    HelperLost {
        /// Details about the missing helper result.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// A previous flush remains in flight, so no new flush was started.
    #[error("a previous flush is still running; no new flush was started")]
    InProgress,
    /// The bridge no longer accepts flush requests.
    #[error("the logger is not running: {phase:?}")]
    NotRunning {
        /// Lifecycle phase observed when the request was rejected.
        phase: crate::LifecyclePhase,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
/// Failure while shutting down the released 1.x bridge facade.
pub enum ShutdownError {
    /// Shutdown did not finish before the supplied deadline.
    #[error("shutdown did not complete within {timeout:?}")]
    TimedOut {
        /// Deadline that elapsed while waiting for shutdown.
        timeout: Duration,
    },
    /// Final flush failed, although shutdown still completed.
    #[error("final flush failed; the logger was still shut down: {diagnostic}")]
    FinalFlush {
        /// Details about the final flush failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// A helper thread could not be started for shutdown.
    #[error("could not start the shutdown helper thread: {diagnostic}")]
    HelperSpawn {
        /// Details about the helper startup failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The shutdown helper ended without publishing a result.
    #[error("the shutdown helper thread ended without a result: {diagnostic}")]
    HelperLost {
        /// Details about the missing helper result.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
/// Failure while admitting an event through the released 1.x bridge facade.
pub enum EmitError {
    /// A producer field key could not be represented safely.
    #[error("invalid field {raw_key:?}: {reason}")]
    InvalidField {
        /// Original producer-supplied field key.
        raw_key: String,
        /// Reason the field key was rejected.
        reason: crate::FieldKeyError,
    },
    /// The assembled event failed validation.
    #[error("invalid event: {diagnostic}")]
    InvalidEvent {
        /// Details about the invalid event.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The writer queue had no capacity for this event.
    #[error("writer queue is full: {diagnostic}")]
    QueueFull {
        /// Details about queue saturation.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The writer could not accept more events.
    #[error("writer is degraded: {diagnostic}")]
    WriterDegraded {
        /// Details about the writer failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// Logger shutdown exceeded its deadline during admission.
    #[error("logger shutdown timed out: {diagnostic}")]
    ShutdownTimedOut {
        /// Details about the shutdown timeout.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The bridge no longer accepts events.
    #[error("logger is not running: {phase:?}")]
    NotRunning {
        /// Lifecycle phase observed when admission was rejected.
        phase: crate::LifecyclePhase,
    },
    /// Emission re-entered the guarded logging path.
    #[error("reentrant emission")]
    Reentrant,
    /// A logger callback panicked and was contained.
    #[error("logger callback panicked")]
    Panicked,
}

impl InitError {
    /// Returns the stable code associated with this initialization failure.
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

    /// Returns recovery guidance for this initialization failure.
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
    /// Returns the stable code associated with this flush failure.
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

    /// Returns recovery guidance for this flush failure.
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
    /// Returns the stable code associated with this shutdown failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::TimedOut { .. } => error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            Self::FinalFlush { diagnostic } => diagnostic.code.clone(),
            Self::HelperSpawn { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            Self::HelperLost { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
        }
    }

    /// Returns recovery guidance for this shutdown failure.
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
    /// Returns the stable code associated with this admission failure.
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

    /// Returns recovery guidance for this admission failure.
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
/// Cloneable, non-owning control for the installed released 1.x bridge.
pub struct LogControl {
    _private: (),
}

/// The canonical control every released root control routes through; root
/// controls never carry an attachment, so this one global is identical to
/// each value the root facade used to hold.
static ROOT_CONTROL: control::LogControl = control::LogControl::new();

impl LogControl {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }

    /// Converts this released 1.x control into its canonical v2 facade.
    ///
    /// Returns a new unattached canonical control over the same process-wide
    /// bridge; this conversion does not create another owner or lifecycle.
    #[must_use]
    pub fn into_v2(self) -> crate::v2::LogControl {
        control::LogControl::new()
    }

    /// Requests a bounded flush using the released root error variants.
    ///
    /// # Errors
    ///
    /// Returns the legacy timeout, writer, helper, in-progress, or stopped variant.
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        ROOT_CONTROL
            .flush(timeout)
            .map_err(|error| legacy_flush(&error, timeout))
    }

    /// Reads the retained bridge health snapshot.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::Unavailable` when no report is retained.
    pub fn health(&self) -> Result<crate::BridgeHealthReport, crate::ControlError> {
        ROOT_CONTROL.health()
    }

    /// Returns the path captured during logger initialization.
    ///
    /// # Errors
    ///
    /// Returns `ControlError::Unavailable` when no path snapshot is available.
    pub fn active_log_path(&self) -> Result<Option<PathBuf>, crate::ControlError> {
        ROOT_CONTROL.active_log_path()
    }

    /// Snapshots exact-once dropped-event counters.
    #[must_use]
    pub fn dropped_events(&self) -> crate::DroppedEvents {
        ROOT_CONTROL.dropped_events()
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
        ROOT_CONTROL.wait_stopped(timeout)
    }

    /// Submits a typed event through the process-wide bridge.
    ///
    /// # Errors
    ///
    /// Returns one of the released typed admission errors after exact-once accounting.
    pub fn try_log(&self, event: crate::BridgeEvent) -> Result<crate::EmitOutcome, EmitError> {
        ROOT_CONTROL.try_log(event).map_err(legacy_emit)
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
        ROOT_CONTROL.query(query)
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
        &ROOT_CONTROL
    }
}

pub(crate) fn legacy_init(
    error: sc_observability_types::v2::InitError,
    configured: LevelFilter,
    available: LevelFilter,
) -> InitError {
    use sc_observability_types::v2::InitError as Core;
    match error {
        Core::Configuration { context } => {
            let code = &context.diagnostic().code;
            if *code == error_codes::SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED {
                InitError::AlreadyInitialized
            } else if *code == error_codes::SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED {
                InitError::ForeignLoggerInstalled
            } else if *code == error_codes::SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL {
                InitError::UnsupportedLevel {
                    configured,
                    available,
                }
            } else {
                InitError::Logger {
                    diagnostic: operation_diagnostic(context.diagnostic()),
                }
            }
        }
        Core::Runtime { context } => {
            let code = &context.diagnostic().code;
            if *code == error_codes::SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED {
                InitError::RuntimeStart {
                    diagnostic: operation_diagnostic(context.diagnostic()),
                }
            } else if *code == error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED {
                InitError::IdentityResolution {
                    diagnostic: operation_diagnostic(context.diagnostic()),
                }
            } else {
                InitError::Logger {
                    diagnostic: operation_diagnostic(context.diagnostic()),
                }
            }
        }
        _ => InitError::Logger {
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
                phase: crate::handle::lifecycle_phase(),
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    fn context(
        code: &'static str,
        remediation: Remediation,
    ) -> sc_observability_types::ErrorContext {
        sc_observability_types::ErrorContext::new(
            ErrorCode::new_static(code),
            format!("{code} fixture"),
            remediation,
        )
        .source(Box::new(std::io::Error::other("canonical source")))
    }

    fn diagnostic(
        code: &'static str,
        remediation: Remediation,
    ) -> sc_observability_types::OperationDiagnostic {
        let context = context(code, remediation);
        operation_diagnostic(context.diagnostic())
    }

    macro_rules! assert_root_contract {
        ($error:expr, $code:expr, $remediation:expr) => {{
            let error = &$error;
            assert_eq!(error.code().as_str(), $code);
            assert_eq!(error.remediation(), $remediation);
            assert!(
                Error::source(error).is_none(),
                "released errors expose diagnostics rather than a new source chain"
            );
        }};
    }

    #[test]
    fn released_log_control_keeps_released_auto_traits() {
        fn assert_traits<
            T: Send + Sync + Unpin + std::panic::UnwindSafe + std::panic::RefUnwindSafe,
        >() {
        }
        assert_traits::<LogControl>();
    }

    #[test]
    fn legacy_init_preserves_runtime_start_variant_and_diagnostic() {
        let context = sc_observability_types::ErrorContext::new(
            error_codes::SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED,
            "coordinator could not start",
            Remediation::recoverable("restore thread resources", ["retry initialization"]),
        );
        let expected = operation_diagnostic(context.diagnostic());
        let error = legacy_init(
            sc_observability_types::v2::InitError::Runtime {
                context: Box::new(context),
            },
            LevelFilter::Info,
            LevelFilter::Trace,
        );
        let InitError::RuntimeStart { diagnostic } = error else {
            panic!("released coordinator failures retain RuntimeStart");
        };
        assert_eq!(diagnostic, expected);
    }

    #[test]
    fn legacy_init_table_preserves_released_variants_and_contracts() {
        let logger_remediation = Remediation::recoverable("repair logger configuration", ["retry"]);
        let runtime_remediation = Remediation::recoverable("restore runtime", ["retry"]);

        let cases = [
            (
                "configuration logger",
                legacy_init(
                    sc_observability_types::v2::InitError::Configuration {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOGGER_QUEUE_CAPACITY_INVALID",
                            logger_remediation.clone(),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOGGER_QUEUE_CAPACITY_INVALID",
                logger_remediation.clone(),
            ),
            (
                "runtime logger",
                legacy_init(
                    sc_observability_types::v2::InitError::Runtime {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOGGER_RUNTIME_UNAVAILABLE",
                            logger_remediation.clone(),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOGGER_RUNTIME_UNAVAILABLE",
                logger_remediation.clone(),
            ),
            (
                "runtime start",
                legacy_init(
                    sc_observability_types::v2::InitError::Runtime {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED",
                            runtime_remediation.clone(),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED",
                Remediation::recoverable(
                    "inspect thread and resource availability, then retry initialization explicitly",
                    std::iter::empty::<String>(),
                ),
            ),
            (
                "identity resolution",
                legacy_init(
                    sc_observability_types::v2::InitError::Runtime {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED",
                            runtime_remediation.clone(),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED",
                runtime_remediation.clone(),
            ),
        ];

        for (name, error, code, remediation) in cases {
            match name {
                "configuration logger" | "runtime logger" => {
                    assert!(matches!(error, InitError::Logger { .. }));
                }
                "runtime start" => assert!(matches!(error, InitError::RuntimeStart { .. })),
                "identity resolution" => {
                    assert!(matches!(error, InitError::IdentityResolution { .. }));
                }
                _ => unreachable!("fixed table row"),
            }
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn legacy_flush_table_preserves_released_variants_and_contracts() {
        let diagnostic_remediation = Remediation::recoverable("inspect logger", ["retry"]);
        let timeout = Duration::from_millis(7);
        let cases = [
            (
                "timeout",
                "SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT",
                legacy_flush(
                    &sc_observability_types::v2::FlushError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                Remediation::recoverable(
                    "retry the flush later or raise the timeout",
                    [
                        "a shutdown before the detached flush returns reports ShutdownError::TimedOut",
                    ],
                ),
            ),
            (
                "helper spawn",
                "SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED",
                legacy_flush(
                    &sc_observability_types::v2::FlushError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                Remediation::not_recoverable(
                    "inspect resource availability and retained logger health; completion is unconfirmed",
                ),
            ),
            (
                "helper lost",
                "SC_OBSERVABILITY_LOG_HELPER_LOST",
                legacy_flush(
                    &sc_observability_types::v2::FlushError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_HELPER_LOST",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                Remediation::not_recoverable(
                    "inspect retained lifecycle and health; do not claim worker completion",
                ),
            ),
        ];

        for (name, code, error, remediation) in cases {
            match name {
                "timeout" => assert!(matches!(
                    error,
                    FlushError::TimedOut { timeout: observed } if observed == timeout
                )),
                "helper spawn" => assert!(matches!(error, FlushError::HelperSpawn { .. })),
                "helper lost" => assert!(matches!(error, FlushError::HelperLost { .. })),
                _ => unreachable!("fixed table row"),
            }
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn released_control_flush_reads_the_failed_phase_at_the_adapter_boundary() {
        struct RestoreStopped;

        impl Drop for RestoreStopped {
            fn drop(&mut self) {
                crate::handle::set_lifecycle(crate::health::BridgeLifecycle::Stopped);
            }
        }

        if !crate::handle::is_isolated_test_child(
            "compat::tests::released_control_flush_reads_the_failed_phase_at_the_adapter_boundary",
        ) {
            return;
        }
        crate::handle::set_lifecycle(crate::health::BridgeLifecycle::Failed);
        let _restore = RestoreStopped;
        let error = LogControl::new()
            .flush(Duration::ZERO)
            .expect_err("failed lifecycle must reject the released flush");
        assert!(matches!(
            error,
            FlushError::NotRunning {
                phase: crate::LifecyclePhase::Failed,
            }
        ));
    }

    #[test]
    fn legacy_shutdown_and_emit_tables_preserve_root_contracts() {
        let diagnostic_remediation = Remediation::recoverable("inspect logger", ["retry"]);
        let timeout = Duration::from_millis(7);
        let shutdown_cases = [
            (
                "timeout",
                legacy_shutdown(
                    sc_observability_types::v2::ShutdownError::Timeout {
                        context: Box::new(context("SC_UNUSED", diagnostic_remediation.clone())),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT",
                Remediation::recoverable(
                    "use control.wait_stopped to observe the original shutdown",
                    std::iter::empty::<String>(),
                ),
            ),
            (
                "helper spawn",
                legacy_shutdown(
                    sc_observability_types::v2::ShutdownError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED",
                Remediation::not_recoverable(
                    "inspect resource availability and logger health; completion is unconfirmed",
                ),
            ),
            (
                "final flush",
                legacy_shutdown(
                    sc_observability_types::v2::ShutdownError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED",
                diagnostic_remediation.clone(),
            ),
        ];
        for (name, error, code, remediation) in shutdown_cases {
            match name {
                "timeout" => assert!(matches!(error, ShutdownError::TimedOut { .. })),
                "helper spawn" => assert!(matches!(error, ShutdownError::HelperSpawn { .. })),
                "final flush" => assert!(matches!(error, ShutdownError::FinalFlush { .. })),
                _ => unreachable!("fixed table row"),
            }
            assert_root_contract!(error, code, remediation);
        }

        let emit_cases = [
            (
                "queue full",
                legacy_emit(crate::error::EmitError::QueueFull {
                    diagnostic: diagnostic(
                        "SC_OBSERVABILITY_LOGGER_QUEUE_FULL",
                        diagnostic_remediation.clone(),
                    ),
                }),
                "SC_OBSERVABILITY_LOGGER_QUEUE_FULL",
                diagnostic_remediation.clone(),
            ),
            (
                "not running",
                legacy_emit(crate::error::EmitError::NotRunning {
                    phase: crate::LifecyclePhase::Failed,
                }),
                "SC_OBSERVABILITY_LOG_NOT_RUNNING",
                Remediation::not_recoverable("the owner has stopped the bridge"),
            ),
        ];
        for (name, error, code, remediation) in emit_cases {
            match name {
                "queue full" => assert!(matches!(error, EmitError::QueueFull { .. })),
                "not running" => assert!(matches!(
                    error,
                    EmitError::NotRunning {
                        phase: crate::LifecyclePhase::Failed
                    }
                )),
                _ => unreachable!("fixed table row"),
            }
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn legacy_emit_table_covers_each_remaining_released_row() {
        let diagnostic_remediation = Remediation::recoverable("repair writer", ["retry"]);
        let cases = [
            (
                "invalid field",
                legacy_emit(crate::error::EmitError::InvalidField {
                    raw_key: "reserved.key".to_owned(),
                    reason: crate::FieldKeyError::ReservedPrefix,
                }),
                "SC_OBSERVABILITY_LOG_INVALID_FIELD",
                Remediation::recoverable("correct the field key", ["resubmit the event"]),
            ),
            (
                "invalid event",
                legacy_emit(crate::error::EmitError::InvalidEvent {
                    diagnostic: diagnostic(
                        "SC_OBSERVABILITY_LOGGER_EVENT_INVALID",
                        diagnostic_remediation.clone(),
                    ),
                }),
                "SC_OBSERVABILITY_LOGGER_EVENT_INVALID",
                diagnostic_remediation.clone(),
            ),
            (
                "writer degraded",
                legacy_emit(crate::error::EmitError::WriterDegraded {
                    diagnostic: diagnostic(
                        "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED",
                        diagnostic_remediation.clone(),
                    ),
                }),
                "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED",
                diagnostic_remediation.clone(),
            ),
            (
                "shutdown timed out",
                legacy_emit(crate::error::EmitError::ShutdownTimedOut {
                    diagnostic: diagnostic(
                        "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT",
                        diagnostic_remediation.clone(),
                    ),
                }),
                "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT",
                diagnostic_remediation.clone(),
            ),
            (
                "not installed",
                legacy_emit(crate::error::EmitError::NotInstalled),
                "SC_OBSERVABILITY_LOG_NOT_RUNNING",
                Remediation::not_recoverable("the owner has stopped the bridge"),
            ),
            (
                "reentrant",
                legacy_emit(crate::error::EmitError::Reentrant),
                "SC_OBSERVABILITY_LOG_REENTRANT_EMIT",
                Remediation::not_recoverable(
                    "remove logging from formatter, redactor, or diagnostic callbacks",
                ),
            ),
            (
                "panicked",
                legacy_emit(crate::error::EmitError::Panicked),
                "SC_OBSERVABILITY_LOG_LOGGER_PANICKED",
                Remediation::not_recoverable(
                    "repair the panicking callback; do not retry implicitly",
                ),
            ),
        ];
        for (name, error, code, remediation) in cases {
            match name {
                "invalid field" => assert!(matches!(error, EmitError::InvalidField { .. })),
                "invalid event" => assert!(matches!(error, EmitError::InvalidEvent { .. })),
                "writer degraded" => assert!(matches!(error, EmitError::WriterDegraded { .. })),
                "shutdown timed out" => {
                    assert!(matches!(error, EmitError::ShutdownTimedOut { .. }));
                }
                "not installed" => assert!(matches!(
                    error,
                    EmitError::NotRunning {
                        phase: crate::LifecyclePhase::Stopped
                    }
                )),
                "reentrant" => assert!(matches!(error, EmitError::Reentrant)),
                "panicked" => assert!(matches!(error, EmitError::Panicked)),
                _ => unreachable!("fixed table row"),
            }
            assert_root_contract!(error, code, remediation);
        }
    }
}
