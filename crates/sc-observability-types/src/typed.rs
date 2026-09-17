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

use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use crate::{
    Diagnostic, DiagnosticInfo, ErrorCode, ErrorContext, EventError, ExportError, FlushError,
    IdentityError, InitError, LogEvent, LogProjector, LogSinkError, MetricProjector, MetricRecord,
    Observable, Observation, ObservationSubscriber, ProcessIdentity, ProcessIdentityResolver,
    ProjectionError, Remediation, ShutdownError, SpanProjector, SpanSignal, SubscriberError,
    sealed,
};

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

macro_rules! define_failure {
    (
        $(#[$meta:meta])*
        $legacy:ident => $failure:ident, $kind:ident {
            $(
                $constructor:ident => $variant:ident => [$canonical:literal $(, $alias:literal)*]
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
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
        #[derive(Debug, PartialEq, Error)]
        #[error("{context}")]
        pub struct $failure {
            kind: $kind,
            #[source]
            context: Box<ErrorContext>,
        }

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
                            ErrorCode::new_static($canonical),
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

        impl sealed::Sealed for $failure {}

        impl DiagnosticInfo for $failure {
            fn diagnostic(&self) -> &Diagnostic {
                self.context.diagnostic()
            }
        }

        impl ClassifiedError for $failure {
            type Kind = $kind;

            fn kind(&self) -> Self::Kind {
                self.kind
            }

            fn context(&self) -> &ErrorContext {
                &self.context
            }
        }

        impl From<$legacy> for $failure {
            fn from(value: $legacy) -> Self {
                Self::from_context(value.0)
            }
        }

        impl From<$failure> for $legacy {
            fn from(value: $failure) -> Self {
                Self(value.context)
            }
        }

        impl_failure_builders!($failure);
    };
}

macro_rules! impl_legacy_classification {
    ($legacy:ident, $failure:ident, $kind:ident) => {
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
    /// Typed process identity resolution failure.
    IdentityError => IdentityFailure, IdentityFailureKind {
        resolution_failed => ResolutionFailed => ["SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"]
    }
}

define_failure! {
    /// Typed initialization failure spanning the neutral/runtime boundaries.
    InitError => InitFailure, InitFailureKind {
        logger_initialization => LoggerInitialization => ["SC_OBSERVABILITY_LOGGER_INIT_FAILED"],
        observation_initialization => ObservationInitialization => ["SC_OBSERVE_INIT_FAILED"],
        invalid_telemetry_config => InvalidTelemetryConfig => ["SC_OBSERVABILITY_OTLP_INVALID_CONFIG"],
        invalid_protocol => InvalidProtocol => ["SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL"],
        exporter_initialization => ExporterInitialization => ["SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED"],
        identity_resolution => IdentityResolution => ["SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"]
    }
}

define_failure! {
    /// Typed event validation or lifecycle failure.
    EventError => EventFailure, EventFailureKind {
        invalid_event => InvalidEvent => ["SC_OBSERVABILITY_LOGGER_INVALID_EVENT"],
        closed => Closed => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN"],
        queue_full => QueueFull => ["SC_OBSERVABILITY_LOGGER_QUEUE_FULL"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        shutdown_timed_out => ShutdownTimedOut => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"],
        span_assembly => SpanAssembly => ["SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"]
    }
}

define_failure! {
    /// Typed explicit flush failure.
    FlushError => FlushFailure, FlushFailureKind {
        logger_flush => LoggerFlush => ["SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        observation_flush => ObservationFlush => ["SC_OBSERVE_FLUSH_FAILED"],
        telemetry_flush => TelemetryFlush => ["SC_OBSERVABILITY_OTLP_FLUSH_FAILED"],
        closed => Closed => ["SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"]
    }
}

define_failure! {
    /// Typed graceful-shutdown failure.
    ShutdownError => ShutdownFailure, ShutdownFailureKind {
        telemetry_flush => TelemetryFlush => ["SC_OBSERVABILITY_OTLP_FLUSH_FAILED"],
        incomplete_spans => IncompleteSpans => ["SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED"],
        writer_degraded => WriterDegraded => ["SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"],
        timed_out => TimedOut => ["SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"]
    }
}

define_failure! {
    /// Typed log, span, or metric projection failure.
    ProjectionError => ProjectionFailure, ProjectionFailureKind {
        telemetry_closed => TelemetryClosed => ["SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"],
        telemetry_export => TelemetryExport => ["SC_OBSERVABILITY_OTLP_EXPORT_FAILED"],
        span_assembly => SpanAssembly => ["SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"],
        routing => Routing => ["SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"]
    }
}

define_failure! {
    /// Typed observation subscriber failure.
    SubscriberError => SubscriberFailure, SubscriberFailureKind {
        routing => Routing => ["SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"]
    }
}

define_failure! {
    /// Typed logging sink failure.
    LogSinkError => LogSinkFailure, LogSinkFailureKind {
        write => Write => ["SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED"],
        maintenance => Maintenance => ["SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED"],
        fault_injected => FaultInjected => ["SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED"]
    }
}

define_failure! {
    /// Typed telemetry exporter failure.
    ExportError => ExportFailure, ExportFailureKind {
        export => Export => ["SC_OBSERVABILITY_OTLP_EXPORT_FAILED"]
    }
}

/// Typed blocking logger-admission failure.
///
/// This value intentionally has no Serde representation. Its discriminant is
/// the public classification, while each context keeps its native source.
#[non_exhaustive]
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

impl_legacy_classification!(IdentityError, IdentityFailure, IdentityFailureKind);
impl_legacy_classification!(InitError, InitFailure, InitFailureKind);
impl_legacy_classification!(EventError, EventFailure, EventFailureKind);
impl_legacy_classification!(FlushError, FlushFailure, FlushFailureKind);
impl_legacy_classification!(ShutdownError, ShutdownFailure, ShutdownFailureKind);
impl_legacy_classification!(ProjectionError, ProjectionFailure, ProjectionFailureKind);
impl_legacy_classification!(SubscriberError, SubscriberFailure, SubscriberFailureKind);
impl_legacy_classification!(LogSinkError, LogSinkFailure, LogSinkFailureKind);
impl_legacy_classification!(ExportError, ExportFailure, ExportFailureKind);

/// Typed process identity resolver contract.
pub trait TypedProcessIdentityResolver: Send + Sync {
    /// Resolves process identity with a typed failure on error.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityFailure`] when identity resolution cannot complete.
    fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure>;
}

/// Typed observation subscriber contract.
pub trait TypedObservationSubscriber<T: Observable>: Send + Sync {
    /// Consumes one observation with a typed subscriber failure on error.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriberFailure`] when the subscriber rejects the observation.
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberFailure>;
}

/// Typed log projector contract.
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

/// Typed span projector contract.
pub trait TypedSpanProjector<T: Observable>: Send + Sync {
    /// Projects an observation into span signals.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionFailure`] when span projection cannot complete.
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionFailure>;
}

/// Typed metric projector contract.
pub trait TypedMetricProjector<T: Observable>: Send + Sync {
    /// Projects an observation into metric records.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionFailure`] when metric projection cannot complete.
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionFailure>;
}

struct LegacyIdentityAdapter {
    inner: Arc<dyn TypedProcessIdentityResolver>,
}

impl ProcessIdentityResolver for LegacyIdentityAdapter {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
        self.inner.resolve().map_err(Into::into)
    }
}

struct TypedIdentityAdapter {
    inner: Arc<dyn ProcessIdentityResolver>,
}

impl TypedProcessIdentityResolver for TypedIdentityAdapter {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure> {
        self.inner.resolve().map_err(Into::into)
    }
}

struct LegacySubscriberAdapter<T: Observable> {
    inner: Arc<dyn TypedObservationSubscriber<T>>,
}

impl<T: Observable> ObservationSubscriber<T> for LegacySubscriberAdapter<T> {
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberError> {
        self.inner.observe(observation).map_err(Into::into)
    }
}

struct TypedSubscriberAdapter<T: Observable> {
    inner: Arc<dyn ObservationSubscriber<T>>,
}

impl<T: Observable> TypedObservationSubscriber<T> for TypedSubscriberAdapter<T> {
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberFailure> {
        self.inner.observe(observation).map_err(Into::into)
    }
}

struct LegacyLogProjectorAdapter<T: Observable> {
    inner: Arc<dyn TypedLogProjector<T>>,
}

impl<T: Observable> LogProjector<T> for LegacyLogProjectorAdapter<T> {
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
        self.inner.project_logs(observation).map_err(Into::into)
    }
}

struct TypedLogProjectorAdapter<T: Observable> {
    inner: Arc<dyn LogProjector<T>>,
}

impl<T: Observable> TypedLogProjector<T> for TypedLogProjectorAdapter<T> {
    fn project_logs(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        self.inner.project_logs(observation).map_err(Into::into)
    }
}

struct LegacySpanProjectorAdapter<T: Observable> {
    inner: Arc<dyn TypedSpanProjector<T>>,
}

impl<T: Observable> SpanProjector<T> for LegacySpanProjectorAdapter<T> {
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        self.inner.project_spans(observation).map_err(Into::into)
    }
}

struct TypedSpanProjectorAdapter<T: Observable> {
    inner: Arc<dyn SpanProjector<T>>,
}

impl<T: Observable> TypedSpanProjector<T> for TypedSpanProjectorAdapter<T> {
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionFailure> {
        self.inner.project_spans(observation).map_err(Into::into)
    }
}

struct LegacyMetricProjectorAdapter<T: Observable> {
    inner: Arc<dyn TypedMetricProjector<T>>,
}

impl<T: Observable> MetricProjector<T> for LegacyMetricProjectorAdapter<T> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        self.inner.project_metrics(observation).map_err(Into::into)
    }
}

struct TypedMetricProjectorAdapter<T: Observable> {
    inner: Arc<dyn MetricProjector<T>>,
}

impl<T: Observable> TypedMetricProjector<T> for TypedMetricProjectorAdapter<T> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
        self.inner.project_metrics(observation).map_err(Into::into)
    }
}

/// Adapts a typed identity resolver to the existing resolver trait.
#[must_use]
pub fn legacy_identity(
    value: Arc<dyn TypedProcessIdentityResolver>,
) -> Arc<dyn ProcessIdentityResolver> {
    Arc::new(LegacyIdentityAdapter { inner: value })
}

/// Adapts an existing identity resolver to the typed resolver trait.
#[must_use]
pub fn typed_identity(
    value: Arc<dyn ProcessIdentityResolver>,
) -> Arc<dyn TypedProcessIdentityResolver> {
    Arc::new(TypedIdentityAdapter { inner: value })
}

/// Adapts a typed subscriber to the existing subscriber trait.
#[must_use]
pub fn legacy_subscriber<T: Observable>(
    value: Arc<dyn TypedObservationSubscriber<T>>,
) -> Arc<dyn ObservationSubscriber<T>> {
    Arc::new(LegacySubscriberAdapter { inner: value })
}

/// Adapts an existing subscriber to the typed subscriber trait.
#[must_use]
pub fn typed_subscriber<T: Observable>(
    value: Arc<dyn ObservationSubscriber<T>>,
) -> Arc<dyn TypedObservationSubscriber<T>> {
    Arc::new(TypedSubscriberAdapter { inner: value })
}

/// Adapts a typed log projector to the existing projector trait.
#[must_use]
pub fn legacy_log_projector<T: Observable>(
    value: Arc<dyn TypedLogProjector<T>>,
) -> Arc<dyn LogProjector<T>> {
    Arc::new(LegacyLogProjectorAdapter { inner: value })
}

/// Adapts an existing log projector to the typed projector trait.
#[must_use]
pub fn typed_log_projector<T: Observable>(
    value: Arc<dyn LogProjector<T>>,
) -> Arc<dyn TypedLogProjector<T>> {
    Arc::new(TypedLogProjectorAdapter { inner: value })
}

/// Adapts a typed span projector to the existing projector trait.
#[must_use]
pub fn legacy_span_projector<T: Observable>(
    value: Arc<dyn TypedSpanProjector<T>>,
) -> Arc<dyn SpanProjector<T>> {
    Arc::new(LegacySpanProjectorAdapter { inner: value })
}

/// Adapts an existing span projector to the typed projector trait.
#[must_use]
pub fn typed_span_projector<T: Observable>(
    value: Arc<dyn SpanProjector<T>>,
) -> Arc<dyn TypedSpanProjector<T>> {
    Arc::new(TypedSpanProjectorAdapter { inner: value })
}

/// Adapts a typed metric projector to the existing projector trait.
#[must_use]
pub fn legacy_metric_projector<T: Observable>(
    value: Arc<dyn TypedMetricProjector<T>>,
) -> Arc<dyn MetricProjector<T>> {
    Arc::new(LegacyMetricProjectorAdapter { inner: value })
}

/// Adapts an existing metric projector to the typed projector trait.
#[must_use]
pub fn typed_metric_projector<T: Observable>(
    value: Arc<dyn MetricProjector<T>>,
) -> Arc<dyn TypedMetricProjector<T>> {
    Arc::new(TypedMetricProjectorAdapter { inner: value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn remediation() -> Remediation {
        Remediation::not_recoverable("test remediation")
    }

    fn context(code: &'static str) -> Box<ErrorContext> {
        Box::new(ErrorContext::new(
            ErrorCode::new_static(code),
            "failure",
            remediation(),
        ))
    }

    fn context_with_source(code: &'static str) -> Box<ErrorContext> {
        Box::new(
            ErrorContext::new(ErrorCode::new_static(code), "failure", remediation())
                .source(Box::new(std::io::Error::other("source"))),
        )
    }

    fn assert_context_fidelity(
        context: &ErrorContext,
        context_pointer: usize,
        backtrace_pointer: usize,
        timestamp: crate::Timestamp,
        expected_display: &str,
    ) {
        assert_eq!(std::ptr::from_ref(context) as usize, context_pointer);
        assert_eq!(
            std::ptr::from_ref(context.backtrace()) as usize,
            backtrace_pointer
        );
        assert_eq!(context.diagnostic().timestamp, timestamp);
        assert_eq!(context.to_string(), expected_display);
        let source = std::error::Error::source(context).expect("source error");
        assert_eq!(source.to_string(), "source");
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "table-driven fixture enumerates every contract constructor"
    )]
    fn named_constructors_store_their_declared_kind_and_code() {
        macro_rules! assert_constructor {
            ($constructor:path, $kind:path, $code:literal) => {
                let failure = $constructor("x", remediation());
                assert_eq!(failure.kind(), $kind);
                assert_eq!(failure.diagnostic().code.as_str(), $code);
            };
        }

        assert_constructor!(
            IdentityFailure::resolution_failed,
            IdentityFailureKind::ResolutionFailed,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"
        );
        assert_constructor!(
            InitFailure::logger_initialization,
            InitFailureKind::LoggerInitialization,
            "SC_OBSERVABILITY_LOGGER_INIT_FAILED"
        );
        assert_constructor!(
            InitFailure::observation_initialization,
            InitFailureKind::ObservationInitialization,
            "SC_OBSERVE_INIT_FAILED"
        );
        assert_constructor!(
            InitFailure::invalid_telemetry_config,
            InitFailureKind::InvalidTelemetryConfig,
            "SC_OBSERVABILITY_OTLP_INVALID_CONFIG"
        );
        assert_constructor!(
            InitFailure::invalid_protocol,
            InitFailureKind::InvalidProtocol,
            "SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL"
        );
        assert_constructor!(
            InitFailure::exporter_initialization,
            InitFailureKind::ExporterInitialization,
            "SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED"
        );
        assert_constructor!(
            InitFailure::identity_resolution,
            InitFailureKind::IdentityResolution,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"
        );
        assert_constructor!(
            EventFailure::invalid_event,
            EventFailureKind::InvalidEvent,
            "SC_OBSERVABILITY_LOGGER_INVALID_EVENT"
        );
        assert_constructor!(
            EventFailure::closed,
            EventFailureKind::Closed,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN"
        );
        assert_constructor!(
            EventFailure::queue_full,
            EventFailureKind::QueueFull,
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL"
        );
        assert_constructor!(
            EventFailure::writer_degraded,
            EventFailureKind::WriterDegraded,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"
        );
        assert_constructor!(
            EventFailure::shutdown_timed_out,
            EventFailureKind::ShutdownTimedOut,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"
        );
        assert_constructor!(
            EventFailure::span_assembly,
            EventFailureKind::SpanAssembly,
            "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"
        );
        assert_constructor!(
            FlushFailure::logger_flush,
            FlushFailureKind::LoggerFlush,
            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"
        );
        assert_constructor!(
            FlushFailure::writer_degraded,
            FlushFailureKind::WriterDegraded,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"
        );
        assert_constructor!(
            FlushFailure::observation_flush,
            FlushFailureKind::ObservationFlush,
            "SC_OBSERVE_FLUSH_FAILED"
        );
        assert_constructor!(
            FlushFailure::telemetry_flush,
            FlushFailureKind::TelemetryFlush,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"
        );
        assert_constructor!(
            FlushFailure::closed,
            FlushFailureKind::Closed,
            "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"
        );
        assert_constructor!(
            ShutdownFailure::telemetry_flush,
            ShutdownFailureKind::TelemetryFlush,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"
        );
        assert_constructor!(
            ShutdownFailure::incomplete_spans,
            ShutdownFailureKind::IncompleteSpans,
            "SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED"
        );
        assert_constructor!(
            ShutdownFailure::writer_degraded,
            ShutdownFailureKind::WriterDegraded,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED"
        );
        assert_constructor!(
            ShutdownFailure::timed_out,
            ShutdownFailureKind::TimedOut,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT"
        );
        assert_constructor!(
            ProjectionFailure::telemetry_closed,
            ProjectionFailureKind::TelemetryClosed,
            "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN"
        );
        assert_constructor!(
            ProjectionFailure::telemetry_export,
            ProjectionFailureKind::TelemetryExport,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
        assert_constructor!(
            ProjectionFailure::span_assembly,
            ProjectionFailureKind::SpanAssembly,
            "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED"
        );
        assert_constructor!(
            ProjectionFailure::routing,
            ProjectionFailureKind::Routing,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
        assert_constructor!(
            SubscriberFailure::routing,
            SubscriberFailureKind::Routing,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
        assert_constructor!(
            LogSinkFailure::write,
            LogSinkFailureKind::Write,
            "SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED"
        );
        assert_constructor!(
            LogSinkFailure::maintenance,
            LogSinkFailureKind::Maintenance,
            "SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED"
        );
        assert_constructor!(
            LogSinkFailure::fault_injected,
            LogSinkFailureKind::FaultInjected,
            "SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED"
        );
        assert_constructor!(
            ExportFailure::export,
            ExportFailureKind::Export,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "table-driven fixture enumerates every contract code mapping"
    )]
    fn every_contract_mapping_classifies_in_its_own_family() {
        macro_rules! assert_context_kind {
            ($failure:ty, $code:literal, $kind:expr) => {
                assert_eq!(<$failure>::from_context(context($code)).kind(), $kind);
            };
        }

        assert_context_kind!(
            IdentityFailure,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED",
            IdentityFailureKind::ResolutionFailed
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVABILITY_LOGGER_INIT_FAILED",
            InitFailureKind::LoggerInitialization
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVE_INIT_FAILED",
            InitFailureKind::ObservationInitialization
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVABILITY_OTLP_INVALID_CONFIG",
            InitFailureKind::InvalidTelemetryConfig
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL",
            InitFailureKind::InvalidProtocol
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED",
            InitFailureKind::ExporterInitialization
        );
        assert_context_kind!(
            InitFailure,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED",
            InitFailureKind::IdentityResolution
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_LOGGER_INVALID_EVENT",
            EventFailureKind::InvalidEvent
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN",
            EventFailureKind::Closed
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL",
            EventFailureKind::QueueFull
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED",
            EventFailureKind::WriterDegraded
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT",
            EventFailureKind::ShutdownTimedOut
        );
        assert_context_kind!(
            EventFailure,
            "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED",
            EventFailureKind::SpanAssembly
        );
        assert_context_kind!(
            FlushFailure,
            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED",
            FlushFailureKind::LoggerFlush
        );
        assert_context_kind!(
            FlushFailure,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED",
            FlushFailureKind::WriterDegraded
        );
        assert_context_kind!(
            FlushFailure,
            "SC_OBSERVE_FLUSH_FAILED",
            FlushFailureKind::ObservationFlush
        );
        assert_context_kind!(
            FlushFailure,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED",
            FlushFailureKind::TelemetryFlush
        );
        assert_context_kind!(
            FlushFailure,
            "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN",
            FlushFailureKind::Closed
        );
        assert_context_kind!(
            ShutdownFailure,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED",
            ShutdownFailureKind::TelemetryFlush
        );
        assert_context_kind!(
            ShutdownFailure,
            "SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED",
            ShutdownFailureKind::IncompleteSpans
        );
        assert_context_kind!(
            ShutdownFailure,
            "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED",
            ShutdownFailureKind::WriterDegraded
        );
        assert_context_kind!(
            ShutdownFailure,
            "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT",
            ShutdownFailureKind::TimedOut
        );
        assert_context_kind!(
            ProjectionFailure,
            "SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN",
            ProjectionFailureKind::TelemetryClosed
        );
        assert_context_kind!(
            ProjectionFailure,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED",
            ProjectionFailureKind::TelemetryExport
        );
        assert_context_kind!(
            ProjectionFailure,
            "SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED",
            ProjectionFailureKind::SpanAssembly
        );
        assert_context_kind!(
            ProjectionFailure,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE",
            ProjectionFailureKind::Routing
        );
        assert_context_kind!(
            SubscriberFailure,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE",
            SubscriberFailureKind::Routing
        );
        assert_context_kind!(
            LogSinkFailure,
            "SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED",
            LogSinkFailureKind::Write
        );
        assert_context_kind!(
            LogSinkFailure,
            "SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED",
            LogSinkFailureKind::Maintenance
        );
        assert_context_kind!(
            LogSinkFailure,
            "SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED",
            LogSinkFailureKind::FaultInjected
        );
        assert_context_kind!(
            ExportFailure,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED",
            ExportFailureKind::Export
        );
    }

    #[test]
    fn every_family_preserves_identity_through_all_builders() {
        macro_rules! assert_builders {
            ($constructor:path, $kind:path, $code:literal) => {{
                let failure = $constructor("failure", remediation());
                let context_pointer = std::ptr::from_ref(failure.context()) as usize;
                let backtrace_pointer = std::ptr::from_ref(failure.context().backtrace()) as usize;
                let timestamp = failure.diagnostic().timestamp;
                let failure = failure
                    .cause("cause")
                    .docs("https://example.invalid/docs")
                    .detail("attempt", json!(2))
                    .source(Box::new(std::io::Error::other("source")));

                assert_eq!(failure.kind(), $kind);
                assert_eq!(failure.diagnostic().code.as_str(), $code);
                assert_eq!(failure.diagnostic().cause.as_deref(), Some("cause"));
                assert_eq!(
                    failure.diagnostic().docs.as_deref(),
                    Some("https://example.invalid/docs")
                );
                assert_eq!(failure.diagnostic().details["attempt"], json!(2));
                assert_context_fidelity(
                    failure.context(),
                    context_pointer,
                    backtrace_pointer,
                    timestamp,
                    "failure: cause; caused by: source",
                );
            }};
        }

        assert_builders!(
            IdentityFailure::resolution_failed,
            IdentityFailureKind::ResolutionFailed,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"
        );
        assert_builders!(
            InitFailure::logger_initialization,
            InitFailureKind::LoggerInitialization,
            "SC_OBSERVABILITY_LOGGER_INIT_FAILED"
        );
        assert_builders!(
            EventFailure::invalid_event,
            EventFailureKind::InvalidEvent,
            "SC_OBSERVABILITY_LOGGER_INVALID_EVENT"
        );
        assert_builders!(
            FlushFailure::logger_flush,
            FlushFailureKind::LoggerFlush,
            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"
        );
        assert_builders!(
            ShutdownFailure::telemetry_flush,
            ShutdownFailureKind::TelemetryFlush,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"
        );
        assert_builders!(
            ProjectionFailure::telemetry_export,
            ProjectionFailureKind::TelemetryExport,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
        assert_builders!(
            SubscriberFailure::routing,
            SubscriberFailureKind::Routing,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
        assert_builders!(
            LogSinkFailure::write,
            LogSinkFailureKind::Write,
            "SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED"
        );
        assert_builders!(
            ExportFailure::export,
            ExportFailureKind::Export,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
    }

    #[test]
    fn every_family_moves_legacy_context_without_reconstruction() {
        macro_rules! assert_round_trip {
            ($legacy:ident, $failure:ident, $kind:path, $code:literal) => {{
                let original = context_with_source($code);
                let context_pointer = std::ptr::from_ref(original.as_ref()) as usize;
                let backtrace_pointer = std::ptr::from_ref(original.backtrace()) as usize;
                let timestamp = original.diagnostic().timestamp;
                let legacy = $legacy(original);
                assert_eq!(ClassifiedError::kind(&legacy), $kind);
                let typed = $failure::from(legacy);
                assert_eq!(typed.kind(), $kind);
                assert_context_fidelity(
                    typed.context(),
                    context_pointer,
                    backtrace_pointer,
                    timestamp,
                    "failure; caused by: source",
                );
                let legacy = $legacy::from(typed);
                assert_context_fidelity(
                    &legacy.0,
                    context_pointer,
                    backtrace_pointer,
                    timestamp,
                    "failure; caused by: source",
                );
                let typed = $failure::from(legacy);
                assert_eq!(typed.kind(), $kind);
                assert_context_fidelity(
                    typed.context(),
                    context_pointer,
                    backtrace_pointer,
                    timestamp,
                    "failure; caused by: source",
                );
            }};
        }

        assert_round_trip!(
            IdentityError,
            IdentityFailure,
            IdentityFailureKind::ResolutionFailed,
            "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED"
        );
        assert_round_trip!(
            InitError,
            InitFailure,
            InitFailureKind::LoggerInitialization,
            "SC_OBSERVABILITY_LOGGER_INIT_FAILED"
        );
        assert_round_trip!(
            EventError,
            EventFailure,
            EventFailureKind::InvalidEvent,
            "SC_OBSERVABILITY_LOGGER_INVALID_EVENT"
        );
        assert_round_trip!(
            FlushError,
            FlushFailure,
            FlushFailureKind::LoggerFlush,
            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"
        );
        assert_round_trip!(
            ShutdownError,
            ShutdownFailure,
            ShutdownFailureKind::TelemetryFlush,
            "SC_OBSERVABILITY_OTLP_FLUSH_FAILED"
        );
        assert_round_trip!(
            ProjectionError,
            ProjectionFailure,
            ProjectionFailureKind::TelemetryExport,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
        assert_round_trip!(
            SubscriberError,
            SubscriberFailure,
            SubscriberFailureKind::Routing,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
        assert_round_trip!(
            LogSinkError,
            LogSinkFailure,
            LogSinkFailureKind::Write,
            "SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED"
        );
        assert_round_trip!(
            ExportError,
            ExportFailure,
            ExportFailureKind::Export,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
    }

    #[test]
    fn every_family_rejects_custom_and_cross_family_codes() {
        macro_rules! assert_unclassified {
            ($failure:ty, $unclassified:path, $code:literal) => {
                assert_eq!(
                    <$failure>::from_context(context($code)).kind(),
                    $unclassified
                );
            };
        }

        assert_unclassified!(
            IdentityFailure,
            IdentityFailureKind::Unclassified,
            "CUSTOM_FAILURE"
        );
        assert_unclassified!(
            InitFailure,
            InitFailureKind::Unclassified,
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL"
        );
        assert_unclassified!(
            EventFailure,
            EventFailureKind::Unclassified,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
        assert_unclassified!(
            FlushFailure,
            FlushFailureKind::Unclassified,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
        assert_unclassified!(
            ShutdownFailure,
            ShutdownFailureKind::Unclassified,
            "SC_OBSERVABILITY_LOGGER_FLUSH_FAILED"
        );
        assert_unclassified!(
            ProjectionFailure,
            ProjectionFailureKind::Unclassified,
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL"
        );
        assert_unclassified!(
            SubscriberFailure,
            SubscriberFailureKind::Unclassified,
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL"
        );
        assert_unclassified!(
            LogSinkFailure,
            LogSinkFailureKind::Unclassified,
            "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
        );
        assert_unclassified!(
            ExportFailure,
            ExportFailureKind::Unclassified,
            "SC_OBSERVE_OBSERVATION_ROUTING_FAILURE"
        );
    }

    #[test]
    fn unknown_and_cross_family_codes_are_unclassified_without_data_loss() {
        let unknown = EventFailure::from_context(context("CUSTOM_FAILURE"));
        assert_eq!(unknown.kind(), EventFailureKind::Unclassified);
        assert_eq!(unknown.diagnostic().code.as_str(), "CUSTOM_FAILURE");

        let cross_family = InitFailure::from_context(context("SC_OBSERVABILITY_LOGGER_QUEUE_FULL"));
        assert_eq!(cross_family.kind(), InitFailureKind::Unclassified);
        assert_eq!(
            cross_family.diagnostic().code.as_str(),
            "SC_OBSERVABILITY_LOGGER_QUEUE_FULL"
        );
    }

    #[test]
    fn legacy_conversion_moves_the_original_context_box() {
        let original = context("SC_OBSERVABILITY_LOGGER_QUEUE_FULL");
        let pointer = std::ptr::from_ref::<ErrorContext>(original.as_ref());
        let typed = EventFailure::from(EventError(original));
        assert_eq!(std::ptr::from_ref(typed.context()), pointer);
        let legacy = EventError::from(typed);
        assert_eq!(std::ptr::from_ref(legacy.0.as_ref()), pointer);
    }

    #[test]
    fn builders_modify_the_existing_context_and_retain_classification() {
        let failure = EventFailure::queue_full("failure", remediation())
            .cause("capacity")
            .docs("https://example.invalid/failure")
            .detail("attempt", json!(2));
        assert_eq!(failure.kind(), EventFailureKind::QueueFull);
        assert_eq!(failure.diagnostic().cause.as_deref(), Some("capacity"));
        assert_eq!(
            failure.diagnostic().docs.as_deref(),
            Some("https://example.invalid/failure")
        );
        assert_eq!(failure.diagnostic().details["attempt"], json!(2));
    }

    #[test]
    fn legacy_serialization_remains_available_and_typed_failures_are_not_serializable() {
        let legacy = EventError(context("SC_OBSERVABILITY_LOGGER_QUEUE_FULL"));
        let encoded = serde_json::to_vec(&legacy).expect("legacy wrapper serializes");
        let decoded: EventError = serde_json::from_slice(&encoded).expect("legacy wrapper decodes");
        assert_eq!(decoded, legacy);
    }

    #[derive(Debug)]
    struct TypedResolver;

    impl TypedProcessIdentityResolver for TypedResolver {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure> {
            Ok(ProcessIdentity::default())
        }
    }

    #[derive(Debug)]
    struct LegacyResolver;

    impl ProcessIdentityResolver for LegacyResolver {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            Ok(ProcessIdentity::default())
        }
    }

    struct TypedProjector;
    impl<T: Observable> TypedLogProjector<T> for TypedProjector {
        fn project_logs(&self, _: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionFailure> {
            Ok(Vec::new())
        }
    }
    impl<T: Observable> TypedSpanProjector<T> for TypedProjector {
        fn project_spans(&self, _: &Observation<T>) -> Result<Vec<SpanSignal>, ProjectionFailure> {
            Ok(Vec::new())
        }
    }
    impl<T: Observable> TypedMetricProjector<T> for TypedProjector {
        fn project_metrics(
            &self,
            _: &Observation<T>,
        ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
            Ok(Vec::new())
        }
    }

    struct LegacyProjector;
    impl<T: Observable> LogProjector<T> for LegacyProjector {
        fn project_logs(&self, _: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
            Ok(Vec::new())
        }
    }
    impl<T: Observable> SpanProjector<T> for LegacyProjector {
        fn project_spans(&self, _: &Observation<T>) -> Result<Vec<SpanSignal>, ProjectionError> {
            Ok(Vec::new())
        }
    }
    impl<T: Observable> MetricProjector<T> for LegacyProjector {
        fn project_metrics(
            &self,
            _: &Observation<T>,
        ) -> Result<Vec<MetricRecord>, ProjectionError> {
            Ok(Vec::new())
        }
    }

    struct TypedSubscriber;
    impl<T: Observable> TypedObservationSubscriber<T> for TypedSubscriber {
        fn observe(&self, _: &Observation<T>) -> Result<(), SubscriberFailure> {
            Ok(())
        }
    }
    struct LegacySubscriber;
    impl<T: Observable> ObservationSubscriber<T> for LegacySubscriber {
        fn observe(&self, _: &Observation<T>) -> Result<(), SubscriberError> {
            Ok(())
        }
    }

    #[test]
    fn explicit_adapters_preserve_success_and_are_object_safe() {
        let observation = Observation::new(
            crate::ServiceName::new("typed-test").expect("valid service"),
            "payload".to_string(),
        );

        assert!(legacy_identity(Arc::new(TypedResolver)).resolve().is_ok());
        assert!(typed_identity(Arc::new(LegacyResolver)).resolve().is_ok());
        assert!(
            legacy_subscriber::<String>(Arc::new(TypedSubscriber))
                .observe(&observation)
                .is_ok()
        );
        assert!(
            typed_subscriber::<String>(Arc::new(LegacySubscriber))
                .observe(&observation)
                .is_ok()
        );
        assert!(
            legacy_log_projector::<String>(Arc::new(TypedProjector))
                .project_logs(&observation)
                .is_ok()
        );
        assert!(
            typed_log_projector::<String>(Arc::new(LegacyProjector))
                .project_logs(&observation)
                .is_ok()
        );
        assert!(
            legacy_span_projector::<String>(Arc::new(TypedProjector))
                .project_spans(&observation)
                .is_ok()
        );
        assert!(
            typed_span_projector::<String>(Arc::new(LegacyProjector))
                .project_spans(&observation)
                .is_ok()
        );
        assert!(
            legacy_metric_projector::<String>(Arc::new(TypedProjector))
                .project_metrics(&observation)
                .is_ok()
        );
        assert!(
            typed_metric_projector::<String>(Arc::new(LegacyProjector))
                .project_metrics(&observation)
                .is_ok()
        );
    }
}
