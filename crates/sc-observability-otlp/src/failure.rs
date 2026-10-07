//! Canonical failure conversions for telemetry admission, flush and
//! shutdown; each keeps the original error as its native source.

use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
use sc_observability_types::v2::{
    EventError as CanonicalEventError, ExportError, FailureClassification, FlushError,
    ShutdownError,
};
use sc_observability_types::{DiagnosticSummary, ErrorContext, Remediation};
use serde_json::Value;

use crate::error_codes;

/// Converts a canonical span-assembly failure into a telemetry export failure,
/// moving the original error context unchanged.
pub(crate) fn export_failure_from_canonical_event(
    err: CanonicalEventError,
) -> CanonicalTelemetryError {
    CanonicalTelemetryError::ExportFailure(ExportError::Transport {
        context: err.into_context(),
    })
}

/// Preserves a shared-lifecycle failure as the source of a facade flush error.
pub(crate) fn flush_lifecycle_failure(error: ExportError) -> FlushError {
    FlushError::Drain {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_FLUSH_FAILED,
                "the shared telemetry lifecycle did not complete its flush barrier",
                Remediation::recoverable(
                    "inspect the exporter lifecycle and retry after it recovers",
                    ["retry flush"],
                ),
            )
            .source(Box::new(error)),
        ),
    }
}

/// Reports a flush requested after the runtime's terminal shutdown barrier.
pub(crate) fn flush_after_shutdown() -> FlushError {
    FlushError::classified_drain(
        Box::new(ErrorContext::new(
            error_codes::OTLP_TELEMETRY_SHUTDOWN,
            "telemetry runtime is shut down",
            Remediation::not_recoverable("do not flush telemetry after shutdown"),
        )),
        FailureClassification::Closed,
    )
}

pub(crate) fn shutdown_export_failure_typed(
    error: ExportError,
    diagnostic_summary: Option<DiagnosticSummary>,
) -> ShutdownError {
    // The legacy shutdown path selected `runtime.last_error` after incomplete
    // span accounting. Preserve that diagnostic selection exactly, while the
    // typed source chain keeps the actual exporter failure available to callers.
    let summary = diagnostic_summary.unwrap_or_else(|| DiagnosticSummary::from(error.diagnostic()));
    let mut context = ErrorContext::new(
        error_codes::OTLP_FLUSH_FAILED,
        "failed to flush telemetry during shutdown",
        Remediation::recoverable(
            "inspect telemetry health and retry shutdown after the exporter recovers",
            ["retry shutdown"],
        ),
    );
    context = context.cause(summary.message);
    if let Some(code) = summary.code {
        context = context.detail(
            "exporter_error_code",
            Value::String(code.as_str().to_owned()),
        );
    }
    ShutdownError::Drain {
        context: Box::new(context.source(Box::new(error))),
    }
}
