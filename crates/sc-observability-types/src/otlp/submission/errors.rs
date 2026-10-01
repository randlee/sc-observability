//! Typed submission, configuration, admission and delivery failures.
use super::{EnvelopeVersion, ExporterBackendId, FlushReport, Representation, Signal};
use crate::{ErrorCode, ErrorContext, Remediation};
use std::path::PathBuf;

pub(crate) fn context(code: ErrorCode, message: impl Into<String>) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        code,
        message,
        Remediation::not_recoverable(
            "Inspect the typed failure and correct its cause before retrying.",
        ),
    ))
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
