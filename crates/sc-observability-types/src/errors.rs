use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Diagnostic, DiagnosticInfo, ErrorContext};

/// Error returned when process identity resolution fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct IdentityError(#[source] pub Box<ErrorContext>);

impl DiagnosticInfo for IdentityError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

macro_rules! error_wrapper {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
        #[error("{0}")]
        pub struct $name(#[source] pub Box<ErrorContext>);

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum ObservationError {
    #[error("observation runtime is shut down")]
    Shutdown,
    #[error("{0}")]
    QueueFull(#[source] Box<ErrorContext>),
    #[error("{0}")]
    RoutingFailure(#[source] Box<ErrorContext>),
}

/// Telemetry emit error returned by `Telemetry` operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum TelemetryError {
    #[error("telemetry runtime is shut down")]
    Shutdown,
    #[error("{0}")]
    ExportFailure(#[source] Box<ErrorContext>),
}
