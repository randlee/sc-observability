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

/// Enumerable registry of all public `sc-observability-types` error codes.
pub const ALL: &[ErrorCode] = &[
    VALUE_VALIDATION_FAILED,
    TRACE_ID_INVALID,
    SPAN_ID_INVALID,
    IDENTITY_RESOLUTION_FAILED,
    DIAGNOSTIC_INVALID,
];
