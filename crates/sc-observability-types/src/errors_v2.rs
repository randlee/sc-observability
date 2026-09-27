//! Canonical 2.0 errors staged without changing the retained 1.x surface.
use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};
use serde::{Deserialize, Serialize};

// Every case owns the original context, including its typed source and backtrace.
macro_rules! context_error {
    ($name:ident, $($variant:ident => $code:expr),+ $(,)?) => {
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
            /// Returns the stable machine-readable code fixed for this variant.
            #[must_use]
            pub fn code(&self) -> crate::ErrorCode {
                match self { $(Self::$variant { .. } => $code,)+ }
            }
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

context_error!(IdentityError, Process => crate::error_codes::IDENTITY_RESOLUTION_FAILED);

context_error!(
    InitError,
    Configuration => crate::error_codes::DIAGNOSTIC_INVALID,
    Runtime => crate::error_codes::DIAGNOSTIC_INVALID
);

context_error!(
    EventError,
    Validation => crate::error_codes::DIAGNOSTIC_INVALID,
    Routing => crate::error_codes::DIAGNOSTIC_INVALID
);

context_error!(FlushError, Drain => crate::error_codes::DIAGNOSTIC_INVALID);

context_error!(
    ShutdownError,
    Timeout => crate::error_codes::DIAGNOSTIC_INVALID,
    Drain => crate::error_codes::DIAGNOSTIC_INVALID
);

context_error!(ProjectionError, Projection => crate::error_codes::DIAGNOSTIC_INVALID);

context_error!(SubscriberError, Subscriber => crate::error_codes::DIAGNOSTIC_INVALID);

context_error!(
    LogSinkError,
    Write => crate::error_codes::DIAGNOSTIC_INVALID,
    Flush => crate::error_codes::DIAGNOSTIC_INVALID
);

context_error!(
    ExportError,
    Transport => crate::error_codes::DIAGNOSTIC_INVALID,
    BlockingBackendInAsyncContext => crate::error_codes::otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT,
    AsyncLifecycleRequired => crate::error_codes::otlp::OTLP_ASYNC_LIFECYCLE_REQUIRED,
    RuntimeTerminated => crate::error_codes::otlp::OTLP_RUNTIME_TERMINATED,
    LifecycleTimeout => crate::error_codes::otlp::OTLP_LIFECYCLE_TIMEOUT,
    QueueFull => crate::error_codes::otlp::OTLP_QUEUE_FULL,
    WorkerTerminated => crate::error_codes::otlp::OTLP_WORKER_TERMINATED,
    ShutdownCancelledRetry => crate::error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
    RetryDeadlineExhausted => crate::error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED,
    NonRetryableHttpStatus => crate::error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL,
    RetryAttemptsExhausted => crate::error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
    TerminalExportFailure => crate::error_codes::otlp::OTLP_EXPORT_TERMINAL
);

context_error!(
    ConfigFailure,
    ZeroDuration => crate::error_codes::otlp::OTLP_CONFIG_ZERO_DURATION,
    DurationOverflow => crate::error_codes::otlp::OTLP_CONFIG_DURATION_OVERFLOW,
    InvalidBoundOrdering => crate::error_codes::otlp::OTLP_CONFIG_BOUND_ORDER,
    InvalidJitterPercent => crate::error_codes::otlp::OTLP_CONFIG_JITTER_PERCENT,
    InvalidQueueCapacity => crate::error_codes::otlp::OTLP_CONFIG_QUEUE_CAPACITY,
    InvalidQueueByteCapacity => crate::error_codes::otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
    ConfigFieldNotApplicable => crate::error_codes::otlp::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
    InsecureTransportRejected => crate::error_codes::otlp::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
    InvalidEndpoint => crate::error_codes::otlp::OTLP_CONFIG_INVALID_ENDPOINT,
    InvalidHeader => crate::error_codes::otlp::OTLP_CONFIG_INVALID_HEADER,
    TransportConstructionFailed => crate::error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
    UnsupportedBackend => crate::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND,
    UnsupportedProtocol => crate::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL,
    TokioRuntimeRequired => crate::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED
);

context_error!(
    MetricModelError,
    InvalidHistogram => crate::error_codes::SC_METRIC_INVALID_HISTOGRAM,
    InvalidTemporality => crate::error_codes::SC_METRIC_INVALID_TEMPORALITY,
    InvalidInterval => crate::error_codes::SC_METRIC_INVALID_INTERVAL
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
            Self::ExportFailure(error) => error.code(),
        }
    }
}
