//! Stable `ErrorCode` registry for `sc-observability-otlp`.

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

/// Enumerable registry of all public `sc-observability-otlp` error codes.
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
