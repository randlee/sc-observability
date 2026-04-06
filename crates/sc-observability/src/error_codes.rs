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
/// Stable error code for logger initialization failures.
pub const LOGGER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INIT_FAILED");
/// Stable error code for flush failures.
pub const LOGGER_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED");
/// Stable error code for deliberate retained-sink fault injection.
#[cfg(feature = "fault-injection")]
pub const LOGGER_SINK_FAULT_INJECTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED");

/// All stable error codes exported by this crate.
pub const ALL: &[ErrorCode] = &[
    LOGGER_INVALID_EVENT,
    LOGGER_SHUTDOWN,
    LOGGER_SINK_WRITE_FAILED,
    LOGGER_INIT_FAILED,
    LOGGER_FLUSH_FAILED,
    #[cfg(feature = "fault-injection")]
    LOGGER_SINK_FAULT_INJECTED,
];
