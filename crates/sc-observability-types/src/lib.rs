//! Shared neutral contracts for the `sc-observability` workspace.
//!
//! This crate defines the reusable value types, diagnostics, typestate span
//! contracts, health reports, and open extension traits consumed by the higher
//! layers in the workspace. It intentionally avoids owning sinks, routing
//! runtimes, exporter behavior, or application-specific payload types.

pub mod constants;
mod diagnostic;
pub mod error_codes;
mod errors;
mod errors_v2;
mod events;
mod health;
mod level;
mod observation_v2;
pub mod otlp;
mod primitives;
mod process;
mod query;
mod signals_v2;
mod span;
mod tracing;
#[cfg(feature = "v1")]
mod v1;
mod validation;

#[cfg(feature = "v1")]
#[doc(inline)]
pub use v1::typed;
#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "released v1 metric and routing paths remain feature-gated"
)]
#[doc(inline)]
pub use v1::{
    LogProjector, MetricKind, MetricProjector, MetricRecord, ObservationSubscriber,
    ProjectionRegistration, SpanProjector, SubscriberRegistration,
};

mod sealed {
    pub trait Sealed {}
}

#[doc(hidden)]
pub mod telemetry_health_provider_sealed {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Token(pub(crate) ());

    #[doc(hidden)]
    pub(crate) const TOKEN: Token = Token(());

    #[doc(hidden)]
    #[must_use]
    pub fn workspace_token() -> Token {
        TOKEN
    }

    pub trait Sealed {
        fn token(&self) -> Token;
    }
}

#[doc(inline)]
pub use constants::DEFAULT_ENV_PREFIX_SEPARATOR;
#[doc(inline)]
pub use constants::OBSERVATION_ENVELOPE_VERSION;
#[doc(inline)]
pub use constants::OBSERVATION_SCHEMA_VERSION;
#[doc(inline)]
pub use diagnostic::{
    Diagnostic, DiagnosticInfo, DiagnosticSummary, ErrorContext, RecoverableSteps, Remediation,
};
#[doc(inline)]
pub use errors::ObservationError;
#[cfg(feature = "v1")]
#[doc(inline)]
#[allow(deprecated, reason = "released v1 wrappers remain feature-gated")]
pub use errors::{EventError, IdentityError, InitError, LogSinkError, TelemetryError};
#[doc(inline)]
pub use errors_v2::{ConfigFailure, FailureClassification, MetricModelError};
#[doc(inline)]
pub use events::{LogEvent, Observable, Observation};
#[doc(inline)]
pub use health::{
    ExporterHealth, ExporterHealthState, FileCount, LoggingHealthReport, LoggingHealthState,
    MaintenanceHealthReport, MaintenanceWorkerState, ObservabilityHealthProvider,
    ObservabilityHealthReport, ObservationHealthState, QueryHealthReport, QueryHealthState,
    SinkHealth, SinkHealthState, TelemetryHealthReport, TelemetryHealthState, WriterState,
};
#[doc(inline)]
pub use level::{
    AdmissionOutcome, ChangeDiagnostic, Level, LevelChange, LevelChangeError, LevelChangeSource,
    LevelFilter, LevelState, OperationDiagnostic,
};
#[doc(inline)]
pub use primitives::{DurationMs, ErrorCode, Timestamp};
#[doc(inline)]
pub use process::{ProcessIdentity, ProcessIdentityPolicy, ProcessIdentityResolver};
#[doc(inline)]
pub use query::{LogFieldMatch, LogOrder, LogQuery, LogSnapshot, QueryError};
#[doc(inline)]
pub use span::{SpanEnded, SpanEvent, SpanRecord, SpanSignal, SpanStarted, SpanStatus};
#[doc(inline)]
pub use tracing::{SpanId, StateTransition, TraceContext, TraceId};
#[doc(inline)]
pub use validation::{
    ActionName, CorrelationId, EntityId, EnvPrefix, MetricName, MetricUnit, OutcomeLabel,
    SchemaVersion, ServiceName, SinkName, StateName, TargetCategory, ToolName,
    ValueValidationError,
};

/// Canonical error contracts and additive neutral signal models.
///
/// The package remains at the workspace version until the atomic D.21 bump.
/// ADR-017 authorizes integration's canonical error migration. Neutral signal
/// models remain under `v2`: the existing root `MetricRecord`, `TraceContext`,
/// and `SpanRecord` APIs and serialization remain unchanged under ADR-012.
/// The version bump does not authorize replacing those root signal types.
pub mod v2 {
    #[doc(inline)]
    pub use crate::errors_v2::{
        ConfigFailure, EventError, ExportError, FailureClassification, FlushError, IdentityError,
        InitError, LogSinkError, MetricModelError, ProjectionError, ShutdownError, SubscriberError,
        TelemetryError,
    };
    #[doc(inline)]
    pub use crate::observation_v2::{
        LogProjector, MetricProjector, ObservationFilter, ObservationSubscriber,
        ProcessIdentityResolver, ProjectionRegistration, SpanProjector, SubscriberRegistration,
    };
    #[doc(inline)]
    pub use crate::signals_v2::{
        AggregationTemporality, AttributeValue, Attributes, FiniteF64, HistogramPoint,
        MetricRecord, MetricValue, SpanEvent, SpanKind, SpanLink, SpanRecord, SpanSignal,
        SpanState, TraceContext, TraceFlags,
    };
    #[doc(inline)]
    pub use crate::{SpanEnded, SpanStarted, SpanStatus};
}
