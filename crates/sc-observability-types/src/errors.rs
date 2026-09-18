#![allow(
    deprecated,
    reason = "this module owns the retained legacy wrapper definitions and their DiagnosticInfo implementations"
)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};

/// Error returned when process identity resolution fails.
#[deprecated(
    since = "1.4.0",
    note = "Use sc_observability_types::typed::IdentityFailure; see migrate-error-api.md."
)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct IdentityError(#[source] pub Box<ErrorContext>);

impl sealed::Sealed for IdentityError {}

#[allow(
    deprecated,
    reason = "IdentityError remains a retained compatibility wrapper"
)]
impl DiagnosticInfo for IdentityError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

macro_rules! error_wrapper {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
        #[error("{0}")]
        pub struct $name(#[source] pub Box<ErrorContext>);

        #[allow(
            deprecated,
            reason = "legacy error wrapper remains a retained compatibility boundary"
        )]
        impl sealed::Sealed for $name {}

        #[allow(
            deprecated,
            reason = "legacy error wrapper retains its DiagnosticInfo implementation"
        )]
        impl DiagnosticInfo for $name {
            fn diagnostic(&self) -> &Diagnostic {
                self.0.diagnostic()
            }
        }
    };
}

error_wrapper!(
    /// Initialization error returned by public construction entry points.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::InitFailure; see migrate-error-api.md."
    )]
    InitError
);
error_wrapper!(
    /// Event validation or lifecycle error returned during emit paths.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::EventFailure; see migrate-error-api.md."
    )]
    EventError
);
error_wrapper!(
    /// Flush error returned by explicit flush operations.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::FlushFailure; see migrate-error-api.md."
    )]
    FlushError
);
error_wrapper!(
    /// Shutdown error returned when graceful shutdown fails.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::ShutdownFailure; see migrate-error-api.md."
    )]
    ShutdownError
);
error_wrapper!(
    /// Projection error returned by log/span/metric projectors.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::ProjectionFailure; see migrate-error-api.md."
    )]
    ProjectionError
);
error_wrapper!(
    /// Subscriber error returned by observation subscribers.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::SubscriberFailure; see migrate-error-api.md."
    )]
    SubscriberError
);
error_wrapper!(
    /// Logging sink error returned by concrete sink implementations.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::LogSinkFailure; see migrate-error-api.md."
    )]
    LogSinkError
);
error_wrapper!(
    /// Export error returned by concrete telemetry exporters.
    #[deprecated(
        since = "1.4.0",
        note = "Use sc_observability_types::typed::ExportFailure; see migrate-error-api.md."
    )]
    ExportError
);

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

/// Telemetry emit error returned by `Telemetry` operations.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
pub enum TelemetryError {
    #[error("telemetry runtime is shut down")]
    /// The telemetry runtime has already been shut down.
    Shutdown,
    #[error("{0}")]
    /// Export or span-assembly work failed for the requested telemetry operation.
    ExportFailure(#[source] Box<ErrorContext>),
}

#[cfg(test)]
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
