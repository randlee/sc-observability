//! Durable client contract. Persistence and draining are implemented by D33.
#![allow(
    clippy::result_large_err,
    reason = "approved errors preserve the inline four-signal FlushReport"
)]
mod adapter;
mod config_file;
pub use config_file::load_telemetry_file;
use sc_lint_attributes::sc_lint;
use sc_observability_types::otlp::submission::{
    AdmissionError, AdmissionReceipt, FlushReport, StatusQuery, StoreStatus, SubmissionEnvelope,
    SubmissionId, TelemetryClient, TelemetryClientConfig, TelemetryClientError, error_codes,
};
use sc_observability_types::{ErrorCode, ErrorContext, Remediation};
use std::time::Duration;
/// Durable client handle; opening fails explicitly until the D33 store is implemented.
#[derive(Debug)]
pub struct DurableTelemetryClient {
    _private: (),
}
fn context(code: ErrorCode, message: &str) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        code,
        message,
        Remediation::recoverable(
            "retry after the durable store implementation is available",
            ["D33 implements persistence and draining"],
        ),
    ))
}
fn unavailable() -> TelemetryClientError {
    AdmissionError::StoreUnavailable {
        context: context(
            error_codes::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE,
            "durable store is not implemented yet",
        ),
    }
    .into()
}
impl TelemetryClient for DurableTelemetryClient {
    #[sc_lint(boundary.allow("cycle.type_method_self_loop"))]
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        if let Ok(otel) = adapter::otel_config_from(&config)
            && let Ok((worker, bounds)) =
                crate::sync_http::submission::SyncHttpConfig::from_otel(&otel)
        {
            drop(crate::sync_http::submission::exporter_for(worker, bounds));
        }
        Err(unavailable())
    }
    fn emit(
        &self,
        _envelope: SubmissionEnvelope,
    ) -> Result<AdmissionReceipt, TelemetryClientError> {
        Err(unavailable())
    }
    fn flush(&self, _deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        Err(unavailable())
    }
    fn flush_submission(
        &self,
        _id: &SubmissionId,
        _deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        Err(unavailable())
    }
    fn shutdown(&self, _deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        Err(unavailable())
    }
    fn status(&self, _query: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
        Err(unavailable())
    }
}
