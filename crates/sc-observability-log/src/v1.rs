//! Released 1.x bridge facade over the canonical v2 implementation.
//!
//! These adapters preserve the error variants and root paths shipped before
//! the canonical operation errors were introduced. Runtime ownership remains
//! in the canonical bridge; adapters only translate its typed outcomes.

#![allow(
    deprecated,
    reason = "this module owns the retained deprecated v1 facade"
)]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sc_observability_types::{ErrorCode, LevelFilter, Remediation};
use serde::{Deserialize, Serialize};

use crate::{control, error_codes};

#[deprecated(note = "use sc_observability_log::v2::InitError")]
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

#[deprecated(note = "use sc_observability_log::v2::FlushError")]
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

#[deprecated(note = "use sc_observability_log::v2::ShutdownError")]
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

#[deprecated(note = "use sc_observability_log::v2::EventError")]
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
    /// The core shutdown deadline has elapsed.
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

#[deprecated(note = "use sc_observability_log::v2::LogControl")]
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
    #[deprecated(
        note = "use sc_observability_log::v2::LogControl::flush or flush_with_timeout; see docs/migration/phase-f.md"
    )]
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        ROOT_CONTROL
            .flush_with_timeout(timeout)
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
        // A missing attachment is stopped independently of any unrelated global owner.
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

/// Released 1.x lifecycle owner retained during the migration window.
#[deprecated(note = "use sc_observability_log::v2::LogGuard")]
#[must_use = "dropping the guard shuts the logger down"]
#[derive(Debug)]
pub struct LogGuard {
    inner: crate::InnerLogGuard,
}

#[allow(
    deprecated,
    reason = "the released wrapper intentionally preserves its own path"
)]
#[allow(
    clippy::missing_errors_doc,
    reason = "the released methods retain their documented contract"
)]
impl LogGuard {
    /// Returns a cloneable, non-owning released control.
    #[must_use]
    pub fn control(&self) -> LogControl {
        LogControl::new()
    }

    /// Raises the staged core's effective filter.
    pub fn elevate_level(
        &mut self,
        level: LevelFilter,
        source: sc_observability_types::LevelChangeSource,
    ) -> Result<sc_observability_types::LevelChange, sc_observability_types::LevelChangeError> {
        self.inner.elevate_level(level, source)
    }

    /// Restores the configured runtime-level baseline.
    pub fn reset_level(
        &mut self,
        source: sc_observability_types::LevelChangeSource,
    ) -> Result<sc_observability_types::LevelChange, sc_observability_types::LevelChangeError> {
        self.inner.reset_level(source)
    }

    /// Requests a bounded flush through the released error surface.
    #[deprecated(
        note = "use sc_observability_log::v2::LogGuard::flush or flush_with_timeout; see docs/migration/phase-f.md"
    )]
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        crate::handle::flush_installed(timeout).map_err(|error| legacy_flush(&error, timeout))
    }

    /// Performs final flush and shutdown through the released error surface.
    #[deprecated(
        note = "use sc_observability_log::v2::LogGuard::shutdown or shutdown_with_timeout; see docs/migration/phase-f.md"
    )]
    pub fn shutdown(self, timeout: Duration) -> Result<(), ShutdownError> {
        self.inner
            .shut_down
            .store(true, std::sync::atomic::Ordering::Release);
        crate::handle::shutdown_sequence(timeout).map_err(|error| legacy_shutdown(error, timeout))
    }

    /// Snapshots bridge-owned dropped-event counters.
    #[must_use]
    pub fn dropped_events(&self) -> crate::DroppedEvents {
        self.inner.dropped_events()
    }

    /// Returns the cached active log path.
    #[must_use]
    pub fn active_log_path(&self) -> Option<&Path> {
        self.inner.active_log_path()
    }

    /// Reads retained bridge health.
    pub fn health(&self) -> Result<crate::BridgeHealthReport, crate::ControlError> {
        self.inner.health()
    }
}

impl Drop for LogGuard {
    fn drop(&mut self) {}
}

/// Installs the released 1.x bridge facade over the canonical lifecycle owner.
#[deprecated(note = "use sc_observability_log::v2::init")]
#[allow(
    deprecated,
    reason = "the released signature retains the deprecated owner type"
)]
#[allow(
    clippy::missing_errors_doc,
    reason = "the released v1 error contract is documented on its canonical replacement"
)]
pub fn init(
    config: crate::LoggerConfig,
    options: crate::BridgeOptions,
) -> Result<LogGuard, InitError> {
    let configured = config.level;
    let available = crate::ensure_static_level(configured)
        .err()
        .unwrap_or(configured);
    crate::init_canonical(config, options)
        .map(|inner| LogGuard { inner })
        .map_err(|error| legacy_init(error, configured, available))
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
        Core::Drain { .. } if diagnostic.code == error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING => {
            FlushError::NotRunning {
                phase: crate::handle::lifecycle_phase(),
            }
        }
        // A saved attachment is stopped even if a separate global owner is running.
        Core::Drain { .. } if diagnostic.code == error_codes::SC_LOG_DETACH_NOT_INSTALLED => {
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

    fn assert_detached_flush_maps_to_stopped(stale: &crate::control::LogControl) {
        let core_flush_error = stale
            .flush_with_timeout(Duration::ZERO)
            .expect_err("detached saved attachment must reject flush");
        assert_eq!(
            core_flush_error.diagnostic().code,
            error_codes::SC_LOG_DETACH_NOT_INSTALLED,
            "the real saved-attachment path supplies the detach diagnostic"
        );
        let released_flush_error = legacy_flush(&core_flush_error, Duration::ZERO);
        assert!(matches!(
            released_flush_error,
            FlushError::NotRunning {
                phase: crate::LifecyclePhase::Stopped,
            }
        ));
        assert_eq!(
            released_flush_error.to_string(),
            "the logger is not running: Stopped"
        );
        assert_root_contract!(
            released_flush_error,
            "SC_OBSERVABILITY_LOG_NOT_RUNNING",
            Remediation::not_recoverable(
                "the lifecycle owner has shut the logger down; the final shutdown flushed what was queued",
            )
        );
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
    fn owned_init_guard_flush_rejects_a_second_in_flight_flush() {
        if !crate::handle::is_isolated_test_child(
            "v1::tests::owned_init_guard_flush_rejects_a_second_in_flight_flush",
        ) {
            return;
        }

        let root = tempfile::tempdir().expect("temporary log root");
        let guard = init(
            crate::LoggerConfig::default_for(
                crate::ServiceName::new("owned-guard-single-flight").expect("service name"),
                root.path().to_path_buf(),
            ),
            crate::BridgeOptions {
                default_action: crate::ActionName::new("log.record").expect("action name"),
                parse_bracket_action: false,
            },
        )
        .expect("owned bridge initialization");

        let release = crate::handle::block_next_bounded_helper();
        let timed_out = guard
            .flush(Duration::from_millis(10))
            .expect_err("the channel-gated helper must time out");
        assert!(matches!(timed_out, FlushError::TimedOut { .. }));

        let retry = guard
            .flush(Duration::from_secs(1))
            .expect_err("a second owned flush must be rejected");
        assert!(matches!(retry, FlushError::InProgress));
        assert_eq!(
            retry.code(),
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );

        release.send(()).expect("release the channel-gated helper");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            match guard.flush(Duration::from_secs(1)) {
                Ok(()) => break,
                Err(FlushError::InProgress) if std::time::Instant::now() < deadline => {
                    std::thread::yield_now();
                }
                Err(error) => panic!("released owned flush must succeed: {error:?}"),
            }
        }
        guard
            .shutdown(Duration::from_secs(10))
            .expect("owned bridge shutdown");
    }

    #[test]
    fn detached_attachment_maps_not_installed_to_released_stopped_while_global_is_running() {
        struct Admit;

        impl crate::BridgeEventPolicy for Admit {
            fn decide(&self, _: &crate::LogEvent) -> crate::BridgeEventDecision {
                crate::BridgeEventDecision::Admit
            }
        }

        struct RestoreStopped;

        impl Drop for RestoreStopped {
            fn drop(&mut self) {
                crate::handle::set_lifecycle(crate::health::BridgeLifecycle::Stopped);
            }
        }

        if !crate::handle::is_isolated_test_child(
            "v1::tests::detached_attachment_maps_not_installed_to_released_stopped_while_global_is_running",
        ) {
            return;
        }

        let root = tempfile::tempdir().expect("temporary log root");
        let service = crate::ServiceName::new("detached-attachment").expect("service name");
        let logger = std::sync::Arc::new(
            sc_observability::v2::Logger::new(crate::LoggerConfig::default_for(
                service,
                root.path().to_path_buf(),
            ))
            .expect("host logger"),
        );
        let mut attachment = crate::attach_logger(
            std::sync::Arc::clone(&logger),
            crate::AttachmentOptions::new(
                crate::BridgeOptions {
                    default_action: crate::ActionName::new("log.record").expect("action name"),
                    parse_bracket_action: false,
                },
                std::sync::Arc::new(Admit),
            ),
        )
        .expect("attach host logger");
        let stale = attachment.control();
        attachment
            .detach(std::time::Duration::ZERO)
            .expect("detach attachment before stale admission");

        // Test-only unrelated lifecycle state: no owner is installed here. The
        // real attachment path above proves `NotInstalled`; this prevents the
        // released adapter from consulting an unrelated global phase.
        crate::handle::set_lifecycle(crate::health::BridgeLifecycle::Running);
        let _restore = RestoreStopped;
        let core_error = stale
            .try_log(crate::BridgeEvent {
                level: crate::EventLevel::Info,
                target: crate::TargetCategory::new("bridge.compat").expect("target"),
                action: None,
                message: Some("stale attachment".to_owned()),
                outcome: None,
                fields: serde_json::Map::new(),
                request_id: None,
                correlation_id: None,
                trace: None,
            })
            .expect_err("detached attachment");
        assert!(matches!(core_error, crate::error::EmitError::NotInstalled));
        assert!(matches!(
            legacy_emit(core_error),
            EmitError::NotRunning {
                phase: crate::LifecyclePhase::Stopped,
            }
        ));

        assert_detached_flush_maps_to_stopped(&stale);

        let _ = std::sync::Arc::try_unwrap(logger)
            .unwrap_or_else(|_| panic!("detach releases host logger"))
            .shutdown();
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
    #[expect(
        clippy::too_many_lines,
        reason = "the released init conversion matrix keeps all variant and root-contract rows adjacent"
    )]
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
                "configuration runtime-start code remains logger",
                legacy_init(
                    sc_observability_types::v2::InitError::Configuration {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED",
                            runtime_remediation.clone(),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED",
                runtime_remediation.clone(),
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
                "configuration logger"
                | "configuration runtime-start code remains logger"
                | "runtime logger" => {
                    assert!(matches!(error, InitError::Logger { .. }));
                }
                "runtime start" => assert!(matches!(error, InitError::RuntimeStart { .. })),
                "identity resolution" => {
                    assert!(matches!(error, InitError::IdentityResolution { .. }));
                }
                _ => unreachable!("fixed table row"),
            }
            let expected_display = match name {
                "configuration logger" => {
                    "sc-observability logger construction failed: SC_OBSERVABILITY_LOGGER_QUEUE_CAPACITY_INVALID fixture"
                }
                "configuration runtime-start code remains logger" => {
                    "sc-observability logger construction failed: SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED fixture"
                }
                "runtime logger" => {
                    "sc-observability logger construction failed: SC_OBSERVABILITY_LOGGER_RUNTIME_UNAVAILABLE fixture"
                }
                "runtime start" => {
                    "could not start bridge lifecycle coordination: SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED fixture"
                }
                "identity resolution" => {
                    "process identity resolution failed: SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED fixture"
                }
                "unknown runtime" => {
                    "sc-observability logger construction failed: SC_OBSERVABILITY_LOG_NOT_RUNNING fixture"
                }
                _ => unreachable!("fixed table row"),
            };
            assert_eq!(error.to_string(), expected_display, "{name}");
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn unknown_runtime_code_remains_released_logger() {
        let remediation = Remediation::recoverable("repair logger configuration", ["retry"]);
        let error = legacy_init(
            sc_observability_types::v2::InitError::Runtime {
                context: Box::new(context(
                    "SC_OBSERVABILITY_LOG_NOT_RUNNING",
                    remediation.clone(),
                )),
            },
            LevelFilter::Info,
            LevelFilter::Trace,
        );
        assert!(matches!(error, InitError::Logger { .. }));
        assert_eq!(
            error.to_string(),
            "sc-observability logger construction failed: SC_OBSERVABILITY_LOG_NOT_RUNNING fixture"
        );
        assert_root_contract!(error, "SC_OBSERVABILITY_LOG_NOT_RUNNING", remediation);
    }

    #[test]
    fn legacy_init_special_configuration_rows_preserve_root_contracts() {
        let special_cases = [
            (
                "already initialized",
                legacy_init(
                    sc_observability_types::v2::InitError::Configuration {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED",
                            Remediation::not_recoverable("unused canonical remediation"),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED",
                Remediation::not_recoverable(
                    "the log facade logger cannot be replaced; keep the first LogGuard",
                ),
            ),
            (
                "foreign logger",
                legacy_init(
                    sc_observability_types::v2::InitError::Configuration {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED",
                            Remediation::not_recoverable("unused canonical remediation"),
                        )),
                    },
                    LevelFilter::Info,
                    LevelFilter::Trace,
                ),
                "SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED",
                Remediation::not_recoverable("choose one application logger before startup"),
            ),
            (
                "unsupported level",
                legacy_init(
                    sc_observability_types::v2::InitError::Configuration {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL",
                            Remediation::not_recoverable("unused canonical remediation"),
                        )),
                    },
                    LevelFilter::Warn,
                    LevelFilter::Info,
                ),
                "SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL",
                Remediation::not_recoverable(
                    "rebuild without the static cap or choose a supported startup baseline",
                ),
            ),
        ];

        for (name, error, code, remediation) in special_cases {
            match name {
                "already initialized" => {
                    assert!(matches!(error, InitError::AlreadyInitialized));
                    assert_eq!(
                        error.to_string(),
                        "sc-observability-log is already initialized in this process"
                    );
                }
                "foreign logger" => {
                    assert!(matches!(error, InitError::ForeignLoggerInstalled));
                    assert_eq!(
                        error.to_string(),
                        "another log::Log implementation is already installed"
                    );
                }
                "unsupported level" => {
                    assert!(matches!(
                        error,
                        InitError::UnsupportedLevel {
                            configured: LevelFilter::Warn,
                            available: LevelFilter::Info,
                        }
                    ));
                    assert_eq!(
                        error.to_string(),
                        "configured level Warn exceeds available static level Info"
                    );
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
            let expected_display = match name {
                "timeout" => "flush did not complete within 7ms",
                "helper spawn" => {
                    "could not start the flush helper thread: SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED fixture"
                }
                "helper lost" => {
                    "the flush helper thread ended without a result: SC_OBSERVABILITY_LOG_HELPER_LOST fixture"
                }
                _ => unreachable!("fixed table row"),
            };
            assert_eq!(error.to_string(), expected_display, "{name}");
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn legacy_flush_in_progress_and_logger_rows_preserve_root_contracts() {
        let diagnostic_remediation = Remediation::recoverable("inspect logger", ["retry"]);
        let timeout = Duration::from_millis(7);
        let cases = [
            (
                "in progress",
                legacy_flush(
                    &sc_observability_types::v2::FlushError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS",
                Remediation::recoverable(
                    "wait for the previous flush to finish, then retry",
                    ["wait for the in-flight flush before making an explicit new request"],
                ),
            ),
            (
                "generic logger",
                legacy_flush(
                    &sc_observability_types::v2::FlushError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED",
                diagnostic_remediation,
            ),
        ];

        for (name, error, code, remediation) in cases {
            match name {
                "in progress" => {
                    assert!(matches!(error, FlushError::InProgress));
                    assert_eq!(
                        error.to_string(),
                        "a previous flush is still running; no new flush was started"
                    );
                }
                "generic logger" => {
                    let FlushError::Logger { diagnostic } = &error else {
                        panic!("generic drain errors retain the released Logger variant");
                    };
                    assert_eq!(
                        diagnostic.message,
                        "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED fixture"
                    );
                }
                _ => unreachable!("fixed table row"),
            }
            let expected_display = match name {
                "in progress" => "a previous flush is still running; no new flush was started",
                "generic logger" => {
                    "sc-observability flush failed: SC_OBSERVABILITY_LOGGER_FLUSH_FAILED fixture"
                }
                _ => unreachable!("fixed table row"),
            };
            assert_eq!(error.to_string(), expected_display, "{name}");
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn synthetic_released_control_flush_preserves_non_running_phase_at_the_adapter_boundary() {
        struct RestoreStopped;

        impl Drop for RestoreStopped {
            fn drop(&mut self) {
                crate::handle::set_lifecycle(crate::health::BridgeLifecycle::Stopped);
            }
        }

        if !crate::handle::is_isolated_test_child(
            "v1::tests::synthetic_released_control_flush_preserves_non_running_phase_at_the_adapter_boundary",
        ) {
            return;
        }
        // Synthetic adapter probe; real producer coverage remains in handle.rs and shutdown_timeout.rs.
        let _restore = RestoreStopped;
        for (lifecycle, phase, message) in [
            (
                crate::health::BridgeLifecycle::ShuttingDown,
                crate::LifecyclePhase::Stopping,
                "the logger is not running: Stopping",
            ),
            (
                crate::health::BridgeLifecycle::Failed,
                crate::LifecyclePhase::Failed,
                "the logger is not running: Failed",
            ),
        ] {
            crate::handle::set_lifecycle(lifecycle);
            let error = LogControl::new()
                .flush(Duration::ZERO)
                .expect_err("non-running lifecycle must reject the released flush");
            assert!(matches!(
                error,
                FlushError::NotRunning { phase: observed } if observed == phase
            ));
            assert_eq!(error.to_string(), message);
            assert_root_contract!(
                error,
                "SC_OBSERVABILITY_LOG_NOT_RUNNING",
                Remediation::not_recoverable(
                    "the lifecycle owner has shut the logger down; the final shutdown flushed what was queued",
                )
            );
        }
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
            (
                "helper lost",
                legacy_shutdown(
                    sc_observability_types::v2::ShutdownError::Drain {
                        context: Box::new(context(
                            "SC_OBSERVABILITY_LOG_HELPER_LOST",
                            diagnostic_remediation.clone(),
                        )),
                    },
                    timeout,
                ),
                "SC_OBSERVABILITY_LOG_HELPER_LOST",
                Remediation::not_recoverable(
                    "inspect saved lifecycle and health; do not claim worker completion",
                ),
            ),
        ];
        for (name, error, code, remediation) in shutdown_cases {
            match name {
                "timeout" => assert!(matches!(error, ShutdownError::TimedOut { .. })),
                "helper spawn" => assert!(matches!(error, ShutdownError::HelperSpawn { .. })),
                "final flush" => assert!(matches!(error, ShutdownError::FinalFlush { .. })),
                "helper lost" => {
                    let ShutdownError::HelperLost { diagnostic } = &error else {
                        panic!("helper loss retains the released HelperLost variant");
                    };
                    assert_eq!(
                        diagnostic.message,
                        "SC_OBSERVABILITY_LOG_HELPER_LOST fixture"
                    );
                }
                _ => unreachable!("fixed table row"),
            }
            let expected_display = match name {
                "timeout" => "shutdown did not complete within 7ms",
                "helper spawn" => {
                    "could not start the shutdown helper thread: SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED fixture"
                }
                "final flush" => {
                    "final flush failed; the logger was still shut down: SC_OBSERVABILITY_LOGGER_FLUSH_FAILED fixture"
                }
                "helper lost" => {
                    "the shutdown helper thread ended without a result: SC_OBSERVABILITY_LOG_HELPER_LOST fixture"
                }
                _ => unreachable!("fixed table row"),
            };
            assert_eq!(error.to_string(), expected_display, "{name}");
            assert_root_contract!(error, code, remediation);
        }
    }

    #[test]
    fn legacy_emit_root_contract_rows_preserve_root_contracts() {
        let diagnostic_remediation = Remediation::recoverable("inspect logger", ["retry"]);
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
            let expected_display = match name {
                "queue full" => "writer queue is full: SC_OBSERVABILITY_LOGGER_QUEUE_FULL fixture",
                "not running" => "logger is not running: Failed",
                _ => unreachable!("fixed table row"),
            };
            assert_eq!(error.to_string(), expected_display, "{name}");
            assert_root_contract!(error, code, remediation);
        }
    }

    fn assert_remaining_emit_display(name: &str, error: &EmitError) {
        let expected_display = match name {
            "invalid field" => "invalid field \"reserved.key\": field key uses a reserved prefix",
            "invalid event" => "invalid event: SC_OBSERVABILITY_LOGGER_EVENT_INVALID fixture",
            "writer degraded" => {
                "writer is degraded: SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED fixture"
            }
            "shutdown timed out" => {
                "logger shutdown timed out: SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT fixture"
            }
            "not installed" => "logger is not running: Stopped",
            "reentrant" => "reentrant emission",
            "panicked" => "logger callback panicked",
            _ => unreachable!("fixed table row"),
        };
        assert_eq!(error.to_string(), expected_display, "{name}");
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
            assert_remaining_emit_display(name, &error);
            assert_root_contract!(error, code, remediation);
        }
    }
}
