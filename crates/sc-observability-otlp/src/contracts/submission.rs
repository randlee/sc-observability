//! The durable drain/exporter boundary shared by D33 and D34.
use sc_observability_types::{
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
pub(crate) trait SubmissionExporter: Send + Sync {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure>;

    /// Interrupt an in-flight submission during durable-client shutdown.
    ///
    /// This crate-private default preserves the test seam for exporters that
    /// have no cancellable transport operation.
    fn cancel(&self) {}
}
#[derive(Debug)]
pub(crate) enum SubmissionExportFailure {
    Retryable(ExportError),
    Terminal(ExportError),
}
