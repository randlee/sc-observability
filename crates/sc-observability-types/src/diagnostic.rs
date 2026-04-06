use std::backtrace::Backtrace;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{ErrorCode, Timestamp, sealed};

/// Ordered recovery steps for a recoverable diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableSteps {
    steps: Vec<String>,
}

impl RecoverableSteps {
    /// Creates a recoverable step list containing exactly one first action.
    pub fn first(step: impl Into<String>) -> Self {
        Self {
            steps: vec![step.into()],
        }
    }

    /// Creates a recoverable step list from a full ordered set of actions.
    pub fn all(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            steps: steps.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the first recommended recovery step, if present.
    pub fn first_step(&self) -> Option<&str> {
        self.steps.first().map(String::as_str)
    }

    /// Returns all ordered recovery steps.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }
}

/// Required remediation metadata attached to every diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Remediation {
    /// The caller can recover by following the ordered steps.
    Recoverable {
        /// Ordered recovery steps the caller can take.
        steps: RecoverableSteps,
    },
    /// The caller cannot recover automatically and must accept the justification.
    NotRecoverable {
        /// Reason the failure cannot be recovered automatically.
        justification: String,
    },
}

impl Remediation {
    /// Builds a recoverable remediation with one required first step and any remaining ordered steps.
    pub fn recoverable(
        first: impl Into<String>,
        rest: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut steps = vec![first.into()];
        steps.extend(rest.into_iter().map(Into::into));
        Self::Recoverable {
            steps: RecoverableSteps::all(steps),
        }
    }

    /// Builds a non-recoverable remediation with the required justification.
    pub fn not_recoverable(justification: impl Into<String>) -> Self {
        Self::NotRecoverable {
            justification: justification.into(),
        }
    }
}

/// Structured diagnostic payload reusable across CLI, logging, and telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// UTC timestamp when the diagnostic was created.
    pub timestamp: Timestamp,
    /// Stable machine-readable error code.
    pub code: ErrorCode,
    /// Human-readable summary message.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional human-readable cause string.
    pub cause: Option<String>,
    /// Required remediation guidance.
    pub remediation: Remediation,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional documentation reference or URL.
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    /// Structured machine-readable details.
    pub details: Map<String, Value>,
}

/// Trait for public error surfaces that can expose an attached diagnostic.
pub trait DiagnosticInfo: sealed::Sealed {
    /// Returns the structured diagnostic attached to this error surface.
    fn diagnostic(&self) -> &Diagnostic;
}

/// Small diagnostic summary used in health and last-error reporting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    /// Optional stable error code for the last reported failure.
    pub code: Option<ErrorCode>,
    /// Human-readable summary message.
    pub message: String,
    /// UTC timestamp when the summarized diagnostic occurred.
    pub at: Timestamp,
}

impl From<&Diagnostic> for DiagnosticSummary {
    fn from(value: &Diagnostic) -> Self {
        Self {
            code: Some(value.code.clone()),
            message: value.message.clone(),
            at: value.timestamp,
        }
    }
}

/// Builder-style context wrapper used by public crate error types.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorContext {
    diagnostic: Diagnostic,
    #[serde(skip, default = "capture_backtrace")]
    backtrace: Backtrace,
    #[serde(skip)]
    source: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,
}

impl PartialEq for ErrorContext {
    fn eq(&self, other: &Self) -> bool {
        self.diagnostic == other.diagnostic
            && self.source.as_ref().map(ToString::to_string)
                == other.source.as_ref().map(ToString::to_string)
    }
}

impl ErrorContext {
    /// Creates a new error context with the required code, message, and remediation.
    pub fn new(code: ErrorCode, message: impl Into<String>, remediation: Remediation) -> Self {
        Self {
            diagnostic: Diagnostic {
                timestamp: Timestamp::now_utc(),
                code,
                message: message.into(),
                cause: None,
                remediation,
                docs: None,
                details: Map::new(),
            },
            backtrace: capture_backtrace(),
            source: None,
        }
    }

    /// Adds a human-readable cause string to the error context.
    pub fn cause(mut self, cause: impl Into<String>) -> Self {
        self.diagnostic.cause = Some(cause.into());
        self
    }

    /// Adds a documentation reference string to the error context.
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.diagnostic.docs = Some(docs.into());
        self
    }

    /// Adds one structured detail field to the error context.
    pub fn detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.diagnostic.details.insert(key.into(), value);
        self
    }

    /// Captures the real source error for display and error chaining.
    pub fn source(mut self, source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        self.source = Some(Arc::from(source));
        self
    }

    /// Returns the structured diagnostic carried by this error context.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Returns the captured construction backtrace.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic.message)?;
        if let Some(cause) = &self.diagnostic.cause {
            write!(f, ": {cause}")?;
        }
        if let Some(source) = std::error::Error::source(self) {
            write!(f, "; caused by: {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

fn capture_backtrace() -> Backtrace {
    Backtrace::capture()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::{IdentityError, error_codes};

    #[test]
    fn remediation_construction_helpers_cover_both_variants() {
        let recoverable = Remediation::recoverable("fix the input", ["retry"]);
        let not_recoverable = Remediation::not_recoverable("manual intervention required");
        let first_only = RecoverableSteps::first("first");
        let all_steps = RecoverableSteps::all(["first", "second"]);

        match recoverable {
            Remediation::Recoverable { steps } => {
                assert_eq!(steps.first_step(), Some("fix the input"));
                assert_eq!(
                    steps.steps(),
                    ["fix the input".to_string(), "retry".to_string()]
                );
            }
            Remediation::NotRecoverable { .. } => panic!("expected recoverable remediation"),
        }

        match not_recoverable {
            Remediation::NotRecoverable { justification } => {
                assert_eq!(justification, "manual intervention required");
            }
            Remediation::Recoverable { .. } => panic!("expected non-recoverable remediation"),
        }

        assert_eq!(first_only.first_step(), Some("first"));
        assert_eq!(first_only.steps(), ["first".to_string()]);
        assert_eq!(
            all_steps.steps(),
            ["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn error_context_display_includes_cause_when_present() {
        let error = ErrorContext::new(
            error_codes::DIAGNOSTIC_INVALID,
            "operation failed",
            Remediation::recoverable("fix the config", ["retry"]),
        )
        .cause("missing field");

        assert_eq!(error.to_string(), "operation failed: missing field");
    }

    #[test]
    fn error_context_display_includes_source_chain_when_present() {
        let error = ErrorContext::new(
            error_codes::DIAGNOSTIC_INVALID,
            "operation failed",
            Remediation::not_recoverable("investigate"),
        )
        .cause("missing field")
        .source(Box::new(std::io::Error::other("disk full")));

        assert_eq!(
            error.to_string(),
            "operation failed: missing field; caused by: disk full"
        );
    }

    #[test]
    fn error_context_builder_sets_docs_details_and_source() {
        let error = ErrorContext::new(
            error_codes::DIAGNOSTIC_INVALID,
            "operation failed",
            Remediation::not_recoverable("investigate manually"),
        )
        .docs("https://example.test/failure")
        .detail("attempt", json!(3))
        .source(Box::new(std::io::Error::other("disk full")));

        assert_eq!(
            error.diagnostic().docs.as_deref(),
            Some("https://example.test/failure")
        );
        assert_eq!(error.diagnostic().details.get("attempt"), Some(&json!(3)));
        assert_eq!(
            error.source.as_ref().map(ToString::to_string).as_deref(),
            Some("disk full")
        );
        assert_eq!(
            std::error::Error::source(&error)
                .map(ToString::to_string)
                .as_deref(),
            Some("disk full")
        );
        assert!(!matches!(
            error.backtrace().status(),
            std::backtrace::BacktraceStatus::Unsupported
        ));
    }

    #[test]
    fn diagnostic_round_trips_through_serde() {
        let original = Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: error_codes::DIAGNOSTIC_INVALID,
            message: "diagnostic invalid".to_string(),
            cause: Some("invalid example".to_string()),
            remediation: Remediation::recoverable(
                "fix the input",
                ["rerun the command", "review the docs"],
            ),
            docs: Some("https://example.test/docs".to_string()),
            details: Map::from_iter([("key".to_string(), json!("value"))]),
        };
        let encoded = serde_json::to_string(&original).expect("serialize diagnostic");
        let decoded: Diagnostic = serde_json::from_str(&encoded).expect("deserialize diagnostic");
        assert_eq!(decoded, original);
    }

    #[test]
    fn diagnostic_summary_captures_code_and_message() {
        let diagnostic = Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: error_codes::DIAGNOSTIC_INVALID,
            message: "diagnostic invalid".to_string(),
            cause: None,
            remediation: Remediation::recoverable("fix the input", ["retry"]),
            docs: None,
            details: Map::new(),
        };
        let summary = DiagnosticSummary::from(&diagnostic);

        assert_eq!(summary.code, Some(error_codes::DIAGNOSTIC_INVALID));
        assert_eq!(summary.message, "diagnostic invalid");
        assert_eq!(summary.at, Timestamp::UNIX_EPOCH);
    }

    #[test]
    fn identity_error_exposes_inner_diagnostic() {
        let context = ErrorContext::new(
            error_codes::IDENTITY_RESOLUTION_FAILED,
            "failed to resolve process identity",
            Remediation::not_recoverable("configure a valid identity source"),
        )
        .detail("source", json!("test"));
        let error = IdentityError(Box::new(context));

        assert_eq!(
            error.diagnostic().code,
            error_codes::IDENTITY_RESOLUTION_FAILED
        );
        assert_eq!(
            error.diagnostic().message,
            "failed to resolve process identity"
        );
        assert_eq!(
            error.diagnostic().details.get("source"),
            Some(&json!("test"))
        );
    }
}
