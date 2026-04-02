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
];
