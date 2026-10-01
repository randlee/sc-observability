//! Submission exporter contract staged for D34.
pub(crate) use super::implementation::SyncHttpConfig;
use crate::config::ValidatedTransportBounds;
use crate::contracts::submission::{SubmissionExportFailure, SubmissionExporter};
use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
use std::sync::Arc;
/// Retains validated worker configuration until D34 wires the existing exporter.
#[derive(Debug)]
pub(crate) struct SyncHttpSubmissionExporter {
    #[expect(
        dead_code,
        reason = "staged by d-29; wired by d-33/d-34 under durable-store"
    )]
    config: SyncHttpConfig,
    #[expect(
        dead_code,
        reason = "staged by d-29; wired by d-33/d-34 under durable-store"
    )]
    bounds: ValidatedTransportBounds,
}
pub(crate) fn exporter_for(
    config: SyncHttpConfig,
    bounds: ValidatedTransportBounds,
) -> Arc<dyn SubmissionExporter> {
    Arc::new(SyncHttpSubmissionExporter { config, bounds })
}
impl SubmissionExporter for SyncHttpSubmissionExporter {
    fn export(
        &self,
        _signal: Signal,
        _envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        Err(SubmissionExportFailure::Terminal(ExportError::Transport {
            context: Box::new(ErrorContext::new(
                crate::error_codes::SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED,
                "submission export is not wired yet",
                Remediation::recoverable(
                    "retry after D34 export integration",
                    ["no submission was exported"],
                ),
            )),
        }))
    }
}
