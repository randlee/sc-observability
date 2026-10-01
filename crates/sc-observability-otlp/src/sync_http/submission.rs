//! Durable submission exporter backed by the synchronous OTLP/HTTP worker.
mod logs;
mod metrics;
mod profiles;
mod resource;
mod traces;
mod values;
use super::implementation::OtlpHttpExporter;
pub(crate) use super::implementation::SyncHttpConfig;
use crate::config::ValidatedTransportBounds;
use crate::constants::PROFILES_EXPORT_PATH;
use crate::contracts::profiles::ProfileExporter;
use crate::contracts::submission::{SubmissionExportFailure, SubmissionExporter};
use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
use std::sync::{Arc, Mutex};

pub(crate) struct SyncHttpSubmissionExporter {
    config: SyncHttpConfig,
    bounds: ValidatedTransportBounds,
    exporter: Mutex<Option<Arc<OtlpHttpExporter>>>,
}
pub(crate) fn exporter_for(
    config: SyncHttpConfig,
    bounds: ValidatedTransportBounds,
) -> Arc<dyn SubmissionExporter> {
    Arc::new(SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter: Mutex::new(None),
    })
}

impl SyncHttpSubmissionExporter {
    fn exporter(&self) -> Result<Arc<OtlpHttpExporter>, ExportError> {
        let mut exporter = self
            .exporter
            .lock()
            .expect("submission exporter initialization lock");
        if exporter.is_none() {
            *exporter = Some(Arc::new(OtlpHttpExporter::from_prepared(
                self.config.clone(),
                &self.bounds,
            )?));
        }
        Ok(Arc::clone(
            exporter.as_ref().expect("initialized submission exporter"),
        ))
    }
}

impl SubmissionExporter for SyncHttpSubmissionExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        let (endpoint_signal, payload) = match signal {
            Signal::Logs => ("logs", logs::request(envelopes)),
            Signal::Traces => ("traces", traces::request(envelopes)),
            Signal::Metrics => ("metrics", metrics::request(envelopes)),
            Signal::Profiles => return self.export_profiles(envelopes).map_err(classify),
            _ => {
                return Err(SubmissionExportFailure::Terminal(ExportError::Transport {
                    context: Box::new(ErrorContext::new(
                        crate::error_codes::SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED,
                        "the requested submission signal encoder is not wired yet",
                        Remediation::recoverable(
                            "retry after D34 signal encoder integration",
                            ["no submission was exported"],
                        ),
                    )),
                }));
            }
        };
        self.exporter()
            .and_then(|exporter| exporter.submit_json_blocking(endpoint_signal, &payload))
            .map_err(classify)
    }
}

impl ProfileExporter<SubmissionEnvelope> for SyncHttpSubmissionExporter {
    fn export_profiles(&self, batch: &[SubmissionEnvelope]) -> Result<(), ExportError> {
        self.exporter()?
            .submit_json_path_blocking(PROFILES_EXPORT_PATH, &profiles::request(batch))
    }
}

fn classify(error: ExportError) -> SubmissionExportFailure {
    match error {
        error @ (ExportError::NonRetryableHttpStatus { .. }
        | ExportError::TerminalExportFailure { .. }) => SubmissionExportFailure::Terminal(error),
        error => SubmissionExportFailure::Retryable(error),
    }
}
