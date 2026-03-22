use sc_observability_types::{MetricRecord, TraceRecord};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OtlpExportError {
    #[error("serialization failed: {0}")]
    Serialization(String),
}

pub fn export_trace_records_best_effort(
    records: &[TraceRecord],
) -> Result<String, OtlpExportError> {
    serde_json::to_string(records).map_err(|err| OtlpExportError::Serialization(err.to_string()))
}

pub fn export_metric_records_best_effort(
    records: &[MetricRecord],
) -> Result<String, OtlpExportError> {
    serde_json::to_string(records).map_err(|err| OtlpExportError::Serialization(err.to_string()))
}
