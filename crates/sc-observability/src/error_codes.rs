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
