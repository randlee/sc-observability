//! Stable `ErrorCode` registry for `sc-observability-types`.

use crate::ErrorCode;

/// A shared value failed validation before it could enter a neutral contract.
/// Recovery: provide a value that satisfies the named type's validation rules.
pub const VALUE_VALIDATION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED");
/// A W3C trace identifier is malformed or has an invalid zero value.
/// Recovery: provide a valid nonzero 32-hex-character trace identifier.
pub const TRACE_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_TRACE_ID_INVALID");
/// A W3C span identifier is malformed or has an invalid zero value.
/// Recovery: provide a valid nonzero 16-hex-character span identifier.
pub const SPAN_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_SPAN_ID_INVALID");
/// Process identity could not be resolved from the configured policy.
/// Recovery: provide a valid fixed identity or a resolver that returns one.
pub const IDENTITY_RESOLUTION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED");
/// A diagnostic context is missing required data or violates its validation rules.
/// Recovery: construct the context with a stable code, message, and remediation.
pub const DIAGNOSTIC_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID");
/// Historical or follow query bounds and filters are invalid.
/// Recovery: provide a supported range, limit, and filter combination.
pub const SC_LOG_QUERY_INVALID_QUERY: ErrorCode =
    ErrorCode::new_static("SC_LOG_QUERY_INVALID_QUERY");
/// The query could not read its configured log storage.
/// Recovery: verify the log path and its read permissions, then retry.
pub const SC_LOG_QUERY_IO: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_IO");
/// Stored log data could not be decoded as a valid event.
/// Recovery: repair or remove the malformed record and retry the query.
pub const SC_LOG_QUERY_DECODE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_DECODE");
/// Query access is unavailable because the required log storage is not configured.
/// Recovery: configure a file-backed log sink before querying.
pub const SC_LOG_QUERY_UNAVAILABLE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_UNAVAILABLE");
/// A query was attempted after the logger query service shut down.
/// Recovery: retain a live logger owner or construct a new logger instance.
pub const SC_LOG_QUERY_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_SHUTDOWN");
/// A level mutation was attempted while logger shutdown is in progress.
/// Recovery: wait for shutdown to finish or use a new logger instance.
pub const LEVEL_STOPPING: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPING");
/// A level mutation was attempted after the logger lifetime ended.
/// Recovery: construct a new logger owner before changing its level.
pub const LEVEL_STOPPED: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPED");
/// A requested level is below the configured baseline.
/// Recovery: request a level at or above the configured baseline.
pub const LEVEL_BELOW_BASELINE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_BELOW_BASELINE");
/// A requested level is unavailable in the current build.
/// Recovery: select a compiled level or enable the required build support.
pub const LEVEL_UNSUPPORTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_UNSUPPORTED");
/// Runtime level state is unavailable or poisoned.
/// Recovery: discard the affected logger owner and construct a fresh instance.
pub const LEVEL_STATE_UNAVAILABLE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE");
/// The runtime-level revision reached its maximum value.
/// Recovery: construct a fresh logger owner with a new revision sequence.
pub const LEVEL_REVISION_EXHAUSTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED");

/// Enumerable registry of all public `sc-observability-types` error codes.
pub const ALL: &[ErrorCode] = &[
    SC_METRIC_INVALID_HISTOGRAM,
    SC_METRIC_INVALID_TEMPORALITY,
    SC_METRIC_INVALID_INTERVAL,
    SC_METRIC_NON_FINITE,
    otlp::OTLP_CONFIG_ZERO_DURATION,
    otlp::OTLP_CONFIG_DURATION_OVERFLOW,
    otlp::OTLP_CONFIG_BOUND_ORDER,
    otlp::OTLP_CONFIG_JITTER_PERCENT,
    otlp::OTLP_CONFIG_QUEUE_CAPACITY,
    otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
    otlp::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
    otlp::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
    otlp::OTLP_CONFIG_INVALID_ENDPOINT,
    otlp::OTLP_CONFIG_INVALID_HEADER,
    otlp::OTLP_CONFIG_INVALID,
