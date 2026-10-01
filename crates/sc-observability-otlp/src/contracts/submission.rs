//! The durable drain/exporter boundary shared by D33 and D34.
use sc_observability_types::{
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by durable-store")
)]
pub(crate) trait SubmissionExporter: Send + Sync {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure>;
}
#[derive(Debug)]
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by durable-store")
)]
pub(crate) enum SubmissionExportFailure {
    Retryable(ExportError),
    Terminal(ExportError),
}
