//! Stable `ErrorCode` registry for `sc-observability`.

use sc_observability_types::ErrorCode;

/// Stable error code for invalid event payloads.
pub const LOGGER_INVALID_EVENT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INVALID_EVENT");
/// Stable error code for post-shutdown logger use.
pub const LOGGER_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SHUTDOWN");
/// Stable error code for sink write failures.
pub const LOGGER_SINK_WRITE_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED");
/// Stable error code for non-blocking queue admission failure.
pub const LOGGER_QUEUE_FULL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_QUEUE_FULL");
/// Stable error code for writer-thread degradation.
pub const LOGGER_WRITER_DEGRADED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED");
/// Stable error code for shutdown exceeding the configured timeout threshold.
pub const LOGGER_SHUTDOWN_TIMED_OUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT");
/// Stable error code for logger initialization failures.
pub const LOGGER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INIT_FAILED");
/// Stable error code for flush failures.
pub const LOGGER_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED");
/// Stable error code for retained-log maintenance failures.
pub const LOGGER_MAINTENANCE_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED");
/// Stable error code for maintenance-worker shutdown join timeouts.
pub const LOGGER_MAINTENANCE_JOIN_TIMEOUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_JOIN_TIMEOUT");
/// Stable error code for maintenance-worker thread failures.
pub const LOGGER_MAINTENANCE_WORKER_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_MAINTENANCE_WORKER_FAILED");
/// Stable error code for deliberate retained-sink fault injection.
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

/// Canonical sc log sink registration duplicate failure.
pub const SC_LOG_SINK_REGISTRATION_DUPLICATE: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_DUPLICATE");
/// Canonical sc log sink registration invalid failure.
pub const SC_LOG_SINK_REGISTRATION_INVALID: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_INVALID");
/// Canonical sc log sink registration closed failure.
pub const SC_LOG_SINK_REGISTRATION_CLOSED: ErrorCode =
    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_CLOSED");

/// Settings diagnostic for log prefix collision.
pub const LOG_PREFIX_COLLISION: ErrorCode = ErrorCode::new_static("LOG-001");
/// Settings diagnostic for log invalid environment.
pub const LOG_INVALID_ENVIRONMENT: ErrorCode = ErrorCode::new_static("LOG-002");
/// Settings diagnostic for log unknown key.
pub const LOG_UNKNOWN_KEY: ErrorCode = ErrorCode::new_static("LOG-003");
/// Settings diagnostic for log invalid value.
pub const LOG_INVALID_VALUE: ErrorCode = ErrorCode::new_static("LOG-004");
/// Settings diagnostic for log resolution.
pub const LOG_RESOLUTION: ErrorCode = ErrorCode::new_static("LOG-005");
