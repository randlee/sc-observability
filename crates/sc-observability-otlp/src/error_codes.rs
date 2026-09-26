//! Types-owned OTLP error-code re-exports.
//!
//! The canonical inventory lives in `sc-observability-types`; this transport
//! crate intentionally has no independent registry or string literals.

pub use sc_observability_types::error_codes::otlp::*;

// Retained 1.x adapter aliases. The pre-existing `typed::*Failure`
// constructors in sc-observability-types deliberately retain these literals
// through D.18; D.21's new transport validation never uses them.
use sc_observability_types::ErrorCode;

/// Retained legacy code for telemetry use after shutdown.
pub const TELEMETRY_SHUTDOWN: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN");
/// Retained legacy code for invalid telemetry configuration.
pub const TELEMETRY_INVALID_CONFIG: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INVALID_CONFIG");
/// Retained legacy code for an invalid OTLP protocol selection.
pub const TELEMETRY_INVALID_PROTOCOL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL");
/// Retained legacy code for an exporter failure.
pub const TELEMETRY_EXPORT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_EXPORT_FAILED");
/// Retained legacy code for a telemetry flush failure.
pub const TELEMETRY_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_FLUSH_FAILED");
/// Retained legacy code for exporter initialization failure.
pub const TELEMETRY_EXPORTER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED");
/// Retained legacy code for incomplete spans dropped during shutdown.
pub const TELEMETRY_INCOMPLETE_SPAN_DROPPED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED");
/// Retained legacy code for span-assembly failure.
pub const TELEMETRY_SPAN_ASSEMBLY_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED");
