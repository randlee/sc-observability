//! Stable `ErrorCode` registry for `sc-observability-log`.
//!
//! Every crate-owned public error variant maps to exactly one code here. Variants
//! that wrap an sc-observability error return that error's own code instead.

use sc_observability_types::ErrorCode;

/// Canonical init configuration error: the bridge was already installed in this process.
/// Recovery: reuse the installed bridge or detach it before installing another.
pub const SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED");
/// Canonical init configuration error: another `log::Log` implementation owns the facade.
/// Recovery: remove the competing logger or use the owner that installed it.
pub const SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED");
/// `InitError::IdentityResolution`: the identity resolver failed or `Auto` found no hostname.
/// Recovery: provide a fixed identity or a resolver that returns a hostname.
pub const SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED");
/// Canonical flush diagnostic: the writer did not acknowledge a flush within the timeout.
/// Recovery: inspect writer health, then retry with a live logger and bounded timeout.
pub const SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT");
/// Canonical shutdown diagnostic: shutdown did not finish within the timeout.
/// Recovery: inspect retained shutdown health and construct a new owner if needed.
pub const SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT");
/// Canonical flush/shutdown diagnostic: the helper thread could not start.
/// Recovery: provide available thread resources and retry the bounded operation.
pub const SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED");
/// Canonical flush/shutdown diagnostic: the helper thread ended without a result.
/// Recovery: inspect the retained failure and construct a new logger owner.
pub const SC_OBSERVABILITY_LOG_HELPER_LOST: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_HELPER_LOST");
/// Canonical init configuration diagnostic: the executable's static facade cap cannot retain the baseline.
/// Recovery: choose a baseline supported by the executable's static facade cap.
pub const SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL");
/// Direct runtime setup failed before bridge admission.
/// Recovery: correct the runtime configuration and initialize again.
pub const SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED");
/// A direct/control request arrived outside the running lifecycle.
/// Recovery: issue the request while the logger lifecycle is running.
pub const SC_OBSERVABILITY_LOG_NOT_RUNNING: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_NOT_RUNNING");
/// A typed direct event supplied an invalid field key.
/// Recovery: use a nonempty field key that satisfies the key validation rules.
pub const SC_OBSERVABILITY_LOG_INVALID_FIELD: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_INVALID_FIELD");
/// A producer re-entered the guarded bridge submission path.
/// Recovery: avoid recursive emission from inside the guarded submission path.
pub const SC_OBSERVABILITY_LOG_REENTRANT_EMIT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_REENTRANT_EMIT");
/// A callback panic was contained by the bridge admission boundary.
/// Recovery: fix the callback panic and submit again after the boundary recovers.
pub const SC_OBSERVABILITY_LOG_LOGGER_PANICKED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_LOGGER_PANICKED");
/// A read-only state/query observation is unavailable.
/// Recovery: keep the logger owner alive and retry the observation.
pub const SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE");
/// A wait request was made before initialization.
/// Recovery: initialize the logger before waiting for shutdown.
pub const SC_OBSERVABILITY_LOG_SHUTDOWN_NOT_STARTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_SHUTDOWN_NOT_STARTED");
/// Canonical flush diagnostic: a previous flush helper is still running; no new flush was started.
/// Recovery: await the existing flush operation before requesting another.
pub const SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS");

/// A host bridge policy rejected an assembled event before logger admission.
/// Recovery: follow the policy-specific diagnostic steps and resubmit explicitly.
pub const SC_OBSERVABILITY_LOG_POLICY_REJECTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOG_POLICY_REJECTED");

/// Every code defined by this crate, in declaration order.
pub const ALL: &[ErrorCode] = &[
    SC_OBSERVABILITY_LOG_POLICY_REJECTED,
    SC_LOG_DETACH_TIMEOUT,
    SC_LOG_DETACH_NOT_INSTALLED,
    SC_LOG_FOREIGN_LOGGER_INSTALLED,
    SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED,
    SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED,
    SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED,
    SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT,
    SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
    SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
    SC_OBSERVABILITY_LOG_HELPER_LOST,
    SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL,
    SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED,
    SC_OBSERVABILITY_LOG_NOT_RUNNING,
    SC_OBSERVABILITY_LOG_INVALID_FIELD,
    SC_OBSERVABILITY_LOG_REENTRANT_EMIT,
    SC_OBSERVABILITY_LOG_LOGGER_PANICKED,
    SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE,
    SC_OBSERVABILITY_LOG_SHUTDOWN_NOT_STARTED,
    SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS,
];

/// The installed logger did not detach before the bounded deadline.
/// Recovery: wait for in-flight logging to finish, then retry when the logger is quiescent.
pub const SC_LOG_DETACH_TIMEOUT: ErrorCode = ErrorCode::new_static("SC_LOG_DETACH_TIMEOUT");
/// Detach was requested without an installed bridge.
/// Recovery: install the bridge before detaching, or treat the bridge as already detached.
pub const SC_LOG_DETACH_NOT_INSTALLED: ErrorCode =
    ErrorCode::new_static("SC_LOG_DETACH_NOT_INSTALLED");
/// `DetachError::ForeignLoggerInstalled`: another logger owner or attachment occupies the
/// global logging facade.
///
/// This is intentionally distinct from
/// [`SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED`]: the latter reports an
/// init-time attempt to install the owned bridge, whereas this code reports a
/// non-owning attachment's occupied-facade rejection.
/// Recovery: use the active logger owner or detach its bridge before attaching.
pub const SC_LOG_FOREIGN_LOGGER_INSTALLED: ErrorCode =
    ErrorCode::new_static("SC_LOG_FOREIGN_LOGGER_INSTALLED");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codes_are_unique_and_prefixed() {
        let mut seen = std::collections::HashSet::new();
        for code in ALL {
            assert!(
                code.as_str().starts_with("SC_OBSERVABILITY_LOG_")
                    || code.as_str().starts_with("SC_LOG_")
            );
            assert!(seen.insert(code.as_str()), "duplicate code {code}");
        }
        assert_eq!(ALL.len(), 20);
    }

    #[test]
    fn foreign_logger_codes_have_distinct_lifecycle_contracts() {
        assert_eq!(
            SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED.as_str(),
            "SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED"
        );
        assert_eq!(
            SC_LOG_FOREIGN_LOGGER_INSTALLED.as_str(),
            "SC_LOG_FOREIGN_LOGGER_INSTALLED"
        );
        assert_ne!(
            SC_OBSERVABILITY_LOG_FOREIGN_LOGGER_INSTALLED,
            SC_LOG_FOREIGN_LOGGER_INSTALLED
        );
    }
}
