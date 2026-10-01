//! The durable drain/exporter boundary shared by D33 and D34.
use sc_observability_types::{
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
#[expect(
    dead_code,
    reason = "staged by d-29; wired by d-33/d-34 under durable-store"
)]
pub(crate) trait SubmissionExporter: Send + Sync {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure>;
}
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "staged by d-29; wired by d-33/d-34 under durable-store"
)]
pub(crate) enum SubmissionExportFailure {
    Retryable(ExportError),
    Terminal(ExportError),
}
