//! Stable `ErrorCode` registry for `sc-observability-types`.

use crate::ErrorCode;

/// Generic validation error code used for shared value-type failures.
pub const VALUE_VALIDATION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED");
/// Error code for invalid W3C trace identifier values.
pub const TRACE_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_TRACE_ID_INVALID");
/// Error code for invalid W3C span identifier values.
pub const SPAN_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_SPAN_ID_INVALID");
/// Error code for process identity resolution failures.
pub const IDENTITY_RESOLUTION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED");
/// Error code for generic diagnostic construction or validation failures.
pub const DIAGNOSTIC_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID");
/// Error code for invalid historical/follow query inputs.
pub const SC_LOG_QUERY_INVALID_QUERY: ErrorCode =
    ErrorCode::new_static("SC_LOG_QUERY_INVALID_QUERY");
/// Error code for query I/O failures.
pub const SC_LOG_QUERY_IO: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_IO");
/// Error code for query decode failures.
pub const SC_LOG_QUERY_DECODE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_DECODE");
/// Error code for query unavailability failures.
pub const SC_LOG_QUERY_UNAVAILABLE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_UNAVAILABLE");
/// Error code for query shutdown failures.
pub const SC_LOG_QUERY_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_SHUTDOWN");
/// Error code for a mutation attempted while logger shutdown is in progress.
pub const LEVEL_STOPPING: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPING");
/// Error code for a mutation attempted after the logger lifetime ended.
pub const LEVEL_STOPPED: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPED");
/// Error code for a requested level below the configured baseline.
pub const LEVEL_BELOW_BASELINE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_BELOW_BASELINE");
/// Error code for a requested level unavailable in the current build.
pub const LEVEL_UNSUPPORTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_UNSUPPORTED");
/// Error code for unavailable or poisoned runtime level state.
pub const LEVEL_STATE_UNAVAILABLE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE");
/// Error code for a runtime-level revision that cannot be incremented.
pub const LEVEL_REVISION_EXHAUSTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED");

/// Enumerable registry of all public `sc-observability-types` error codes.
pub const ALL: &[ErrorCode] = &[
    VALUE_VALIDATION_FAILED,
    TRACE_ID_INVALID,
    SPAN_ID_INVALID,
    IDENTITY_RESOLUTION_FAILED,
    DIAGNOSTIC_INVALID,
    SC_LOG_QUERY_INVALID_QUERY,
    SC_LOG_QUERY_IO,
    SC_LOG_QUERY_DECODE,
    SC_LOG_QUERY_UNAVAILABLE,
    SC_LOG_QUERY_SHUTDOWN,
    LEVEL_STOPPING,
    LEVEL_STOPPED,
    LEVEL_BELOW_BASELINE,
    LEVEL_UNSUPPORTED,
    LEVEL_STATE_UNAVAILABLE,
    LEVEL_REVISION_EXHAUSTED,
];
