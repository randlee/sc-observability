//! `sc-observability-otlp` error-code registry (ADR-005).

use sc_observability_types::ErrorCode;

/// Error code for exporter failures during log, trace, or metric export.
pub const TELEMETRY_EXPORT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_EXPORT_FAILED");

/// Enumerable registry of all `sc-observability-otlp` error codes.
pub const ALL: &[ErrorCode] = &[
    TELEMETRY_EXPORT_FAILED,
    ErrorCode::new_static(sync::INVALID_CONFIG),
    ErrorCode::new_static(sync::RUNTIME_ENTERED),
    ErrorCode::new_static(sync::INVALID_RECORD),
    ErrorCode::new_static(sync::INPUT_LIMIT_EXCEEDED),
    ErrorCode::new_static(sync::CALLER_REJECTED),
];

/// Validation codes carried by the synchronous client's
/// `SyncError::Validation` variant.
pub mod sync {
    /// The endpoint, a header, the timeout or a root certificate is invalid.
    pub const INVALID_CONFIG: &str = "SC_OBSERVABILITY_OTLP_SYNC_INVALID_CONFIG";
    /// The calling thread is inside an entered Tokio runtime; native Tokio
    /// hosts use the official asynchronous exporters directly.
    pub const RUNTIME_ENTERED: &str = "SC_OBSERVABILITY_OTLP_SYNC_RUNTIME_ENTERED";
    /// A record carries malformed identifiers, an inverted time range or
    /// another value the OTLP data model rejects.
    pub const INVALID_RECORD: &str = "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD";
    /// Frontend input exceeds the per-call byte or record limit.
    pub const INPUT_LIMIT_EXCEEDED: &str = "SC_OBSERVABILITY_OTLP_SYNC_INPUT_LIMIT_EXCEEDED";
    /// A caller-supplied record or measurement closure failed.
    pub const CALLER_REJECTED: &str = "SC_OBSERVABILITY_OTLP_SYNC_CALLER_REJECTED";
}
