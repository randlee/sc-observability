use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ErrorCode, Remediation, Timestamp, error_codes};

/// Canonical event/log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// Verbose trace-level event.
    Trace,
    /// Debug-level event intended for development or diagnostics.
    Debug,
    /// Informational event for normal operation.
    Info,
    /// Warning event signaling degraded or unexpected behavior.
    Warn,
    /// Error event signaling a failure.
    Error,
}

/// Level threshold used by filtering surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelFilter {
    /// Allow trace, debug, info, warn, and error events.
    Trace,
    /// Allow debug, info, warn, and error events.
    Debug,
    /// Allow info, warn, and error events.
    Info,
    /// Allow warn and error events.
    Warn,
    /// Allow only error events.
    Error,
    /// Disable all events.
    Off,
}

/// Result of applying core admission policy to one valid log event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionOutcome {
    /// The event passed level filtering and entered the writer queue.
    Accepted,
    /// The event was valid but did not meet the effective level threshold.
    Filtered,
}

/// Actor that requested a runtime logging-level change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelChangeSource {
    /// The host application changed its runtime logging policy.
    Application,
    /// An interactive user requested the change.
    UserRequest,
    /// A diagnostic session requested the change.
    DiagnosticSession,
}

/// One immutable snapshot of a logger's configured and effective level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelState {
    /// The level resolved from the logger configuration at construction time.
    pub configured_level: LevelFilter,
    /// The currently effective level after any runtime override.
    pub effective_level: LevelFilter,
    /// Monotonically increasing revision of successful changes.
    pub revision: u64,
}

/// Structured diagnostic retained when the transition diagnostic was not admitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationDiagnostic {
    /// Stable machine-readable failure code.
    pub code: ErrorCode,
    /// Human-readable failure summary.
    pub message: String,
    /// Recovery guidance from the originating failure.
    pub remediation: Remediation,
    /// Time at which the failure was observed.
    pub at: Timestamp,
}

impl std::fmt::Display for OperationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Admission result for the transition's internal diagnostic event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ChangeDiagnostic {
    /// The transition diagnostic entered the writer queue.
    Accepted,
    /// The transition committed but its diagnostic did not enter the queue.
    NotAccepted {
        /// Preserved diagnostic from the failed admission attempt.
        diagnostic: OperationDiagnostic,
    },
}

/// Result of a successful or no-op runtime level request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LevelChange {
    /// The logger committed a different effective level.
    Changed {
        /// Snapshot before the commit.
        previous: LevelState,
        /// Snapshot after the commit.
        current: LevelState,
        /// Actor that requested the change.
        source: LevelChangeSource,
        /// Result of attempting the structured transition diagnostic.
        diagnostic: ChangeDiagnostic,
    },
    /// The requested level was already effective.
    Unchanged {
        /// Current coherent snapshot.
        state: LevelState,
    },
}

/// Failure returned by a runtime level request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LevelChangeError {
    /// Shutdown has started and the change cannot be serialized safely.
    #[error("logger is stopping")]
    Stopping,
    /// The logger is stopped or its weak owner outlived the runtime.
    #[error("logger is stopped")]
    Stopped,
    /// The requested level is less verbose than the configured baseline.
    #[error("requested level is below the configured baseline")]
    BelowBaseline {
        /// Requested effective level.
        requested: LevelFilter,
        /// Immutable configured baseline.
        configured: LevelFilter,
    },
    /// A static build cap cannot support the request.
    #[error("requested level is not supported by this build")]
    UnsupportedLevel {
        /// Requested level.
        requested: LevelFilter,
        /// Most verbose level available in the build.
        available: LevelFilter,
    },
    /// The internal state is unavailable without changing it.
    #[error("{diagnostic}")]
    Unavailable {
        /// Original state or revision failure diagnostic.
        diagnostic: OperationDiagnostic,
    },
}

impl LevelChangeError {
    /// Returns the stable code for this failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Stopping => error_codes::LEVEL_STOPPING,
            Self::Stopped => error_codes::LEVEL_STOPPED,
            Self::BelowBaseline { .. } => error_codes::LEVEL_BELOW_BASELINE,
            Self::UnsupportedLevel { .. } => error_codes::LEVEL_UNSUPPORTED,
            Self::Unavailable { diagnostic } => diagnostic.code.clone(),
        }
    }

    /// Returns recovery guidance for this failure.
    #[must_use]
    pub fn remediation(&self) -> Remediation {
        match self {
            Self::Stopping => Remediation::recoverable(
                "wait for shutdown completion; create a new logger if logging is still needed",
                std::iter::empty::<String>(),
            ),
            Self::Stopped => Remediation::recoverable(
                "create a new logger; do not retry this owner",
                std::iter::empty::<String>(),
            ),
            Self::BelowBaseline { .. } => Remediation::recoverable(
                "request the configured level or greater verbosity",
                std::iter::empty::<String>(),
            ),
            Self::UnsupportedLevel { .. } => Remediation::recoverable(
                "rebuild the application without the conflicting static level cap",
                std::iter::empty::<String>(),
            ),
            Self::Unavailable { diagnostic } => diagnostic.remediation.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_level_values_use_the_native_contract_shapes() {
        let state = LevelState {
            configured_level: LevelFilter::Info,
            effective_level: LevelFilter::Debug,
            revision: 7,
        };
        let change = LevelChange::Changed {
            previous: LevelState {
                effective_level: LevelFilter::Info,
                ..state
            },
            current: state,
            source: LevelChangeSource::UserRequest,
            diagnostic: ChangeDiagnostic::Accepted,
        };

        let encoded = serde_json::to_value(&change).expect("serialize level change");
        assert_eq!(encoded["kind"], "changed");
        assert_eq!(encoded["value"]["source"], "user_request");
        assert_eq!(
            serde_json::from_value::<LevelChange>(encoded).expect("deserialize level change"),
            change
        );
    }

    #[test]
    fn level_change_error_uses_stable_code_and_remediation() {
        let error = LevelChangeError::Stopped;
        assert_eq!(error.code().as_str(), "SC_OBSERVABILITY_LEVEL_STOPPED");
        assert!(matches!(
            error.remediation(),
            Remediation::Recoverable { .. }
        ));
    }

    #[test]
    fn published_level_filter_fixture_retains_its_native_serde_shape() {
        let fixture = include_str!("../tests/fixtures/bp1-v1.2.0/level_filter.json").trim();
        let level: LevelFilter = serde_json::from_str(fixture).expect("published fixture parses");
        assert_eq!(level, LevelFilter::Info);
        assert_eq!(
            serde_json::to_string(&level).expect("serialize level"),
            fixture
        );
    }
}
