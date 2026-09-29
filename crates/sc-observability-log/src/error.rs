//! Public error enums and the dropped-event cause.
//!
//! Every error type is a discriminated union: callers `match` on the variant and
//! read its typed fields. Each public error enum also exposes a stable
//! [`ErrorCode`] and a mandatory [`Remediation`] per variant; variants wrapping an
//! sc-observability error return that error's own code and remediation. Each
//! remains a directly serializable data contract, so consumers never parse a
//! display string.

use sc_observability_types::{ErrorCode, ErrorContext, Remediation};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error_codes;

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
#[non_exhaustive]
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
    /// The saved attachment no longer occupies the bridge slot.
    #[error("logger attachment is not installed")]
    NotInstalled,
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
            Self::NotInstalled => error_codes::SC_LOG_DETACH_NOT_INSTALLED,
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
            Self::NotInstalled => Remediation::recoverable(
                "the saved attachment no longer occupies the bridge slot",
                [
                    "discard the stale control",
                    "obtain a control from the current attachment",
                ],
            ),
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

/// Canonical operation errors used by the public bridge API.
pub use sc_observability_types::{FlushError, InitError, ShutdownError};

/// Build the original operation context carried by the canonical error families.
pub(crate) fn operation_context(
    code: ErrorCode,
    message: impl Into<String>,
    remediation: Remediation,
) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(code, message, remediation))
}

pub(crate) fn operation_context_with_source<E>(
    code: ErrorCode,
    message: impl Into<String>,
    remediation: Remediation,
    source: E,
) -> Box<ErrorContext>
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(ErrorContext::new(code, message, remediation).source(Box::new(source)))
}

pub(crate) fn init_configuration(
    code: ErrorCode,
    message: impl Into<String>,
    remediation: Remediation,
) -> InitError {
    InitError::Configuration {
        context: operation_context(code, message, remediation),
    }
}

pub(crate) fn init_runtime(
    code: ErrorCode,
    message: impl Into<String>,
    remediation: Remediation,
) -> InitError {
    InitError::Runtime {
        context: operation_context(code, message, remediation),
    }
}

pub(crate) fn flush_drain(context: Box<ErrorContext>) -> FlushError {
    FlushError::Drain { context }
}

pub(crate) fn shutdown_timeout(context: Box<ErrorContext>) -> ShutdownError {
    ShutdownError::Timeout { context }
}

pub(crate) fn shutdown_drain(context: Box<ErrorContext>) -> ShutdownError {
    ShutdownError::Drain { context }
}

/// Why an event was dropped on the non-blocking emit path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropCause {
    /// The bounded writer queue was full.
    QueueFull,
    /// An invalid event or a runtime label/field key failing the sanitizer.
    InvalidEvent,
    /// The writer thread is degraded.
    WriterDegraded,
    /// The logger exceeded its shutdown threshold.
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

    fn projected(code: &'static str) -> sc_observability_types::OperationDiagnostic {
        sc_observability_types::OperationDiagnostic {
            code: ErrorCode::new_static(code),
            message: "projected diagnostic".to_owned(),
            remediation: Remediation::recoverable("follow the diagnostic", ["then retry"]),
            at: sc_observability_types::Timestamp::now_utc(),
        }
    }

    fn assert_non_empty(remediation: &Remediation) {
        match remediation {
            Remediation::Recoverable { steps } => {
                assert!(!steps.steps().is_empty());
                assert!(steps.steps().iter().all(|step| !step.is_empty()));
            }
            Remediation::NotRecoverable { justification } => assert!(!justification.is_empty()),
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
    fn canonical_operation_errors_preserve_context_source_and_codes() {
        let context = ErrorContext::new(
            ErrorCode::new_static("SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT"),
            "flush deadline elapsed",
            Remediation::recoverable("retry explicitly", ["check writer health"]),
        )
        .source(Box::new(std::io::Error::other("native flush source")));
        let flush = FlushError::Drain {
            context: Box::new(context),
        };
        assert_eq!(
            flush.diagnostic().code.as_str(),
            "SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT"
        );
        assert_eq!(
            std::error::Error::source(&flush).unwrap().to_string(),
            "native flush source"
        );
        assert!(
            std::error::Error::source(&flush)
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .is_some()
        );
        let init = init_configuration(
            error_codes::SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL,
            "configured baseline exceeds static level cap",
            Remediation::not_recoverable("choose an executable-supported baseline"),
        );
        assert_eq!(
            init.diagnostic().code,
            error_codes::SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL
        );
        let shutdown = shutdown_timeout(operation_context(
            error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
            "shutdown deadline elapsed",
            Remediation::recoverable("observe the existing shutdown", ["wait again"]),
        ));
        assert_eq!(
            shutdown.diagnostic().code,
            error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT
        );
    }

    #[test]
    fn stale_attachment_errors_are_typed_and_round_trip() {
        assert_operation_contract!(
            EmitError::NotInstalled,
            error_codes::SC_LOG_DETACH_NOT_INSTALLED
        );
        let flush = FlushError::Drain {
            context: operation_context(
                error_codes::SC_LOG_DETACH_NOT_INSTALLED,
                "logger attachment is not installed",
                Remediation::not_recoverable("install or attach a logger before flushing"),
            ),
        };
        assert_eq!(
            flush.diagnostic().code,
            error_codes::SC_LOG_DETACH_NOT_INSTALLED
        );
    }

    #[test]
    fn remaining_facade_errors_keep_their_diagnostic_contracts() {
        let native = || projected("SC_OBSERVABILITY_LOG_NATIVE_DIAGNOSTIC");
        let errors = [
            EmitError::InvalidField {
                raw_key: "bad".to_owned(),
                reason: FieldKeyError::Empty,
            },
            EmitError::InvalidEvent {
                diagnostic: native(),
            },
            EmitError::QueueFull {
                diagnostic: native(),
            },
            EmitError::WriterDegraded {
                diagnostic: native(),
            },
            EmitError::ShutdownTimedOut {
                diagnostic: native(),
            },
            EmitError::NotRunning {
                phase: LifecyclePhase::Stopped,
            },
            EmitError::Reentrant,
            EmitError::Panicked,
        ];
        for error in errors {
            assert!(!error.code().as_str().is_empty());
            assert_non_empty(&error.remediation());
            assert_native_error_contract(&error);
        }

        let control_errors = [
            ControlError::NotRunning {
                phase: LifecyclePhase::Stopped,
            },
            ControlError::Query {
                diagnostic: native(),
            },
            ControlError::Unavailable {
                diagnostic: native(),
            },
        ];
        for error in control_errors {
            assert!(!error.code().as_str().is_empty());
            assert_non_empty(&error.remediation());
            assert_native_error_contract(&error);
        }

        let wait_errors = [
            WaitError::NotStarted,
            WaitError::TimedOut {
                timeout: Duration::from_millis(1),
            },
            WaitError::Unavailable {
                diagnostic: native(),
            },
        ];
        for error in wait_errors {
            assert!(!error.code().as_str().is_empty());
            assert_non_empty(&error.remediation());
            assert_native_error_contract(&error);
        }
    }

    #[test]
    fn drop_cause_all_lists_every_variant_once() {
        let unique: std::collections::HashSet<_> = DropCause::ALL.iter().collect();
        assert_eq!(unique.len(), DropCause::ALL.len());
    }
}
