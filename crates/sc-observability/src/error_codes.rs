//! Stable `ErrorCode` registry for `sc-observability`.

use sc_observability_types::ErrorCode;

/// An event payload failed validation before logger admission.
/// Recovery: provide an event with valid fields and values.
pub const LOGGER_INVALID_EVENT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INVALID_EVENT");
/// The logger was used after shutdown began or completed.
/// Recovery: stop emitting to the closed owner and construct a new logger.
pub const LOGGER_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SHUTDOWN");
/// A configured sink rejected or failed to write an admitted event.
/// Recovery: inspect sink health and its I/O configuration before retrying.
pub const LOGGER_SINK_WRITE_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED");
/// The non-blocking admission queue was full.
/// Recovery: preserve fail-open behavior, inspect health, and retry later if appropriate.
pub const LOGGER_QUEUE_FULL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_QUEUE_FULL");
/// The writer thread is degraded and cannot provide normal sink service.
/// Recovery: inspect the retained failure and construct a new logger owner.
pub const LOGGER_WRITER_DEGRADED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED");
/// Logger shutdown exceeded its configured timeout threshold.
/// Recovery: inspect retained shutdown health and use a fresh owner after recovery.
pub const LOGGER_SHUTDOWN_TIMED_OUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT");
/// Logger initialization failed before the owner became operational.
/// Recovery: correct the reported configuration or environment and initialize again.
pub const LOGGER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INIT_FAILED");
/// A logger flush failed while draining admitted events.
/// Recovery: inspect sink and writer health, then retry with a live owner.
pub const LOGGER_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED");
/// Retained-log maintenance failed while pruning or rotating storage.
/// Recovery: correct storage access or retention configuration and retry maintenance.
pub const LOGGER_MAINTENANCE_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED");
/// The maintenance worker did not join before the shutdown deadline.
/// Recovery: inspect worker health and complete shutdown with a bounded retry.
pub const LOGGER_MAINTENANCE_JOIN_TIMEOUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_JOIN_TIMEOUT");
/// The maintenance worker terminated with a failure.
/// Recovery: inspect the retained worker error and construct a new logger owner.
pub const LOGGER_MAINTENANCE_WORKER_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_WORKER_FAILED");
/// A retained sink deliberately injected a test failure.
/// Recovery: disable fault injection outside the test and retry the operation.
#[cfg(feature = "fault-injection")]
pub const LOGGER_SINK_FAULT_INJECTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED");

/// All stable error codes exported by this crate.
pub const ALL: &[ErrorCode] = &[
    LOG_PREFIX_COLLISION,
    LOG_INVALID_ENVIRONMENT,
    LOG_UNKNOWN_KEY,
    LOG_INVALID_VALUE,
    LOG_RESOLUTION,
    SC_LOG_SINK_REGISTRATION_DUPLICATE,
    SC_LOG_SINK_REGISTRATION_INVALID,
    SC_LOG_SINK_REGISTRATION_CLOSED,
    LOGGER_INVALID_EVENT,
    LOGGER_SHUTDOWN,
    LOGGER_SINK_WRITE_FAILED,
    LOGGER_QUEUE_FULL,
    LOGGER_WRITER_DEGRADED,
    LOGGER_SHUTDOWN_TIMED_OUT,
    LOGGER_INIT_FAILED,
    LOGGER_FLUSH_FAILED,
    LOGGER_MAINTENANCE_FAILED,
    LOGGER_MAINTENANCE_JOIN_TIMEOUT,
    LOGGER_MAINTENANCE_WORKER_FAILED,
    #[cfg(feature = "fault-injection")]
    LOGGER_SINK_FAULT_INJECTED,
];

/// A sink registration reused an existing sink identity.
/// Recovery: register each sink once or use a distinct identity.
pub const SC_LOG_SINK_REGISTRATION_DUPLICATE: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_DUPLICATE");
/// Sink registration metadata or configuration failed validation.
/// Recovery: provide a valid registration with the required metadata.
pub const SC_LOG_SINK_REGISTRATION_INVALID: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_INVALID");
/// A sink registration was attempted after registration closed.
/// Recovery: register before closure or create a new logger owner.
pub const SC_LOG_SINK_REGISTRATION_CLOSED: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_CLOSED");

/// Legacy settings diagnostics retain the `LOG-001`..`LOG-005` spellings;
/// they are stable settings codes, not `SC_OBSERVABILITY_*` requirement IDs.
/// A requested log prefix collides with an existing environment prefix.
/// Recovery: choose a unique log prefix or rename the conflicting setting.
pub const LOG_PREFIX_COLLISION: ErrorCode = ErrorCode::new_static("LOG-001");
/// The log environment configuration is invalid or incomplete.
/// Recovery: provide a valid environment prefix and supported values.
pub const LOG_INVALID_ENVIRONMENT: ErrorCode = ErrorCode::new_static("LOG-002");
/// The log settings contain an unsupported key.
/// Recovery: remove the unknown key or use a documented setting name.
pub const LOG_UNKNOWN_KEY: ErrorCode = ErrorCode::new_static("LOG-003");
/// A log setting contains a value outside its accepted domain.
/// Recovery: replace it with a documented value valid for that setting.
pub const LOG_INVALID_VALUE: ErrorCode = ErrorCode::new_static("LOG-004");
/// Log settings could not be resolved from the configured inputs.
/// Recovery: correct the reported inputs and retry settings resolution.
pub const LOG_RESOLUTION: ErrorCode = ErrorCode::new_static("LOG-005");

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::ALL;

    #[test]
    fn registry_codes_are_unique() {
        let unique = ALL
            .iter()
            .map(sc_observability_types::ErrorCode::as_str)
            .collect::<HashSet<_>>();
        assert_eq!(unique.len(), ALL.len());
    }
}
