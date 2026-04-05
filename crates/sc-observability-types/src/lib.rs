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
mod events;
mod health;
mod level;
mod metric;
mod primitives;
mod process;
mod projection;
mod query;
mod span;
mod tracing;
mod validation;

mod sealed {
    pub trait Sealed {}
}

#[doc(hidden)]
pub mod telemetry_health_provider_sealed {
    pub trait Sealed {}
}

#[doc(inline)]
pub use diagnostic::{
    Diagnostic, DiagnosticInfo, DiagnosticSummary, ErrorContext, RecoverableSteps, Remediation,
};
#[doc(inline)]
pub use errors::{
    EventError, ExportError, FlushError, IdentityError, InitError, LogSinkError, ObservationError,
    ProjectionError, ShutdownError, SubscriberError, TelemetryError,
};
#[doc(inline)]
pub use events::{LogEvent, Observable, Observation};
#[doc(inline)]
pub use health::{
    ExporterHealth, ExporterHealthState, LoggingHealthReport, LoggingHealthState,
    ObservabilityHealthProvider, ObservabilityHealthReport, ObservationHealthState,
    QueryHealthReport, QueryHealthState, SinkHealth, SinkHealthState, TelemetryHealthReport,
    TelemetryHealthState,
};
#[doc(inline)]
pub use level::{Level, LevelFilter};
#[doc(inline)]
pub use metric::{MetricKind, MetricRecord};
#[doc(inline)]
pub use primitives::{DurationMs, ErrorCode, Timestamp};
#[doc(inline)]
pub use process::{ProcessIdentity, ProcessIdentityPolicy, ProcessIdentityResolver};
#[doc(inline)]
pub use projection::{
    LogProjector, MetricProjector, ObservationFilter, ObservationSubscriber,
    ProjectionRegistration, SpanProjector, SubscriberRegistration,
};
#[doc(inline)]
pub use query::{LogFieldMatch, LogOrder, LogQuery, LogSnapshot, QueryError};
#[doc(inline)]
pub use span::{SpanEnded, SpanEvent, SpanRecord, SpanSignal, SpanStarted, SpanStatus};
#[doc(inline)]
pub use tracing::{SpanId, StateTransition, TraceContext, TraceId};
#[doc(inline)]
pub use validation::{
    ActionName, EnvPrefix, MetricName, ServiceName, TargetCategory, ToolName, ValueValidationError,
};
