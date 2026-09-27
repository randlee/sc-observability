//! Canonical 2.0 errors staged without changing the retained 1.x surface.
use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};
use serde::{Deserialize, Serialize};

// Every case owns the original context, including its typed source and backtrace.
macro_rules! context_error {
    ($name:ident, $($variant:ident),+ $(,)?) => {
        #[doc = concat!("Canonical ", stringify!($name), " with preserved diagnostic context.")]
        #[non_exhaustive]
        #[derive(Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum $name {
            $(
                #[doc = concat!(stringify!($variant), " failure; see the canonical cause mapping.")]
                #[error("{context}")]
                $variant {
                    /// Diagnostic, remediation, source and construction backtrace.
                    #[source]
                    context: Box<ErrorContext>,
                },
            )+
        }
        impl $name {
            /// Returns the original error context without reconstruction.
            #[must_use]
            pub fn context(&self) -> &ErrorContext {
                match self { $(Self::$variant { context } => context,)+ }
            }
            /// Returns the preserved diagnostic.
            #[must_use]
            pub fn diagnostic(&self) -> &Diagnostic { self.context().diagnostic() }
            /// Takes the original boxed context, preserving source identity and backtrace.
            #[must_use]
            pub fn into_context(self) -> Box<ErrorContext> {
                match self { $(Self::$variant { context } => context,)+ }
            }
        }
        impl sealed::Sealed for $name {}
        impl DiagnosticInfo for $name {
            fn diagnostic(&self) -> &Diagnostic { self.diagnostic() }
        }
    };
}

context_error!(IdentityError, Process);

context_error!(InitError, Configuration, Runtime);

context_error!(EventError, Validation, Routing);

context_error!(FlushError, Drain);

context_error!(ShutdownError, Timeout, Drain);

context_error!(ProjectionError, Projection);

context_error!(SubscriberError, Subscriber);

context_error!(LogSinkError, Write, Flush);

context_error!(
    ExportError,
    Transport,
    BlockingBackendInAsyncContext,
    AsyncLifecycleRequired,
    RuntimeTerminated,
    LifecycleTimeout,
    QueueFull,
    WorkerTerminated,
    ShutdownCancelledRetry,
    RetryDeadlineExhausted,
    NonRetryableHttpStatus,
    RetryAttemptsExhausted,
    TerminalExportFailure
);

context_error!(
    ConfigFailure,
    ZeroDuration,
    DurationOverflow,
    InvalidBoundOrdering,
    InvalidJitterPercent,
    InvalidQueueCapacity,
    InvalidQueueByteCapacity,
    ConfigFieldNotApplicable,
    InsecureTransportRejected,
    InvalidEndpoint,
    InvalidHeader,
    TransportConstructionFailed,
    UnsupportedBackend,
    UnsupportedProtocol,
    TokioRuntimeRequired
);

context_error!(
    MetricModelError,
    InvalidHistogram,
    InvalidTemporality,
    InvalidInterval
);

/// Telemetry admission guard or the precise canonical export failure.
#[non_exhaustive]
#[derive(Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum TelemetryError {
    /// Admission is closed; stable code `OTLP_TELEMETRY_SHUTDOWN`.
    #[error("telemetry runtime is shut down")]
    Shutdown,
    /// Preserves the export variant, its context and typed source chain.
    #[error("{0}")]
    ExportFailure(#[from] ExportError),
}
impl TelemetryError {
    /// Returns the stable code without discarding the export cause.
    #[must_use]
    pub fn code(&self) -> crate::ErrorCode {
        match self {
            Self::Shutdown => crate::error_codes::otlp::OTLP_TELEMETRY_SHUTDOWN,
            Self::ExportFailure(error) => error.diagnostic().code.clone(),
        }
    }
}
