//! Durable submission exporter backed by the synchronous OTLP/HTTP worker.
mod logs;
mod metrics;
mod profiles;
mod resource;
#[cfg(test)]
mod tests;
mod traces;
mod values;
pub(crate) use super::implementation::SyncHttpConfig;
use super::implementation::{OtlpHttpExporter, SubmissionRoute};
use crate::config::ValidatedTransportBounds;
use crate::constants::MAX_OTLP_ENCODED_REQUEST_BYTES;
use crate::contracts::profiles::ProfileExporter;
use crate::contracts::submission::{SubmissionExportFailure, SubmissionExporter};
use crate::error_codes::OTLP_EXPORT_TERMINAL;
use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::submission::{Signal, SubmissionEnvelope},
    v2::ExportError,
};
use std::{fmt, sync::Arc};

#[cfg(test)]
pub(super) fn golden_fixture(name: &str, file: &str) -> String {
    let path = format!(
        "{}/../sc-observability-types/tests/fixtures/otlp_submission/golden/{name}/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).expect("canonical submission fixture reads")
}

pub(crate) struct SyncHttpSubmissionExporter {
    config: SyncHttpConfig,
    bounds: ValidatedTransportBounds,
    exporter: Arc<OtlpHttpExporter>,
}

impl fmt::Debug for SyncHttpSubmissionExporter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SyncHttpSubmissionExporter")
            .field("config", &self.config)
            .field("bounds", &self.bounds)
            .field("exporter", &"synchronous HTTP worker")
            .finish()
    }
}
pub(crate) fn exporter_for(
    config: SyncHttpConfig,
    bounds: ValidatedTransportBounds,
) -> Result<Arc<dyn SubmissionExporter>, ExportError> {
    let exporter = Arc::new(OtlpHttpExporter::from_prepared(config.clone(), &bounds)?);
    Ok(Arc::new(SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter,
    }))
}

impl SyncHttpSubmissionExporter {
    fn exporter(&self) -> Arc<OtlpHttpExporter> {
        Arc::clone(&self.exporter)
    }
}

impl SubmissionExporter for SyncHttpSubmissionExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        let (route, encode) = match signal {
            Signal::Logs => (
                SubmissionRoute::Signal(Signal::Logs),
                logs::request
                    as fn(&[SubmissionEnvelope]) -> Result<serde_json::Value, ExportError>,
            ),
            Signal::Traces => (
                SubmissionRoute::Signal(Signal::Traces),
                traces::request
                    as fn(&[SubmissionEnvelope]) -> Result<serde_json::Value, ExportError>,
            ),
            Signal::Metrics => (
                SubmissionRoute::Signal(Signal::Metrics),
                metrics::request
                    as fn(&[SubmissionEnvelope]) -> Result<serde_json::Value, ExportError>,
            ),
            Signal::Profiles => return self.export_profiles(envelopes).map_err(classify),
            _ => {
                return Err(SubmissionExportFailure::Terminal(ExportError::Transport {
                    context: Box::new(ErrorContext::new(
                        OTLP_EXPORT_TERMINAL,
                        "this sc-observability-otlp version cannot encode the requested submission signal",
                        Remediation::not_recoverable(
                            "upgrade sc-observability-otlp to a version that encodes this signal",
                        ),
                    )),
                }));
            }
        };
        self.export_encoded(route, envelopes, encode)
    }

    fn cancel(&self) {
        self.exporter.cancel_submission();
    }
}

impl SyncHttpSubmissionExporter {
    fn export_encoded(
        &self,
        route: SubmissionRoute,
        envelopes: &[SubmissionEnvelope],
        encode: fn(&[SubmissionEnvelope]) -> Result<serde_json::Value, ExportError>,
    ) -> Result<(), SubmissionExportFailure> {
        let payload = encode(envelopes).map_err(classify)?;
        self.submit_encoded(route, envelopes, &payload, encode)
            .map_err(classify)
    }

    fn submit_encoded(
        &self,
        route: SubmissionRoute,
        envelopes: &[SubmissionEnvelope],
        payload: &serde_json::Value,
        encode: fn(&[SubmissionEnvelope]) -> Result<serde_json::Value, ExportError>,
    ) -> Result<(), ExportError> {
        if payload.to_string().len() <= MAX_OTLP_ENCODED_REQUEST_BYTES || envelopes.len() <= 1 {
            return self.exporter().submit_json_blocking(route, payload);
        }

        // Request boundaries must be based on encoded bytes, not the input
        // envelope count: escaped and base64 values can expand substantially.
        for envelope in envelopes {
            self.submit_encoded(
                route,
                std::slice::from_ref(envelope),
                &encode(std::slice::from_ref(envelope))?,
                encode,
            )?;
        }
        Ok(())
    }
}

impl ProfileExporter<SubmissionEnvelope> for SyncHttpSubmissionExporter {
    fn export_profiles(&self, batch: &[SubmissionEnvelope]) -> Result<(), ExportError> {
        let exporter = self.exporter();
        for envelope in batch.iter().filter(|envelope| envelope.profiles.is_some()) {
            // Profile-table indices are local to one envelope's dictionary. Sending one
            // request per dictionary preserves every index without flattening tables.
            exporter.submit_json_blocking(
                SubmissionRoute::Profiles,
                &profiles::request(std::slice::from_ref(envelope))?,
            )?;
        }
        Ok(())
    }
}

fn classify(error: ExportError) -> SubmissionExportFailure {
    match error {
        error @ (ExportError::NonRetryableHttpStatus { .. }
        | ExportError::RetryAttemptsExhausted { .. }
        | ExportError::RetryDeadlineExhausted { .. }
        | ExportError::TerminalExportFailure { .. }
        // A producer-side deadline has an unknown worker outcome. Retrying
        // the drained batch could duplicate a request the worker completes.
        | ExportError::LifecycleTimeout { .. }) => SubmissionExportFailure::Terminal(error),
        error => SubmissionExportFailure::Retryable(error),
    }
}

#[cfg(test)]
#[test]
fn completed_retry_budgets_are_terminal_but_shutdown_is_recoverable() {
    let context = || {
        Box::new(ErrorContext::new(
            crate::error_codes::SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED,
            "retry classification fixture",
            Remediation::recoverable("retry after restart", ["fixture"]),
        ))
    };
    for error in [
        ExportError::RetryAttemptsExhausted { context: context() },
        ExportError::RetryDeadlineExhausted { context: context() },
        ExportError::LifecycleTimeout { context: context() },
    ] {
        assert!(matches!(
            classify(error),
            SubmissionExportFailure::Terminal(_)
        ));
    }
    assert!(matches!(
        classify(ExportError::ShutdownCancelledRetry { context: context() }),
        SubmissionExportFailure::Retryable(_)
    ));
}
