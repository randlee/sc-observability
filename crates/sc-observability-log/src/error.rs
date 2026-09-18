//! Public error enums and the dropped-event cause.
//!
//! Every error type is a discriminated union: callers `match` on the variant and
//! read its typed fields. Each public error enum also exposes a stable
//! [`ErrorCode`] and a mandatory [`Remediation`] per variant; variants wrapping an
//! sc-observability error return that error's own code and remediation. Each
//! remains a directly serializable data contract, so consumers never parse a
//! display string.

use std::time::Duration;

use sc_observability_types::{DiagnosticInfo, ErrorCode, LevelFilter, Remediation};
use serde::{Deserialize, Serialize};

use crate::error_codes;

/// Projects a core diagnostic without retaining its opaque source error.
pub(crate) fn diagnostic_from_info(
    source: &impl DiagnosticInfo,
) -> sc_observability_types::OperationDiagnostic {
    let diagnostic = source.diagnostic();
    sc_observability_types::OperationDiagnostic {
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        remediation: diagnostic.remediation.clone(),
        at: diagnostic.timestamp,
    }
}

/// Lifecycle projection used by the reviewed B.P3 direct/control contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    /// The bridge is installed and can admit work.
    Running,
    /// The owner has begun final shutdown.
    Stopping,
    /// Final shutdown was confirmed.
    Stopped,
    /// Completion could not be confirmed; this never claims the writer stopped.
    Failed,
}

/// Reason a direct producer field cannot enter the bridge event.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FieldKeyError {
    /// The raw key is empty.
    #[error("field key is empty")]
    Empty,
    /// The raw key belongs to bridge-owned metadata.
    #[error("field key uses a reserved prefix")]
    ReservedPrefix,
    /// Two distinct raw keys normalize to the same output key.
    #[error("field key collides with {other_raw_key:?}")]
    Collision {
        /// The distinct raw key that normalized to the same output key.
        other_raw_key: String,
    },
}

/// Typed direct-admission failure; each path has already recorded exactly one drop cause.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EmitError {
    /// A producer field cannot be represented safely.
    #[error("invalid field {raw_key:?}: {reason}")]
    InvalidField {
        /// The producer-supplied key that was rejected.
        raw_key: String,
        /// The specific key-validation failure.
        reason: FieldKeyError,
    },
    /// The core rejected the assembled event.
    #[error("invalid event: {diagnostic}")]
    InvalidEvent {
        /// The core diagnostic explaining the invalid event.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The core queue is full.
    #[error("writer queue is full: {diagnostic}")]
    QueueFull {
        /// The core diagnostic describing queue saturation.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The writer cannot accept more work.
    #[error("writer is degraded: {diagnostic}")]
    WriterDegraded {
        /// The core diagnostic describing the writer failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The core shutdown deadline has elapsed.
    #[error("logger shutdown timed out: {diagnostic}")]
    ShutdownTimedOut {
        /// The core diagnostic describing the elapsed shutdown deadline.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The lifecycle is no longer running.
    #[error("logger is not running: {phase:?}")]
    NotRunning {
        /// The lifecycle phase observed when emission was attempted.
        phase: LifecyclePhase,
    },
    /// The producer re-entered the guarded path.
    #[error("reentrant emission")]
    Reentrant,
    /// A logger callback panic was contained.
    #[error("logger callback panicked")]
    Panicked,
}

/// Failure of a read-only control operation.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ControlError {
    /// The lifecycle does not permit the request.
    #[error("logger is not running: {phase:?}")]
    NotRunning {
        /// The lifecycle phase observed when the control operation was attempted.
        phase: LifecyclePhase,
    },
    /// A core query failed.
    #[error("query failed: {diagnostic}")]
    Query {
        /// The core diagnostic explaining the failed query.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// A snapshot or capability is unavailable.
    #[error("control operation unavailable: {diagnostic}")]
    Unavailable {
        /// The core diagnostic for the unavailable snapshot or capability.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

/// Failure while waiting for owner shutdown completion.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum WaitError {
    /// No bridge lifecycle has begun in this process.
    #[error("logger was not started")]
    NotStarted,
    /// The completed owner result was not observed by the deadline.
    #[error("shutdown wait timed out after {timeout:?}")]
    TimedOut {
        /// The maximum time the caller waited for an owner result.
        timeout: Duration,
    },
    /// Observation state is unavailable.
    #[error("shutdown state unavailable: {diagnostic}")]
    Unavailable {
        /// The diagnostic explaining why retained state cannot be observed.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

/// Outcome retained after owner shutdown has begun.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum UnconfirmedShutdown {
    /// The shutdown helper could not be started.
    HelperSpawn {
        /// The diagnostic for the failed helper startup.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The shutdown helper disappeared before a final result.
    HelperLost {
        /// The diagnostic for the helper that ended without a final result.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

/// Final or observable pending shutdown state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShutdownOutcome {
    /// The core writer reported a confirmed stop.
    Stopped,
    /// The writer stopped despite a final flush error.
    StoppedWithFlushError {
        /// The final flush diagnostic retained alongside the confirmed stop.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// Completion was not confirmed; callers must not infer stopped.
    Unconfirmed {
        /// The reason completion could not be confirmed.
        cause: UnconfirmedShutdown,
    },
}

/// Read-only shutdown observation returned to controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownReport {
    /// Completion outcome.
    pub outcome: ShutdownOutcome,
    /// Health projection taken at observation time.
    pub health: crate::BridgeHealthReport,
}

impl EmitError {
    /// Stable code for this direct-admission failure.
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

    /// Recovery guidance preserved from the staged core where available.
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

impl ControlError {
    /// Stable code for this control failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotRunning { .. } => error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
            Self::Query { diagnostic } => diagnostic.code.clone(),
            Self::Unavailable { .. } => error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE,
        }
    }

    /// Recovery guidance for this control failure.
    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::Query { diagnostic } => diagnostic.remediation.clone(),
            Self::Unavailable { .. } => Remediation::not_recoverable(
                "inspect retained logger health and lifecycle evidence",
            ),
            Self::NotRunning { .. } => {
                Remediation::not_recoverable("the owner has stopped the bridge")
            }
        }
    }
}

impl WaitError {
    /// Stable code for this wait failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotStarted => error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_NOT_STARTED,
            Self::TimedOut { .. } => error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            Self::Unavailable { .. } => error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE,
        }
    }

    /// Recovery guidance for this wait failure.
    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::Unavailable { .. } => Remediation::not_recoverable(
                "inspect retained logger health and lifecycle evidence",
            ),
            Self::NotStarted => Remediation::recoverable(
                "initialize the bridge before waiting",
                std::iter::empty::<String>(),
            ),
            Self::TimedOut { .. } => Remediation::recoverable(
                "wait longer for the existing owner shutdown",
                std::iter::empty::<String>(),
            ),
        }
    }
}

/// Error returned by [`init`](crate::init).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum InitError {
    /// The bridge is already installed (or being installed) in this process.
    #[error("sc-observability-log is already initialized in this process")]
    AlreadyInitialized,
    /// Another `log::Log` implementation owns the `log` facade.
    #[error("another log::Log implementation is already installed")]
    ForeignLoggerInstalled,
    /// The requested baseline cannot be represented by the executable's
    /// compile-time `log` cap. This is rejected before global installation.
    #[error("configured level {configured:?} exceeds available static level {available:?}")]
    UnsupportedLevel {
        /// Requested configured baseline.
        configured: LevelFilter,
        /// Most verbose facade level compiled into this executable.
        available: LevelFilter,
    },
    /// `ProcessIdentityPolicy::Resolver` failed, or `Auto` could not resolve a non-empty hostname.
    #[error("process identity resolution failed: {diagnostic}")]
    IdentityResolution {
        /// Stable diagnostic projected from the resolver failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// `sc_observability::Logger::new` failed.
    #[error("sc-observability logger construction failed: {diagnostic}")]
    Logger {
        /// Stable diagnostic projected from the core construction failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// Required lifecycle coordination could not be reserved before facade installation.
    #[error("could not start bridge lifecycle coordination: {diagnostic}")]
    RuntimeStart {
        /// Stable startup diagnostic without an opaque helper source.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

/// Error returned by [`LogGuard::flush`](crate::LogGuard::flush) and [`LogControl::flush`](crate::LogControl::flush).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FlushError {
    /// The writer did not acknowledge the flush within `timeout`.
    #[error("flush did not complete within {timeout:?}")]
    TimedOut {
        /// The timeout that elapsed.
        timeout: Duration,
    },
    /// A sink flush failed or the writer disconnected.
    #[error("sc-observability flush failed: {diagnostic}")]
    Logger {
        /// Stable diagnostic projected from the core flush failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The flush helper thread could not be started.
    #[error("could not start the flush helper thread: {diagnostic}")]
    HelperSpawn {
        /// Stable diagnostic for the helper-start failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The flush helper thread ended without a result.
    #[error("the flush helper thread ended without a result: {diagnostic}")]
    HelperLost {
        /// Stable diagnostic for the missing helper result.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// A previous flush helper is still running (possibly detached after its caller's
    /// timeout); no new helper was started and nothing new was flushed.
    #[error("a previous flush is still running; no new flush was started")]
    InProgress,
    /// The owner has stopped accepting flush requests.
    #[error("the logger is not running: {phase:?}")]
    NotRunning {
        /// The lifecycle phase observed when the flush was requested.
        phase: LifecyclePhase,
    },
}

/// Error returned by [`LogGuard::shutdown`](crate::LogGuard::shutdown).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShutdownError {
    /// Sole ownership, final flush and writer join did not finish within `timeout`.
    #[error("shutdown did not complete within {timeout:?}")]
    TimedOut {
        /// The timeout that elapsed.
        timeout: Duration,
    },
    /// The final flush failed; the logger was still shut down.
    #[error("final flush failed; the logger was still shut down: {diagnostic}")]
    FinalFlush {
        /// Stable diagnostic projected from the final core flush failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The shutdown helper thread could not be started.
    #[error("could not start the shutdown helper thread: {diagnostic}")]
    HelperSpawn {
        /// Stable diagnostic for the coordinator-send failure.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
    /// The shutdown helper thread ended without a result.
    #[error("the shutdown helper thread ended without a result: {diagnostic}")]
    HelperLost {
        /// Stable diagnostic for the missing coordinator result.
        diagnostic: sc_observability_types::OperationDiagnostic,
    },
}

impl InitError {
    /// Stable code per variant; wrapped sc-observability errors return their own code.
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

    /// Mandatory remediation per variant; wrapped errors return their own remediation.
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
    /// Stable code per variant; wrapped sc-observability errors return their own code.
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

    /// Mandatory remediation per variant; wrapped errors return their own remediation.
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
    /// Stable code per variant; wrapped sc-observability errors return their own code.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::TimedOut { .. } => error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            Self::FinalFlush { diagnostic } => diagnostic.code.clone(),
            Self::HelperSpawn { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            Self::HelperLost { .. } => error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
        }
    }

    /// Mandatory remediation per variant; wrapped errors return their own remediation.
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

/// Why an event was dropped on the non-blocking emit path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropCause {
    /// `TryLogError::QueueFull`: the bounded writer queue was full.
    QueueFull,
    /// `TryLogError::InvalidEvent`, or a runtime label or field key failing the sanitizer.
    InvalidEvent,
    /// `TryLogError::WriterDegraded`: the writer thread is degraded.
    WriterDegraded,
    /// `TryLogError::ShutdownTimedOut`: the logger exceeded its shutdown threshold.
    ShutdownTimedOut,
    /// No logger was installed (after shutdown took it), or a structured submission after shutdown.
    NotInstalled,
    /// A panic inside the emit guard: sc-observability `try_log` or a `log` record's formatting.
    LoggerPanicked,
    /// A record was emitted while the emit guard was active on the thread (formatter, hook, sink).
    ReentrantEmit,
}

impl DropCause {
    /// Every variant, in declaration order.
    pub const ALL: [DropCause; 7] = [
        Self::QueueFull,
        Self::InvalidEvent,
        Self::WriterDegraded,
        Self::ShutdownTimedOut,
        Self::NotInstalled,
        Self::LoggerPanicked,
        Self::ReentrantEmit,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_non_empty(remediation: &Remediation) {
        match remediation {
            Remediation::Recoverable { steps } => {
                assert!(!steps.steps().is_empty());
                assert!(steps.steps().iter().all(|step| !step.is_empty()));
            }
            Remediation::NotRecoverable { justification } => assert!(!justification.is_empty()),
        }
    }

    fn projected(code: &'static str) -> sc_observability_types::OperationDiagnostic {
        sc_observability_types::OperationDiagnostic {
            code: ErrorCode::new_static(code),
            message: "projected diagnostic".to_owned(),
            remediation: Remediation::recoverable("follow the diagnostic", ["then retry"]),
            at: sc_observability_types::Timestamp::now_utc(),
        }
    }

    #[test]
    fn init_error_codes_and_remediations() {
        let cases = [
            (
                InitError::AlreadyInitialized,
                error_codes::SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED,
            ),
            (
                InitError::ForeignLoggerInstalled,
                error_codes::SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED,
            ),
            (
                InitError::IdentityResolution {
                    diagnostic: projected("SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED"),
                },
                error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED,
            ),
            (
                InitError::Logger {
                    diagnostic: projected("SC_OBSERVABILITY_LOGGER_INIT_FAILED"),
                },
                ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INIT_FAILED"),
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert_non_empty(&error.remediation());
        }
        let wrapped = InitError::Logger {
            diagnostic: projected("W"),
        };
        assert_eq!(
            wrapped.remediation(),
            Remediation::recoverable("follow the diagnostic", ["then retry"])
        );
        let identity = InitError::IdentityResolution {
            diagnostic: projected("I"),
        };
        assert_eq!(identity.code(), ErrorCode::new_static("I"));
        assert_eq!(
            identity.remediation(),
            Remediation::recoverable("follow the diagnostic", ["then retry"])
        );
    }

    #[test]
    fn flush_error_codes_and_remediations() {
        let cases = [
            (
                FlushError::TimedOut {
                    timeout: Duration::from_millis(5),
                },
                error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT,
            ),
            (
                FlushError::Logger {
                    diagnostic: projected("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"),
                },
                ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"),
            ),
            (
                FlushError::HelperSpawn {
                    diagnostic: projected("SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED"),
                },
                error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            ),
            (
                FlushError::HelperLost {
                    diagnostic: projected("SC_OBSERVABILITY_LOG_HELPER_LOST"),
                },
                error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
            ),
            (
                FlushError::NotRunning {
                    phase: LifecyclePhase::Stopped,
                },
                error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
            ),
            (
                FlushError::InProgress,
                error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS,
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert_non_empty(&error.remediation());
        }
    }

    #[test]
    fn shutdown_error_codes_and_remediations() {
        let cases = [
            (
                ShutdownError::TimedOut {
                    timeout: Duration::from_millis(5),
                },
                error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            ),
            (
                ShutdownError::FinalFlush {
                    diagnostic: projected("SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"),
                },
                ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"),
            ),
            (
                ShutdownError::HelperSpawn {
                    diagnostic: projected("SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED"),
                },
                error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
            ),
            (
                ShutdownError::HelperLost {
                    diagnostic: projected("SC_OBSERVABILITY_LOG_HELPER_LOST"),
                },
                error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert_non_empty(&error.remediation());
        }
    }

    fn assert_native_error_contract<T>(error: &T)
    where
        T: Clone + serde::Serialize + serde::de::DeserializeOwned + std::error::Error,
    {
        assert!(std::error::Error::source(error).is_none());
        assert!(!error.to_string().is_empty());
        let encoded = serde_json::to_value(error).unwrap();
        assert_eq!(serde_json::to_value(error.clone()).unwrap(), encoded);
        let decoded: T = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
    }

    #[test]
    fn field_key_error_variants_are_cloneable_and_round_trip() {
        for error in [
            FieldKeyError::Empty,
            FieldKeyError::ReservedPrefix,
            FieldKeyError::Collision {
                other_raw_key: "other".to_owned(),
            },
        ] {
            assert_native_error_contract(&error);
        }
    }

    macro_rules! assert_operation_contract {
        ($error:expr, $code:expr) => {{
            let error = $error;
            assert_eq!(error.code(), $code);
            assert_non_empty(&error.remediation());
            assert_native_error_contract(&error);
        }};
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the target contract requires this one exhaustive, auditable variant fixture"
    )]
    fn every_native_operation_error_variant_is_data_only_and_round_trips() {
        let diagnostic = || projected("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC");

        assert_operation_contract!(
            InitError::AlreadyInitialized,
            error_codes::SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED
        );
        assert_operation_contract!(
            InitError::ForeignLoggerInstalled,
            error_codes::SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED
        );
        assert_operation_contract!(
            InitError::UnsupportedLevel {
                configured: LevelFilter::Trace,
                available: LevelFilter::Info,
            },
            error_codes::SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL
        );
        assert_operation_contract!(
            InitError::IdentityResolution {
                diagnostic: diagnostic(),
            },
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
        );
        assert_operation_contract!(
            InitError::Logger {
                diagnostic: diagnostic(),
            },
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
        );
        assert_operation_contract!(
            InitError::RuntimeStart {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED
        );

        assert_operation_contract!(
            FlushError::TimedOut {
                timeout: Duration::from_millis(1),
            },
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT
        );
        assert_operation_contract!(
            FlushError::Logger {
                diagnostic: diagnostic(),
            },
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
        );
        assert_operation_contract!(
            FlushError::HelperSpawn {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED
        );
        assert_operation_contract!(
            FlushError::HelperLost {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST
        );
        assert_operation_contract!(
            FlushError::InProgress,
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
        assert_operation_contract!(
            FlushError::NotRunning {
                phase: LifecyclePhase::Stopped,
            },
            error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
        );

        assert_operation_contract!(
            ShutdownError::TimedOut {
                timeout: Duration::from_millis(1),
            },
            error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT
        );
        assert_operation_contract!(
            ShutdownError::FinalFlush {
                diagnostic: diagnostic(),
            },
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
        );
        assert_operation_contract!(
            ShutdownError::HelperSpawn {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED
        );
        assert_operation_contract!(
            ShutdownError::HelperLost {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST
        );

        assert_operation_contract!(
            EmitError::InvalidField {
                raw_key: "invalid".to_owned(),
                reason: FieldKeyError::Collision {
                    other_raw_key: "other".to_owned(),
                },
            },
            error_codes::SC_OBSERVABILITY_LOG_INVALID_FIELD
        );
        for error in [
            EmitError::InvalidEvent {
                diagnostic: diagnostic(),
            },
            EmitError::QueueFull {
                diagnostic: diagnostic(),
            },
            EmitError::WriterDegraded {
                diagnostic: diagnostic(),
            },
            EmitError::ShutdownTimedOut {
                diagnostic: diagnostic(),
            },
        ] {
            assert_eq!(
                error.code(),
                ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
            );
            assert_non_empty(&error.remediation());
            assert_native_error_contract(&error);
        }
        assert_operation_contract!(
            EmitError::NotRunning {
                phase: LifecyclePhase::Stopped,
            },
            error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
        );
        assert_operation_contract!(
            EmitError::Reentrant,
            error_codes::SC_OBSERVABILITY_LOG_REENTRANT_EMIT
        );
        assert_operation_contract!(
            EmitError::Panicked,
            error_codes::SC_OBSERVABILITY_LOG_LOGGER_PANICKED
        );

        assert_operation_contract!(
            ControlError::NotRunning {
                phase: LifecyclePhase::Stopped,
            },
            error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
        );
        assert_operation_contract!(
            ControlError::Query {
                diagnostic: diagnostic(),
            },
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC")
        );
        assert_operation_contract!(
            ControlError::Unavailable {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE
        );

        assert_operation_contract!(
            WaitError::NotStarted,
            error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_NOT_STARTED
        );
        assert_operation_contract!(
            WaitError::TimedOut {
                timeout: Duration::from_millis(1),
            },
            error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT
        );
        assert_operation_contract!(
            WaitError::Unavailable {
                diagnostic: diagnostic(),
            },
            error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE
        );
    }

    #[test]
    fn drop_cause_all_lists_every_variant_once() {
        let unique: std::collections::HashSet<_> = DropCause::ALL.iter().collect();
        assert_eq!(unique.len(), DropCause::ALL.len());
    }
}
