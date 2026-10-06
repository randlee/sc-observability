//! Released OTLP error-code surface plus canonical internal re-exports.

use sc_observability_types::ErrorCode;

/// Error code for telemetry use after shutdown.
pub const TELEMETRY_SHUTDOWN: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN");
/// Error code for invalid telemetry configuration.
pub const TELEMETRY_INVALID_CONFIG: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INVALID_CONFIG");
/// Error code for invalid OTLP protocol selection.
pub const TELEMETRY_INVALID_PROTOCOL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL");
/// Error code for exporter failures during log, trace, or metric export.
pub const TELEMETRY_EXPORT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_EXPORT_FAILED");
/// Error code for flush-time export failures.
pub const TELEMETRY_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_FLUSH_FAILED");
/// Error code for exporter initialization failures.
pub const TELEMETRY_EXPORTER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED");
/// Error code for incomplete spans dropped during shutdown.
pub const TELEMETRY_INCOMPLETE_SPAN_DROPPED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED");
/// Error code for span assembly failures before export.
pub const TELEMETRY_SPAN_ASSEMBLY_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED");

/// Enumerable registry of all released `sc-observability-otlp` error codes.
pub const ALL: &[ErrorCode] = &[
    TELEMETRY_SHUTDOWN,
    TELEMETRY_INVALID_CONFIG,
    TELEMETRY_INVALID_PROTOCOL,
    TELEMETRY_EXPORT_FAILED,
    TELEMETRY_FLUSH_FAILED,
    TELEMETRY_EXPORTER_INIT_FAILED,
    TELEMETRY_INCOMPLETE_SPAN_DROPPED,
    TELEMETRY_SPAN_ASSEMBLY_FAILED,
];

pub(crate) use sc_observability_types::error_codes::otlp::{
    OTLP_CONFIG_BOUND_ORDER, OTLP_CONFIG_FIELD_NOT_APPLICABLE,
    OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED, OTLP_CONFIG_INVALID_ENDPOINT,
    OTLP_CONFIG_INVALID_HEADER, OTLP_CONFIG_JITTER_PERCENT, OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
    OTLP_CONFIG_QUEUE_CAPACITY, OTLP_CONFIG_ZERO_DURATION, OTLP_FLUSH_FAILED,
    OTLP_INCOMPLETE_SPAN_DROPPED, OTLP_SPAN_ASSEMBLY_FAILED, OTLP_TELEMETRY_SHUTDOWN,
    OTLP_TRANSPORT_CONSTRUCTION_FAILED,
};

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
pub(crate) use sc_observability_types::error_codes::otlp::{
    OTLP_EXPORT_TERMINAL, OTLP_LIFECYCLE_TIMEOUT, OTLP_QUEUE_FULL,
};

#[cfg(test)]
pub(crate) use sc_observability_types::error_codes::otlp::OTLP_RUNTIME_TERMINATED;

/// Internal durable-store diagnostic; no new public API.
#[cfg(feature = "durable-store")]
pub(crate) const DURABLE_OVERSIZE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_DURABLE_RECORD_TOO_LARGE");

/// Internal durable-store diagnostic; no new public API.
#[cfg(feature = "durable-store")]
pub(crate) const DURABLE_CORRUPT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_DURABLE_CORRUPT_ENVELOPE");

/// Internal durable-store diagnostic; no new public API.
#[cfg(feature = "durable-store")]
pub(crate) const DURABLE_QUERY: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_DURABLE_UNSUPPORTED_QUERY");

/// Internal durable-store diagnostic; no new public API.
#[cfg(feature = "durable-store")]
pub(crate) const DURABLE_LOCK_TIMEOUT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_DURABLE_LOCK_TIMEOUT");
