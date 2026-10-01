//! Canonical failure conversions for telemetry admission, flush and
//! shutdown; each keeps the original error as its native source.

use sc_observability_types::typed::{FlushFailure, ShutdownFailure};
use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
use sc_observability_types::v2::{EventError as CanonicalEventError, ExportError};
use sc_observability_types::{DiagnosticSummary, ErrorContext, Remediation};
use serde_json::Value;

use crate::error_codes;

/// Builds a telemetry export failure with the crate-local error code.
#[expect(
    dead_code,
    reason = "crate-local export failure helper is retained for internal construction sites"
)]
pub(crate) fn export_failure(message: impl Into<String>) -> CanonicalTelemetryError {
    CanonicalTelemetryError::ExportFailure(ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
        )),
    })
}

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
pub(crate) fn flush_lifecycle_failure(error: ExportError) -> FlushFailure {
    FlushFailure::telemetry_flush(
        "the shared telemetry lifecycle did not complete its flush barrier",
        Remediation::recoverable(
            "inspect the exporter lifecycle and retry after it recovers",
            ["retry flush"],
        ),
    )
    .source(Box::new(error))
}

/// Converts a flush failure into a shutdown failure, chaining the flush
/// failure as the shutdown context's native source.
///
/// `flush_outcome` never currently returns `Err` (it is intentionally
/// `Result`-shaped so shutdown can propagate real flush failures without a
/// public-signature change later; see its own `unnecessary_wraps` rationale),
/// so this conversion is unreachable at runtime today. It is kept, rather
/// than deleted, for that future propagation path, and is covered directly
/// by `shutdown_flush_failure_preserves_flush_context_as_native_source`
/// below so a regression in its error-context/source chaining is still
/// caught even while the call site is dormant.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn shutdown_flush_failure(error: FlushFailure) -> ShutdownFailure {
    ShutdownFailure::from_context(Box::new(
        ErrorContext::new(
            error_codes::OTLP_FLUSH_FAILED,
            "failed to flush telemetry during shutdown",
            Remediation::recoverable(
                "inspect telemetry health and retry shutdown after the exporter recovers",
                ["retry shutdown"],
            ),
        )
        .source(Box::new(error)),
    ))
}

pub(crate) fn shutdown_export_failure_typed(
    error: ExportError,
    diagnostic_summary: Option<DiagnosticSummary>,
) -> ShutdownFailure {
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
    ShutdownFailure::from_context(Box::new(context.source(Box::new(error))))
}
