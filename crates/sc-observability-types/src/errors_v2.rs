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
                    #[error(transparent)]
                    $variant {
                        /// Diagnostic, remediation, source and construction backtrace.
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

impl FlushError {
    /// Returns the typed export cause retained by a drain failure, when present.
    #[must_use]
    pub fn export_cause(&self) -> Option<&ExportError> {
        std::error::Error::source(self.context())
            .and_then(|source| source.downcast_ref::<ExportError>())
    }
}

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

/// Canonical export failures with preserved diagnostic context.
#[non_exhaustive]
#[derive(Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportError {
    /// Transport failure; preserves its underlying registered diagnostic code.
    #[error(transparent)]
    Transport {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Blocking backend in async context failure; see the canonical cause mapping.
    #[error(transparent)]
    BlockingBackendInAsyncContext {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Async lifecycle required failure; see the canonical cause mapping.
    #[error(transparent)]
    AsyncLifecycleRequired {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Runtime terminated failure; see the canonical cause mapping.
    #[error(transparent)]
    RuntimeTerminated {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Lifecycle timeout failure; see the canonical cause mapping.
    #[error(transparent)]
    LifecycleTimeout {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Queue full failure; see the canonical cause mapping.
    #[error(transparent)]
    QueueFull {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Worker terminated failure; see the canonical cause mapping.
    #[error(transparent)]
    WorkerTerminated {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Shutdown cancelled retry failure; see the canonical cause mapping.
    #[error(transparent)]
    ShutdownCancelledRetry {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Retry deadline exhausted failure; see the canonical cause mapping.
    #[error(transparent)]
    RetryDeadlineExhausted {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Non-retryable HTTP status failure; see the canonical cause mapping.
    #[error(transparent)]
    NonRetryableHttpStatus {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Retry attempts exhausted failure; see the canonical cause mapping.
    #[error(transparent)]
    RetryAttemptsExhausted {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
    /// Terminal export failure; see the canonical cause mapping.
    #[error(transparent)]
    TerminalExportFailure {
        #[doc = "Diagnostic, remediation, source and construction backtrace."]
        context: Box<ErrorContext>,
    },
}

impl ExportError {
    /// Returns the original error context without reconstruction.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::Transport { context }
            | Self::BlockingBackendInAsyncContext { context }
            | Self::AsyncLifecycleRequired { context }
            | Self::RuntimeTerminated { context }
            | Self::LifecycleTimeout { context }
            | Self::QueueFull { context }
            | Self::WorkerTerminated { context }
            | Self::ShutdownCancelledRetry { context }
            | Self::RetryDeadlineExhausted { context }
            | Self::NonRetryableHttpStatus { context }
            | Self::RetryAttemptsExhausted { context }
            | Self::TerminalExportFailure { context } => context,
        }
    }
    /// Returns the preserved diagnostic.
    #[must_use]
    pub fn diagnostic(&self) -> &Diagnostic {
        self.context().diagnostic()
    }
    /// Returns the stable machine-readable code for this export failure.
    #[must_use]
    pub fn code(&self) -> crate::ErrorCode {
        match self {
            Self::Transport { context } => context.diagnostic().code.clone(),
            Self::BlockingBackendInAsyncContext { .. } => {
                crate::error_codes::otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT
            }
            Self::AsyncLifecycleRequired { .. } => {
                crate::error_codes::otlp::OTLP_ASYNC_LIFECYCLE_REQUIRED
            }
            Self::RuntimeTerminated { .. } => crate::error_codes::otlp::OTLP_RUNTIME_TERMINATED,
            Self::LifecycleTimeout { .. } => crate::error_codes::otlp::OTLP_LIFECYCLE_TIMEOUT,
            Self::QueueFull { .. } => crate::error_codes::otlp::OTLP_QUEUE_FULL,
            Self::WorkerTerminated { .. } => crate::error_codes::otlp::OTLP_WORKER_TERMINATED,
            Self::ShutdownCancelledRetry { .. } => {
                crate::error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY
            }
            Self::RetryDeadlineExhausted { .. } => {
                crate::error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED
            }
            Self::NonRetryableHttpStatus { .. } => {
                crate::error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL
            }
            Self::RetryAttemptsExhausted { .. } => {
                crate::error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED
            }
            Self::TerminalExportFailure { .. } => crate::error_codes::otlp::OTLP_EXPORT_TERMINAL,
        }
    }
    /// Takes the original boxed context, preserving source identity and backtrace.
    #[must_use]
    pub fn into_context(self) -> Box<ErrorContext> {
        match self {
            Self::Transport { context }
            | Self::BlockingBackendInAsyncContext { context }
            | Self::AsyncLifecycleRequired { context }
            | Self::RuntimeTerminated { context }
            | Self::LifecycleTimeout { context }
            | Self::QueueFull { context }
            | Self::WorkerTerminated { context }
            | Self::ShutdownCancelledRetry { context }
            | Self::RetryDeadlineExhausted { context }
            | Self::NonRetryableHttpStatus { context }
            | Self::RetryAttemptsExhausted { context }
            | Self::TerminalExportFailure { context } => context,
        }
    }
}
impl sealed::Sealed for ExportError {}
impl DiagnosticInfo for ExportError {
    fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic()
    }
}

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
    #[error("{context}")]
    Shutdown {
        /// Diagnostic, remediation, source and construction backtrace.
        #[source]
        context: Box<ErrorContext>,
    },
    /// Preserves the export variant, its context and typed source chain.
    #[error("{0}")]
    ExportFailure(#[from] ExportError),
}
impl TelemetryError {
    /// Returns the original error context without reconstruction.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::Shutdown { context } => context,
            Self::ExportFailure(error) => error.context(),
        }
    }

    /// Returns the preserved diagnostic.
    #[must_use]
    pub fn diagnostic(&self) -> &Diagnostic {
        self.context().diagnostic()
    }

    /// Takes the original boxed context, preserving source identity and backtrace.
    #[must_use]
    pub fn into_context(self) -> Box<ErrorContext> {
        match self {
            Self::Shutdown { context } => context,
            Self::ExportFailure(error) => error.into_context(),
        }
    }

    /// Returns the stable code without discarding the export cause.
    #[must_use]
    pub fn code(&self) -> crate::ErrorCode {
        match self {
            Self::Shutdown { .. } => crate::error_codes::otlp::OTLP_TELEMETRY_SHUTDOWN,
            Self::ExportFailure(error) => error.code(),
        }
    }
}

impl sealed::Sealed for TelemetryError {}

impl DiagnosticInfo for TelemetryError {
    fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic()
    }
}
