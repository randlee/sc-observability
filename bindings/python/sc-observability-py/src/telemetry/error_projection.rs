//! Private Python kind/variant projection; the CLI can mirror this table.
//! Non-exhaustive future variants remain `unknown`, with debug detail in the wire cause.
use sc_observability_types::otlp::submission::{
    AdmissionError, DeliveryError, SubmissionError, TelemetryConfigError,
};
pub(super) fn submission_variant(error: &SubmissionError) -> &'static str {
    match error {
        SubmissionError::InvalidJson { .. } => "invalid_json",
        SubmissionError::UnsupportedVersion { .. } => "unsupported_version",
        SubmissionError::EmptySubmission { .. } => "empty_submission",
        SubmissionError::Validation { .. } => "validation",
        SubmissionError::ValueOutOfRange { .. } => "value_out_of_range",
        SubmissionError::CorrelationConflict { .. } => "correlation_conflict",
        SubmissionError::TimingConflict { .. } => "timing_conflict",
        SubmissionError::DictionaryReference { .. } => "dictionary_reference",
        _ => "unknown",
    }
}
pub(super) fn submission_path(error: &SubmissionError) -> Option<&str> {
    match error {
        SubmissionError::Validation { path, .. }
        | SubmissionError::ValueOutOfRange { path, .. }
        | SubmissionError::DictionaryReference { path, .. } => Some(path),
        _ => None,
    }
}
pub(super) fn admission_variant(error: &AdmissionError) -> &'static str {
    match error {
        AdmissionError::StoreUnavailable { .. } => "store_unavailable",
        AdmissionError::DiskBoundExceeded { .. } => "disk_bound_exceeded",
        AdmissionError::Persistence { .. } => "persistence",
        AdmissionError::SchemaTooNew { .. } => "schema_too_new",
        AdmissionError::Closed { .. } => "closed",
        _ => "unknown",
    }
}
pub(super) fn delivery_variant(error: &DeliveryError) -> &'static str {
    match error {
        DeliveryError::DeadlineExceeded { .. } => "deadline_exceeded",
        DeliveryError::TerminalFailure { .. } => "terminal_failure",
        _ => "unknown",
    }
}
pub(super) fn delivery_report(
    error: &DeliveryError,
) -> Option<&sc_observability_types::otlp::submission::FlushReport> {
    match error {
        DeliveryError::DeadlineExceeded { report, .. }
        | DeliveryError::TerminalFailure { report, .. } => Some(report),
        _ => None,
    }
}
pub(super) fn config_variant(error: &TelemetryConfigError) -> &'static str {
    match error {
        TelemetryConfigError::ConfigFile { .. } => "config_file",
        TelemetryConfigError::MissingField { .. } => "missing_field",
        TelemetryConfigError::InvalidField { .. } => "invalid_field",
        TelemetryConfigError::UnsupportedCombination { .. } => "unsupported_combination",
        _ => "unknown",
    }
}
