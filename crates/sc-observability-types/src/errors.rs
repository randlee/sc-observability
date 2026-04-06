use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};

/// Error returned when process identity resolution fails.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct IdentityError(#[source] pub Box<ErrorContext>);

impl sealed::Sealed for IdentityError {}

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

        impl sealed::Sealed for $name {}

        impl DiagnosticInfo for $name {
            fn diagnostic(&self) -> &Diagnostic {
                self.0.diagnostic()
            }
        }
    };
}

error_wrapper!(
    /// Initialization error returned by public construction entry points.
    InitError
);
error_wrapper!(
    /// Event validation or lifecycle error returned during emit paths.
    EventError
);
error_wrapper!(
    /// Flush error returned by explicit flush operations.
    FlushError
);
error_wrapper!(
    /// Shutdown error returned when graceful shutdown fails.
    ShutdownError
);
error_wrapper!(
    /// Projection error returned by log/span/metric projectors.
    ProjectionError
);
error_wrapper!(
    /// Subscriber error returned by observation subscribers.
    SubscriberError
);
error_wrapper!(
    /// Logging sink error returned by concrete sink implementations.
    LogSinkError
);
error_wrapper!(
    /// Export error returned by concrete telemetry exporters.
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
