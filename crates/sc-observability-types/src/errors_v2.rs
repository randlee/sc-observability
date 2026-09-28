//! Canonical 2.0 errors staged without changing the retained 1.x surface.
use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};
use serde::{Deserialize, Serialize};

/// Wire failure category and its native-owned, operation-specific context.
///
/// This deliberately records only stable contract terms.  Boundary crates use
/// it to select their legacy or canonical wire representation without
/// re-deciding a native error's meaning or inventing a field name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClassification {
    /// Invalid caller-controlled input identified by its real input field.
    Validation { field: &'static str },
    /// A bounded queue cannot accept more work.
    QueueFull,
    /// The requested lifecycle operation is no longer available.
    Closed,
    /// A dependency or runtime is unavailable.
    Unavailable,
    /// An I/O or transport operation failed.
    Io,
    /// An operation exceeded its deadline.
    Timeout { operation: &'static str },
    /// An operation was cancelled during controlled shutdown.
    Cancelled { operation: &'static str },
    /// A local invariant or unexpected implementation failure occurred.
    Internal,
}

impl FailureClassification {
    /// Creates a validation classification for the exact native input field.
    #[must_use]
    pub const fn validation(field: &'static str) -> Self {
        Self::Validation { field }
    }

    /// Creates a timeout classification for the exact native operation.
    #[must_use]
    pub const fn timeout(operation: &'static str) -> Self {
        Self::Timeout { operation }
    }
}

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

impl IdentityError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::validation("process")
    }
}

context_error!(
    InitError,
    Configuration => crate::error_codes::DIAGNOSTIC_INVALID,
    Runtime => crate::error_codes::DIAGNOSTIC_INVALID
);

impl InitError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Configuration { .. } => FailureClassification::validation("configuration"),
            Self::Runtime { .. } => FailureClassification::Unavailable,
        }
    }
}

context_error!(
    EventError,
    Validation => crate::error_codes::DIAGNOSTIC_INVALID,
    Routing => crate::error_codes::DIAGNOSTIC_INVALID
);

impl EventError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Validation { .. } => FailureClassification::validation("event"),
            Self::Routing { .. } => FailureClassification::Unavailable,
        }
    }
}

context_error!(FlushError, Drain => crate::error_codes::DIAGNOSTIC_INVALID);

impl FlushError {
    /// Returns the typed export cause retained by a drain failure, when present.
    #[must_use]
    pub fn export_cause(&self) -> Option<&ExportError> {
        std::error::Error::source(self.context())
            .and_then(|source| source.downcast_ref::<ExportError>())
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub fn failure_classification(&self) -> FailureClassification {
        self.export_cause().map_or(
            FailureClassification::Io,
            ExportError::failure_classification,
        )
    }
}

context_error!(
    ShutdownError,
    Timeout => crate::error_codes::DIAGNOSTIC_INVALID,
    Drain => crate::error_codes::DIAGNOSTIC_INVALID
);

impl ShutdownError {
    /// Returns the typed export cause retained by a drain failure, when present.
    #[must_use]
    pub fn export_cause(&self) -> Option<&ExportError> {
        std::error::Error::source(self.context())
            .and_then(|source| source.downcast_ref::<ExportError>())
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Timeout { .. } => FailureClassification::timeout("shutdown"),
            Self::Drain { .. } => self.export_cause().map_or(
                FailureClassification::Io,
                ExportError::failure_classification,
            ),
        }
    }
}

context_error!(ProjectionError, Projection => crate::error_codes::DIAGNOSTIC_INVALID);

impl ProjectionError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::validation("projection")
    }
}

context_error!(SubscriberError, Subscriber => crate::error_codes::DIAGNOSTIC_INVALID);

impl SubscriberError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::Unavailable
    }
}

context_error!(
    LogSinkError,
    Write => crate::error_codes::DIAGNOSTIC_INVALID,
    Flush => crate::error_codes::DIAGNOSTIC_INVALID
);

impl LogSinkError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::Io
    }
}

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
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Transport { .. }
            | Self::NonRetryableHttpStatus { .. }
            | Self::RetryAttemptsExhausted { .. }
            | Self::TerminalExportFailure { .. } => FailureClassification::Io,
            Self::BlockingBackendInAsyncContext { .. } => {
                FailureClassification::validation("runtime")
            }
            Self::AsyncLifecycleRequired { .. } => FailureClassification::validation("lifecycle"),
            Self::RuntimeTerminated { .. } | Self::WorkerTerminated { .. } => {
                FailureClassification::Unavailable
            }
            Self::LifecycleTimeout { .. } => FailureClassification::timeout("lifecycle"),
            Self::QueueFull { .. } => FailureClassification::QueueFull,
            Self::ShutdownCancelledRetry { .. } => FailureClassification::Cancelled {
                operation: "shutdown",
            },
            Self::RetryDeadlineExhausted { .. } => FailureClassification::timeout("retry"),
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

impl ConfigFailure {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        let field = match self {
            Self::ZeroDuration { .. } | Self::DurationOverflow { .. } => "duration",
            Self::InvalidBoundOrdering { .. } => "bounds",
            Self::InvalidJitterPercent { .. } => "jitter_percent",
            Self::InvalidQueueCapacity { .. } => "queue_capacity",
            Self::InvalidQueueByteCapacity { .. } => "queue_byte_capacity",
            Self::ConfigFieldNotApplicable { .. } => "config",
            Self::InsecureTransportRejected { .. } | Self::InvalidEndpoint { .. } => "endpoint",
            Self::InvalidHeader { .. } => "headers",
            Self::TransportConstructionFailed { .. } => "transport",
            Self::UnsupportedBackend { .. } => "backend",
            Self::UnsupportedProtocol { .. } => "protocol",
            Self::TokioRuntimeRequired { .. } => "runtime",
        };
        FailureClassification::validation(field)
    }
}

context_error!(
    MetricModelError,
    InvalidHistogram => crate::error_codes::SC_METRIC_INVALID_HISTOGRAM,
    InvalidTemporality => crate::error_codes::SC_METRIC_INVALID_TEMPORALITY,
    InvalidInterval => crate::error_codes::SC_METRIC_INVALID_INTERVAL
);

impl MetricModelError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        let field = match self {
            Self::InvalidHistogram { .. } => "histogram",
            Self::InvalidTemporality { .. } => "temporality",
            Self::InvalidInterval { .. } => "interval",
        };
        FailureClassification::validation(field)
    }
}

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
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Shutdown { .. } => FailureClassification::Closed,
            Self::ExportFailure(error) => error.failure_classification(),
        }
    }
}

impl sealed::Sealed for TelemetryError {}

impl DiagnosticInfo for TelemetryError {
    fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic()
    }
}
