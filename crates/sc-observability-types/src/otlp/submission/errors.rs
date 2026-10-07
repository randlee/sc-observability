//! Typed submission, configuration, admission and delivery failures.
use super::{EnvelopeVersion, ExporterBackendId, FlushReport, Representation, Signal};
use crate::{ErrorCode, ErrorContext, Remediation};
use std::path::PathBuf;

/// Builds the diagnostic for `code` with that code's specific remediation.
pub(crate) fn context(code: ErrorCode, message: impl Into<String>) -> Box<ErrorContext> {
    let remediation = remediation(&code);
    Box::new(ErrorContext::new(code, message, remediation))
}

/// Builds the diagnostic for `code`, retaining `source` as the error cause chain.
pub(crate) fn context_with_source(
    code: ErrorCode,
    message: impl Into<String>,
    source: impl std::error::Error + Send + Sync + 'static,
) -> Box<ErrorContext> {
    let remediation = remediation(&code);
    Box::new(ErrorContext::new(code, message, remediation).source(Box::new(source)))
}

/// Concrete caller guidance for each submission, admission, delivery and
/// configuration code; retryable conditions are recoverable.
fn remediation(code: &ErrorCode) -> Remediation {
    use crate::error_codes as c;
    let fixed = |justification: &str| Remediation::not_recoverable(justification);
    if *code == c::SC_OBSERVABILITY_SUBMIT_INVALID_JSON {
        fixed(
            "The document is not JSON; fix the syntax at the reported line and column and resubmit.",
        )
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION {
        fixed(
            "This library reads only the current envelope version; resubmit with that version or upgrade the reader.",
        )
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_EMPTY {
        fixed(
            "A submission must carry at least one log, span, metric or profile; add a record and resubmit.",
        )
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_VALIDATION {
        fixed("The named field violates the signal data model; correct it and resubmit.")
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE {
        fixed("OTLP stores integers as int64; send the value as a double or string, or reduce it.")
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_CORRELATION_CONFLICT {
        fixed(
            "Records sharing a correlation_id must agree on trace_id and span_id; remove or align the explicit identifiers.",
        )
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT {
        fixed(
            "Supply either end_time or duration_nanos, or make start_time plus duration_nanos equal end_time.",
        )
    } else if *code == c::SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE {
        fixed(
            "Every profiles index must address an existing dictionary entry; add the entry or correct the index.",
        )
    } else if *code == c::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE {
        Remediation::recoverable(
            "Check that the store path exists, is writable and is not locked by another process.",
            ["Retry the emit after the store is reachable."],
        )
    } else if *code == c::SC_OBSERVABILITY_ADMIT_DISK_BOUND {
        Remediation::recoverable(
            "Flush to deliver pending rows and free store capacity.",
            [
                "Retry the emit.",
                "Raise max_store_bytes or select the evict_oldest disk-bound policy if loss of old rows is acceptable.",
            ],
        )
    } else if *code == c::SC_OBSERVABILITY_ADMIT_PERSISTENCE {
        Remediation::recoverable(
            "Check free disk space and store file permissions.",
            ["Retry the emit; nothing was committed."],
        )
    } else if *code == c::SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW {
        fixed(
            "The store was written by a newer release; upgrade this client or point it at a different store path.",
        )
    } else if *code == c::SC_OBSERVABILITY_ADMIT_CLOSED {
        Remediation::recoverable(
            "Open a new client; a shut-down client admits nothing.",
            ["Emit again through the new client."],
        )
    } else if *code == c::SC_OBSERVABILITY_DELIVERY_DEADLINE {
        Remediation::recoverable(
            "Rows remain durably pending; flush again later or with a longer deadline.",
            ["Check that the collector endpoint is reachable."],
        )
    } else if *code == c::SC_OBSERVABILITY_DELIVERY_FAILED {
        fixed(
            "The collector rejected the rows terminally; inspect status for the per-signal error code and correct the payload or collector configuration.",
        )
    } else if *code == c::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE {
        fixed(
            "Fix the named configuration file so it exists, is readable and is valid YAML with known keys.",
        )
    } else if *code == c::SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING {
        fixed("Supply the named field explicitly or in the configuration file.")
    } else if *code == c::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID {
        fixed("Set the named configuration field to a value inside the documented bounds.")
    } else if *code == c::SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED {
        fixed("Select a backend that supports this signal representation, or omit the signal.")
    } else if *code == c::SC_OBSERVABILITY_TEST_DOUBLE_SCRIPTED_FAILURE {
        fixed(
            "The test script requested this failure; change the script to exercise another outcome.",
        )
    } else {
        fixed("Correct the cause named by the diagnostic code before retrying.")
    }
}

/// Typed `SubmissionError` retaining diagnostic context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SubmissionError {
    /// `SUBMIT_INVALID_JSON` failure.
    #[error("{context}")]
    InvalidJson {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_UNSUPPORTED_VERSION` failure.
    #[error("{context}")]
    UnsupportedVersion {
        /// Failure detail.
        found: EnvelopeVersion,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_EMPTY` failure.
    #[error("{context}")]
    EmptySubmission {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_VALIDATION` failure.
    #[error("{context}")]
    Validation {
        /// Failure detail.
        path: String,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_VALUE_OUT_OF_RANGE` failure.
    #[error("{context}")]
    ValueOutOfRange {
        /// Failure detail.
        path: String,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_CORRELATION_CONFLICT` failure.
    #[error("{context}")]
    CorrelationConflict {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_TIMING_CONFLICT` failure.
    #[error("{context}")]
    TimingConflict {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `SUBMIT_DICTIONARY_REFERENCE` failure.
    #[error("{context}")]
    DictionaryReference {
        /// Failure detail.
        path: String,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
}
impl SubmissionError {
    /// Returns the original error context.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::InvalidJson { context, .. }
            | Self::UnsupportedVersion { context, .. }
            | Self::EmptySubmission { context, .. }
            | Self::Validation { context, .. }
            | Self::ValueOutOfRange { context, .. }
            | Self::CorrelationConflict { context, .. }
            | Self::TimingConflict { context, .. }
            | Self::DictionaryReference { context, .. } => context,
        }
    }
    /// Returns the stable error code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        &self.context().diagnostic().code
    }
}
impl crate::sealed::Sealed for SubmissionError {}
impl crate::DiagnosticInfo for SubmissionError {
    fn diagnostic(&self) -> &crate::Diagnostic {
        self.context().diagnostic()
    }
}

/// Typed `AdmissionError` retaining diagnostic context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AdmissionError {
    /// `ADMIT_STORE_UNAVAILABLE` failure.
    #[error("{context}")]
    StoreUnavailable {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `ADMIT_DISK_BOUND` failure.
    #[error("{context}")]
    DiskBoundExceeded {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `ADMIT_PERSISTENCE` failure.
    #[error("{context}")]
    Persistence {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `ADMIT_SCHEMA_TOO_NEW` failure.
    #[error("{context}")]
    SchemaTooNew {
        /// Failure detail.
        found: u32,
        /// Failure detail.
        supported: u32,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `ADMIT_CLOSED` failure.
    #[error("{context}")]
    Closed {
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
}
impl AdmissionError {
    /// Returns the original error context.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::StoreUnavailable { context, .. }
            | Self::DiskBoundExceeded { context, .. }
            | Self::Persistence { context, .. }
            | Self::SchemaTooNew { context, .. }
            | Self::Closed { context, .. } => context,
        }
    }
    /// Returns the stable error code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        &self.context().diagnostic().code
    }
}
impl crate::sealed::Sealed for AdmissionError {}
impl crate::DiagnosticInfo for AdmissionError {
    fn diagnostic(&self) -> &crate::Diagnostic {
        self.context().diagnostic()
    }
}

/// Typed `DeliveryError` retaining diagnostic context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DeliveryError {
    /// `DELIVERY_DEADLINE` failure.
    #[error("{context}")]
    DeadlineExceeded {
        /// Failure detail.
        report: FlushReport,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `DELIVERY_FAILED` failure.
    #[error("{context}")]
    TerminalFailure {
        /// Failure detail.
        report: FlushReport,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
}
impl DeliveryError {
    /// Returns the original error context.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::DeadlineExceeded { context, .. } | Self::TerminalFailure { context, .. } => {
                context
            }
        }
    }
    /// Returns the stable error code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        &self.context().diagnostic().code
    }
}
impl crate::sealed::Sealed for DeliveryError {}
impl crate::DiagnosticInfo for DeliveryError {
    fn diagnostic(&self) -> &crate::Diagnostic {
        self.context().diagnostic()
    }
}

/// Typed `TelemetryConfigError` retaining diagnostic context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TelemetryConfigError {
    /// `TELEMETRY_CONFIG_FILE` failure.
    #[error("{context}")]
    ConfigFile {
        /// Failure detail.
        path: PathBuf,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `TELEMETRY_CONFIG_MISSING` failure.
    #[error("{context}")]
    MissingField {
        /// Failure detail.
        field: &'static str,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `TELEMETRY_CONFIG_INVALID` failure.
    #[error("{context}")]
    InvalidField {
        /// Failure detail.
        field: &'static str,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
    /// `TELEMETRY_UNSUPPORTED` failure.
    #[error("{context}")]
    UnsupportedCombination {
        /// Failure detail.
        backend: ExporterBackendId,
        /// Failure detail.
        signal: Signal,
        /// Failure detail.
        representation: Representation,
        /// Preserved diagnostic, remediation and source.
        context: Box<ErrorContext>,
    },
}
impl TelemetryConfigError {
    /// Returns the original error context.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::ConfigFile { context, .. }
            | Self::MissingField { context, .. }
            | Self::InvalidField { context, .. }
            | Self::UnsupportedCombination { context, .. } => context,
        }
    }
    /// Returns the stable error code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        &self.context().diagnostic().code
    }
}
impl crate::sealed::Sealed for TelemetryConfigError {}
impl crate::DiagnosticInfo for TelemetryConfigError {
    fn diagnostic(&self) -> &crate::Diagnostic {
        self.context().diagnostic()
    }
}

/// Shared failure boundary for all telemetry clients.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TelemetryClientError {
    /// Invalid submission.
    #[error(transparent)]
    Submission(#[from] SubmissionError),
    /// Durable admission failed.
    #[error(transparent)]
    Admission(#[from] AdmissionError),
    /// Delivery did not complete successfully.
    #[error(transparent)]
    Delivery(#[from] DeliveryError),
    /// Invalid client configuration.
    #[error(transparent)]
    Config(#[from] TelemetryConfigError),
}
impl TelemetryClientError {
    /// Returns the machine-readable cause code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        match self {
            Self::Submission(e) => e.code(),
            Self::Admission(e) => e.code(),
            Self::Delivery(e) => e.code(),
            Self::Config(e) => e.code(),
        }
    }
}
impl SubmissionError {
    pub(crate) fn validation(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            path: path.into(),
            context: context(
                crate::error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION,
                message,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::context;
    #[cfg(feature = "v1")]
    use super::remediation;
    #[cfg(feature = "v1")]
    use crate::ErrorCode;
    use crate::{Remediation, error_codes};

    #[cfg(feature = "v1")]
    #[test]
    fn every_submission_code_has_specific_remediation() {
        let fallback = remediation(&ErrorCode::new_static("UNREGISTERED"));
        let mut seen = Vec::new();
        for code in [
            error_codes::SC_OBSERVABILITY_SUBMIT_INVALID_JSON,
            error_codes::SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION,
            error_codes::SC_OBSERVABILITY_SUBMIT_EMPTY,
            error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION,
            error_codes::SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE,
            error_codes::SC_OBSERVABILITY_SUBMIT_CORRELATION_CONFLICT,
            error_codes::SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT,
            error_codes::SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE,
            error_codes::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE,
            error_codes::SC_OBSERVABILITY_ADMIT_DISK_BOUND,
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
            error_codes::SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW,
            error_codes::SC_OBSERVABILITY_ADMIT_CLOSED,
            error_codes::SC_OBSERVABILITY_DELIVERY_DEADLINE,
            error_codes::SC_OBSERVABILITY_DELIVERY_FAILED,
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING,
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            error_codes::SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED,
            error_codes::SC_OBSERVABILITY_TEST_DOUBLE_SCRIPTED_FAILURE,
        ] {
            let advice = remediation(&code);
            assert_ne!(advice, fallback, "{code} uses the generic remediation");
            assert!(
                !seen.contains(&advice),
                "{code} repeats another remediation"
            );
            seen.push(advice);
        }
        assert_eq!(seen.len(), 20, "submission codes missing from the registry");
    }

    #[test]
    fn retryable_codes_are_recoverable() {
        for code in [
            error_codes::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE,
            error_codes::SC_OBSERVABILITY_ADMIT_DISK_BOUND,
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
            error_codes::SC_OBSERVABILITY_ADMIT_CLOSED,
            error_codes::SC_OBSERVABILITY_DELIVERY_DEADLINE,
        ] {
            assert!(
                matches!(
                    context(code.clone(), "m").diagnostic().remediation,
                    Remediation::Recoverable { .. }
                ),
                "{code}"
            );
        }
    }
}
