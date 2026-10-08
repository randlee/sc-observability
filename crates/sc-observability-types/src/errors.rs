#![allow(
    deprecated,
    reason = "this module owns the retained legacy wrapper definitions and their DiagnosticInfo implementations"
)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ErrorContext;
#[cfg(feature = "v1")]
use crate::{Diagnostic, DiagnosticInfo, sealed};

/// Error returned when process identity resolution fails.
#[cfg(feature = "v1")]
#[deprecated(
    since = "1.4.0",
    note = "use sc_observability_types::v2::IdentityError"
)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct IdentityError(#[source] pub Box<ErrorContext>);

#[cfg(feature = "v1")]
impl sealed::Sealed for IdentityError {}

#[allow(
    deprecated,
    reason = "IdentityError remains a retained compatibility wrapper"
)]
#[cfg(feature = "v1")]
impl DiagnosticInfo for IdentityError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

/// Initialization error returned by public construction entry points.
#[cfg(feature = "v1")]
#[deprecated(since = "1.5.0", note = "use sc_observability_types::v2::InitError")]
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct InitError(#[source] pub Box<ErrorContext>);

#[cfg(feature = "v1")]
impl sealed::Sealed for InitError {}

#[cfg(feature = "v1")]
impl DiagnosticInfo for InitError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

/// Event validation or lifecycle error returned during emit paths.
#[cfg(feature = "v1")]
#[deprecated(since = "1.5.0", note = "use sc_observability_types::v2::EventError")]
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct EventError(#[source] pub Box<ErrorContext>);

#[cfg(feature = "v1")]
impl sealed::Sealed for EventError {}

#[cfg(feature = "v1")]
impl DiagnosticInfo for EventError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

/// Logging sink error returned by concrete sink implementations.
#[cfg(feature = "v1")]
#[deprecated(since = "1.5.0", note = "use sc_observability_types::v2::LogSinkError")]
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct LogSinkError(#[source] pub Box<ErrorContext>);

#[cfg(feature = "v1")]
impl sealed::Sealed for LogSinkError {}

#[cfg(feature = "v1")]
impl DiagnosticInfo for LogSinkError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

/// Routing/runtime error returned by `Observability::emit`.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
pub enum ObservationError {
    #[error("observation runtime is shut down")]
    /// The routing runtime has already been shut down.
    Shutdown,
    #[error("{0}")]
    /// The runtime could not accept more observations.
    QueueFull(#[source] Box<ErrorContext>),
    #[error("{0}")]
    /// No eligible subscriber or projector path handled the observation.
    RoutingFailure(#[source] Box<ErrorContext>),
}

#[cfg(all(test, feature = "v1"))]
mod tests {
    use super::*;
    use crate::{Remediation, error_codes};

    #[test]
    fn wrapper_errors_expose_source_context() {
        let wrapped = InitError(Box::new(
            ErrorContext::new(
                error_codes::DIAGNOSTIC_INVALID,
                "operation failed",
                Remediation::not_recoverable("investigate manually"),
            )
            .source(Box::new(std::io::Error::other("disk full"))),
        ));

        let source = std::error::Error::source(&wrapped).expect("context source");
        assert_eq!(source.to_string(), "operation failed; caused by: disk full");
        assert_eq!(
            source.source().map(ToString::to_string).as_deref(),
            Some("disk full")
        );
    }
}
