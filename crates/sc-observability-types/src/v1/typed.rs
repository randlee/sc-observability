//! Additive typed failures and neutral extension-trait adapters.
//!
//! This module is intentionally separate from the crate-root compatibility
//! surface. Existing wrappers and extension traits remain unchanged while new
//! callers can opt into stored, family-specific failure classification.
//!
//! Typed failures intentionally do not implement `Clone` or Serde:
//!
//! ```compile_fail
//! use sc_observability_types::{ErrorCode, ErrorContext, Remediation};
//! use sc_observability_types::typed::IdentityFailure;
//!
//! let failure = IdentityFailure::from_context(Box::new(ErrorContext::new(
//!     ErrorCode::new_static("CUSTOM_FAILURE"),
//!     "failure",
//!     Remediation::not_recoverable("test"),
//! )));
//! let _clone = failure.clone();
//! ```

//!
//! ```compile_fail
//! use sc_observability_types::{ErrorCode, ErrorContext, Remediation};
//! use sc_observability_types::typed::IdentityFailure;
//!
//! let failure = IdentityFailure::from_context(Box::new(ErrorContext::new(
//!     ErrorCode::new_static("CUSTOM_FAILURE"),
//!     "failure",
//!     Remediation::not_recoverable("test"),
//! )));
//! let _encoded = serde_json::to_vec(&failure);
//! ```

use std::fmt;

use serde_json::Value;
use thiserror::Error;

#[allow(deprecated)]
use crate::errors::{
    EventError as LegacyEventError, IdentityError as LegacyIdentityError,
    InitError as LegacyInitError, LogSinkError as LegacyLogSinkError,
};
use crate::errors_v2::{
    FlushError as CanonicalFlushError, InitError as CanonicalInitError,
    ShutdownError as CanonicalShutdownError,
};
use crate::{
    Diagnostic, DiagnosticInfo, ErrorCode, ErrorContext, LogEvent, Observable, Observation,
    ProcessIdentity, Remediation, error_codes, sealed,
};

#[deprecated(note = "removed; see docs/migration/phase-f.md")]
/// A diagnostic error whose family-specific kind is available without parsing
/// its diagnostic at every call site.
pub trait ClassifiedError: DiagnosticInfo {
    /// The closed-over classification family for this error value.
    type Kind: Copy + Eq;

    /// Returns the stored or compatibility-derived failure kind.
    fn kind(&self) -> Self::Kind;

    /// Returns the original structured diagnostic context.
    fn context(&self) -> &ErrorContext;
}

macro_rules! impl_failure_builders {
    ($failure:ident) => {
        #[allow(deprecated)]
        impl $failure {
            /// Adds a human-readable cause to this failure.
            #[must_use]
            pub fn cause(mut self, cause: impl Into<String>) -> Self {
                self.context.set_cause(cause);
                self
            }

            /// Adds a documentation reference to this failure.
            #[must_use]
            pub fn docs(mut self, docs: impl Into<String>) -> Self {
                self.context.set_docs(docs);
                self
            }

            /// Adds one structured diagnostic detail to this failure.
            #[must_use]
            pub fn detail(mut self, key: impl Into<String>, value: Value) -> Self {
                self.context.set_detail(key, value);
                self
            }

            /// Attaches the original source error to this failure.
            #[must_use]
            pub fn source(
                mut self,
                source: Box<dyn std::error::Error + Send + Sync + 'static>,
            ) -> Self {
                self.context.set_source(source);
                self
            }
        }
    };
}

macro_rules! failure_error_code {
    ($canonical:literal) => {
        ErrorCode::new_static($canonical)
    };
    ($canonical:literal => $code:expr) => {
        $code
    };
}

#[allow(deprecated)]
macro_rules! define_failure {
    (
        $(#[$meta:meta])*
        $legacy:ident => $failure:ident, $kind:ident {
            $(
                $constructor:ident => $variant:ident => [$canonical:literal $(=> $code:expr)? $(, $alias:literal)*]
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[allow(deprecated)]
        #[non_exhaustive]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $kind {
            $(
                #[doc = concat!("The `", stringify!($variant), "` classification.")]
                $variant,
            )+
            /// A custom, missing, or family-mismatched diagnostic code.
            Unclassified,
        }

        $(#[$meta])*
        #[allow(deprecated)]
        #[derive(Debug, PartialEq)]
        pub struct $failure {
            kind: $kind,
            context: Box<ErrorContext>,
        }

        #[allow(deprecated)]
        impl fmt::Display for $failure {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.context, formatter)
            }
        }

        #[allow(deprecated)]
        impl std::error::Error for $failure {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&*self.context)
            }
        }

        #[allow(deprecated)]
        impl $failure {
            fn classify_context(context: &ErrorContext) -> $kind {
                match context.diagnostic().code.as_str() {
                    $(
                        $canonical $(| $alias)* => $kind::$variant,
                    )+
                    _ => $kind::Unclassified,
                }
            }

            $(
                #[doc = concat!("Creates a typed failure classified as `", stringify!($variant), "`.")]
                #[must_use]
                pub fn $constructor(
                    message: impl Into<String>,
                    remediation: Remediation,
                ) -> Self {
                    Self {
                        kind: $kind::$variant,
                        context: Box::new(ErrorContext::new(
                            failure_error_code!($canonical $(=> $code)?),
                            message,
                            remediation,
                        )),
                    }
                }
            )+

            /// Classifies an existing context without changing or copying it.
            #[must_use]
            pub fn from_context(context: Box<ErrorContext>) -> Self {
                let kind = Self::classify_context(&context);
                Self { kind, context }
            }

            /// Consumes this failure and returns its original context box.
            #[must_use]
            pub fn into_context(self) -> Box<ErrorContext> {
                self.context
            }
        }

        #[allow(deprecated)]
        impl sealed::Sealed for $failure {}

        #[allow(deprecated)]
        impl DiagnosticInfo for $failure {
            fn diagnostic(&self) -> &Diagnostic {
                self.context.diagnostic()
            }
        }

        #[allow(deprecated)]
        impl ClassifiedError for $failure {
            type Kind = $kind;

            fn kind(&self) -> Self::Kind {
                self.kind
            }

            fn context(&self) -> &ErrorContext {
                &self.context
            }
        }

        impl_failure_builders!($failure);
    };
}

macro_rules! impl_legacy_classification {
    ($legacy:ident, $failure:ident, $kind:ident) => {
        #[allow(deprecated)]
        impl ClassifiedError for $legacy {
            type Kind = $kind;

            fn kind(&self) -> Self::Kind {
                <$failure>::classify_context(&self.0)
            }

            fn context(&self) -> &ErrorContext {
                &self.0
            }
        }
    };
}

define_failure! {
    #[deprecated(note = "use sc_observability_types::v2::IdentityError")]
    /// Typed process identity resolution failure.
    LegacyIdentityError => IdentityFailure, IdentityFailureKind {
        resolution_failed => ResolutionFailed => [
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"
                => error_codes::IDENTITY_RESOLUTION_FAILED
        ]
    }
}

define_failure! {
    #[deprecated(note = "use sc_observability_types::v2::InitError")]
    /// Typed initialization failure spanning the neutral/runtime boundaries.
    LegacyInitError => InitFailure, InitFailureKind {
        logger_initialization => LoggerInitialization => ["SC_OBSERVABILITY_LOGGER_INIT_FAILED"],
        observation_initialization => ObservationInitialization => ["SC_OBSERVE_INIT_FAILED"],
        invalid_telemetry_config => InvalidTelemetryConfig => ["OTLP_CONFIG_INVALID", "SC_OBSERVABILITY_OTLP_INVALID_CONFIG"],
        invalid_protocol => InvalidProtocol => ["OTLP_UNSUPPORTED_PROTOCOL", "SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL"],
        exporter_initialization => ExporterInitialization => ["OTLP_TRANSPORT_CONSTRUCTION_FAILED", "SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED"],
        identity_resolution => IdentityResolution => ["SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"]
    }
}

define_failure! {
    #[deprecated(note = "use sc_observability_types::v2::EventError")]
    /// Typed event validation or lifecycle failure.
    LegacyEventError => EventFailure, EventFailureKind {
        invalid_event => InvalidEvent => ["SC_OBSERVABILITY_LOGGER_INVALID_EVENT"],
        closed => Closed => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN"],
        queue_full => QueueFull => ["SC_OBSERVABILITY_LOGGER_QUEUE_FULL"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        shutdown_timed_out => ShutdownTimedOut => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"],
        span_assembly => SpanAssembly => ["OTLP_SPAN_ASSEMBLY_FAILED", "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"]
    }
}

define_failure! {
    #[deprecated(note = "use sc_observability_types::v2::FlushError")]
    /// Typed explicit flush failure.
    LegacyFlushError => FlushFailure, FlushFailureKind {
        logger_flush => LoggerFlush => ["SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        observation_flush => ObservationFlush => ["SC_OBSERVE_FLUSH_FAILED"],
        telemetry_flush => TelemetryFlush => ["OTLP_FLUSH_FAILED", "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"],
        closed => Closed => ["OTLP_TELEMETRY_SHUTDOWN", "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"]
    }
}

define_failure! {
    #[deprecated(note = "use sc_observability_types::v2::ShutdownError")]
    /// Typed graceful-shutdown failure.
    LegacyShutdownError => ShutdownFailure, ShutdownFailureKind {
        telemetry_flush => TelemetryFlush => ["OTLP_FLUSH_FAILED", "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"],
        incomplete_spans => IncompleteSpans => ["OTLP_INCOMPLETE_SPAN_DROPPED", "SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        timed_out => TimedOut => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"]
    }
}

define_failure! {
    #[deprecated(note = "removed; see docs/migration/phase-f.md")]
    /// Typed log, span, or metric projection failure.
    LegacyProjectionError => ProjectionFailure, ProjectionFailureKind {
        telemetry_closed => TelemetryClosed => ["OTLP_TELEMETRY_SHUTDOWN", "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"],
        telemetry_export => TelemetryExport => ["OTLP_EXPORT_TERMINAL", "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"],
        span_assembly => SpanAssembly => ["OTLP_SPAN_ASSEMBLY_FAILED", "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"],
        routing => Routing => ["SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"]
    }
}

define_failure! {
    #[deprecated(note = "removed; see docs/migration/phase-f.md")]
    /// Typed observation subscriber failure.
    LegacySubscriberError => SubscriberFailure, SubscriberFailureKind {
        routing => Routing => ["SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"]
    }
}

define_failure! {
    #[deprecated(note = "removed; see docs/migration/phase-f.md")]
    /// Typed logging sink failure.
    LegacyLogSinkError => LogSinkFailure, LogSinkFailureKind {
        write => Write => ["SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED"],
        maintenance => Maintenance => ["SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED"],
        fault_injected => FaultInjected => ["SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED"]
    }
}

define_failure! {
    #[deprecated(note = "removed; see docs/migration/phase-f.md")]
    /// Typed telemetry exporter failure.
    LegacyExportError => ExportFailure, ExportFailureKind {
        export => Export => ["OTLP_EXPORT_TERMINAL", "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"]
    }
}

/// Typed blocking logger-admission failure.
///
/// This value intentionally has no Serde representation. Its discriminant is
/// the public classification, while each context keeps its native source.
#[non_exhaustive]
#[deprecated(note = "use sc_observability_types::v2::EventError")]
#[allow(deprecated)]
#[derive(Debug, PartialEq, Error)]
pub enum LogFailure {
    /// Event validation failed before admission.
    #[error(transparent)]
    InvalidEvent(EventFailure),
    /// The writer can no longer accept work reliably.
    #[error("{0}")]
    WriterDegraded(#[source] Box<ErrorContext>),
    /// Writer shutdown exceeded its configured timeout.
    #[error("{0}")]
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}

/// Typed non-blocking logger-admission failure.
///
/// This value intentionally has no Serde representation. Its discriminant is
/// the public classification, while each context keeps its native source.
#[non_exhaustive]
#[deprecated(note = "use sc_observability_types::v2::EventError")]
#[allow(deprecated)]
#[derive(Debug, PartialEq, Error)]
pub enum TryLogFailure {
    /// Event validation failed before admission.
    #[error(transparent)]
    InvalidEvent(EventFailure),
    /// The bounded writer queue was full.
    #[error("{0}")]
    QueueFull(#[source] Box<ErrorContext>),
    /// The writer can no longer accept work reliably.
    #[error("{0}")]
    WriterDegraded(#[source] Box<ErrorContext>),
    /// Writer shutdown exceeded its configured timeout.
    #[error("{0}")]
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}

#[allow(deprecated)]
impl From<CanonicalInitError> for InitFailure {
    fn from(value: CanonicalInitError) -> Self {
        Self::from_context(value.into_context())
    }
}

#[allow(deprecated)]
impl From<CanonicalFlushError> for FlushFailure {
    fn from(value: CanonicalFlushError) -> Self {
        Self::from_context(value.into_context())
    }
}

#[allow(deprecated)]
impl From<CanonicalShutdownError> for ShutdownFailure {
    fn from(value: CanonicalShutdownError) -> Self {
        Self::from_context(value.into_context())
    }
}

macro_rules! impl_retained_legacy_conversion {
    ($legacy:ty, $failure:ty) => {
        #[allow(deprecated)]
        impl From<$legacy> for $failure {
            fn from(value: $legacy) -> Self {
                Self::from_context(value.0)
            }
        }

        #[allow(deprecated)]
        impl From<$failure> for $legacy {
            fn from(value: $failure) -> Self {
                Self(value.context)
            }
        }
    };
}

impl_retained_legacy_conversion!(LegacyIdentityError, IdentityFailure);
impl_retained_legacy_conversion!(LegacyInitError, InitFailure);
impl_retained_legacy_conversion!(LegacyEventError, EventFailure);
impl_retained_legacy_conversion!(LegacyLogSinkError, LogSinkFailure);

impl_legacy_classification!(LegacyIdentityError, IdentityFailure, IdentityFailureKind);
impl_legacy_classification!(LegacyInitError, InitFailure, InitFailureKind);
impl_legacy_classification!(LegacyEventError, EventFailure, EventFailureKind);
impl_legacy_classification!(LegacyLogSinkError, LogSinkFailure, LogSinkFailureKind);

/// Typed process identity resolver contract.
#[deprecated(note = "removed; see docs/migration/phase-f.md")]
#[allow(deprecated)]
pub trait TypedProcessIdentityResolver: Send + Sync {
    /// Resolves process identity with a typed failure on error.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityFailure`] when identity resolution cannot complete.
    fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure>;
}

/// Typed observation subscriber contract.
#[deprecated(note = "removed; see docs/migration/phase-f.md")]
#[allow(deprecated)]
pub trait TypedObservationSubscriber<T: Observable>: Send + Sync {
    /// Consumes one observation with a typed subscriber failure on error.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriberFailure`] when the subscriber rejects the observation.
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberFailure>;
}

/// Typed log projector contract.
#[deprecated(note = "removed; see docs/migration/phase-f.md")]
#[allow(deprecated)]
pub trait TypedLogProjector<T: Observable>: Send + Sync {
    /// Projects an observation into log events.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionFailure`] when log projection cannot complete.
    fn project_logs(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure>;
}
