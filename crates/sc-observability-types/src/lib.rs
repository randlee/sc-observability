//! Shared neutral contracts for the `sc-observability` workspace.
//!
//! This crate defines the reusable value types, diagnostics, typestate span
//! contracts, health reports, and open extension traits consumed by the higher
//! layers in the workspace. It intentionally avoids owning sinks, routing
//! runtimes, exporter behavior, or application-specific payload types.

pub mod constants;
pub mod error_codes;
mod errors;
mod health;
mod query;

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Sub};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use thiserror::Error;
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub use errors::{
    EventError, ExportError, FlushError, IdentityError, InitError, LogSinkError, ObservationError,
    ProjectionError, ShutdownError, SubscriberError, TelemetryError,
};
pub use health::{
    ExporterHealth, ExporterHealthState, LoggingHealthReport, LoggingHealthState,
    ObservabilityHealthReport, ObservationHealthState, QueryHealthReport, QueryHealthState,
    SinkHealth, SinkHealthState, TelemetryHealthProvider, TelemetryHealthReport,
    TelemetryHealthState,
};
pub use query::{LogFieldMatch, LogFieldPredicate, LogOrder, LogQuery, LogSnapshot, QueryError};

/// Canonical millisecond duration type used across the workspace.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DurationMs(u64);

impl DurationMs {
    /// Returns the raw millisecond count.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for DurationMs {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DurationMs> for u64 {
    fn from(value: DurationMs) -> Self {
        value.0
    }
}

impl fmt::Display for DurationMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// Canonical UTC timestamp type used across the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    /// Canonical Unix epoch timestamp in UTC.
    pub const UNIX_EPOCH: Self = Self(OffsetDateTime::UNIX_EPOCH);

    /// Returns the current UTC timestamp.
    pub fn now_utc() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    /// Normalizes an arbitrary offset date-time into the canonical UTC timestamp.
    pub fn from_offset_date_time(value: OffsetDateTime) -> Self {
        Self(value.to_offset(UtcOffset::UTC))
    }

    /// Returns the normalized inner UTC date-time value.
    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self::from_offset_date_time(value)
    }
}

impl From<Timestamp> for OffsetDateTime {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self::from_offset_date_time(self.0 + rhs)
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self::from_offset_date_time(self.0 - rhs)
    }
}

impl Sub for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered = self
            .0
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| fmt::Error)?;
        f.write_str(&rendered)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let rendered = self
            .0
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&rendered)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let parsed = OffsetDateTime::parse(&value, &Rfc3339).map_err(serde::de::Error::custom)?;
        Ok(Self::from_offset_date_time(parsed))
    }
}

/// Stable machine-readable error code used across diagnostics and error types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorCode(Cow<'static, str>);

impl ErrorCode {
    /// Creates an error code from a `'static` string without allocating.
    pub const fn new_static(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// Creates an error code from owned or borrowed string data by taking ownership.
    pub fn new_owned(code: impl Into<String>) -> Self {
        Self(Cow::Owned(code.into()))
    }

    /// Returns the string representation of the error code.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

/// Validation error returned when a public value type rejects an input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("{message}")]
pub struct ValueValidationError {
    code: ErrorCode,
    message: String,
}

impl ValueValidationError {
    /// Creates a validation error using the default shared validation code.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: error_codes::VALUE_VALIDATION_FAILED,
            message: message.into(),
        }
    }

    /// Creates a validation error with an explicit stable error code.
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Returns the stable error code associated with the validation failure.
    pub fn code(&self) -> &ErrorCode {
        &self.code
    }
}

macro_rules! validated_name_type {
    ($name:ident, $doc:literal, $validator:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Creates a validated value from caller-provided string data.
            pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
                let value = value.into();
                $validator(&value)?;
                Ok(Self(value))
            }

            /// Returns the underlying validated string value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

fn validate_identifier(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("identifier must not be empty"));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "identifier must match [A-Za-z0-9._-]+",
        ))
    }
}

fn validate_env_prefix(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("env prefix must not be empty"));
    }
    if value.ends_with(constants::DEFAULT_ENV_PREFIX_SEPARATOR) {
        return Err(ValueValidationError::new(
            "env prefix must not end with underscore",
        ));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "env prefix must match [A-Z0-9_]+",
        ))
    }
}

fn validate_metric_name(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("metric name must not be empty"));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "metric name must match [A-Za-z0-9._\\-/]+",
        ))
    }
}

validated_name_type!(
    ToolName,
    "Validated tool identity used for top-level configuration.",
    validate_identifier
);
validated_name_type!(
    EnvPrefix,
    "Validated environment prefix used for config loading namespaces.",
    validate_env_prefix
);
validated_name_type!(
    ServiceName,
    "Validated service name carried in logs and telemetry.",
    validate_identifier
);
validated_name_type!(
    TargetCategory,
    "Validated stable target category for log events.",
    validate_identifier
);
validated_name_type!(
    ActionName,
    "Validated stable action name for log and span events.",
    validate_identifier
);
validated_name_type!(
    MetricName,
    "Validated metric identity using [A-Za-z0-9._\\-/]+.",
    validate_metric_name
);

/// Ordered recovery steps for a recoverable diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableSteps {
    steps: Vec<String>,
}

impl RecoverableSteps {
    /// Creates a recoverable step list containing exactly one first action.
    pub fn first(step: impl Into<String>) -> Self {
        Self {
            steps: vec![step.into()],
        }
    }

    /// Creates a recoverable step list from a full ordered set of actions.
    pub fn all(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            steps: steps.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the first recommended recovery step, if present.
    pub fn first_step(&self) -> Option<&str> {
        self.steps.first().map(String::as_str)
    }

    /// Returns all ordered recovery steps.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }
}

/// Required remediation metadata attached to every diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Remediation {
    Recoverable { steps: RecoverableSteps },
    NotRecoverable { justification: String },
}

impl Remediation {
    /// Builds a recoverable remediation with one required first step and any remaining ordered steps.
    pub fn recoverable(
        first: impl Into<String>,
        rest: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut steps = vec![first.into()];
        steps.extend(rest.into_iter().map(Into::into));
        Self::Recoverable {
            steps: RecoverableSteps::all(steps),
        }
    }

    /// Builds a non-recoverable remediation with the required justification for why recovery is not possible.
    pub fn not_recoverable(justification: impl Into<String>) -> Self {
        Self::NotRecoverable {
            justification: justification.into(),
        }
    }
}

/// Structured diagnostic payload reusable across CLI, logging, and telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub timestamp: Timestamp,
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    pub remediation: Remediation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub details: Map<String, Value>,
}

/// Trait for public error surfaces that can expose an attached diagnostic.
pub trait DiagnosticInfo {
    fn diagnostic(&self) -> &Diagnostic;
}

/// Small diagnostic summary used in health and last-error reporting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    pub code: Option<ErrorCode>,
    pub message: String,
    pub at: Timestamp,
}

impl From<&Diagnostic> for DiagnosticSummary {
    fn from(value: &Diagnostic) -> Self {
        Self {
            code: Some(value.code.clone()),
            message: value.message.clone(),
            at: value.timestamp,
        }
    }
}

/// Builder-style context wrapper used by public crate error types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    diagnostic: Diagnostic,
    #[serde(skip)]
    source: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,
}

impl PartialEq for ErrorContext {
    fn eq(&self, other: &Self) -> bool {
        self.diagnostic == other.diagnostic
            && self.source.as_ref().map(ToString::to_string)
                == other.source.as_ref().map(ToString::to_string)
    }
}

impl ErrorContext {
    /// Creates a new error context with the required code, message, and remediation.
    pub fn new(code: ErrorCode, message: impl Into<String>, remediation: Remediation) -> Self {
        Self {
            diagnostic: Diagnostic {
                timestamp: Timestamp::now_utc(),
                code,
                message: message.into(),
                cause: None,
                remediation,
                docs: None,
                details: Map::new(),
            },
            source: None,
        }
    }

    /// Adds a human-readable cause string to the error context.
    pub fn cause(mut self, cause: impl Into<String>) -> Self {
        self.diagnostic.cause = Some(cause.into());
        self
    }

    /// Adds a documentation reference string to the error context.
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.diagnostic.docs = Some(docs.into());
        self
    }

    /// Adds one structured detail field to the error context.
    pub fn detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.diagnostic.details.insert(key.into(), value);
        self
    }

    /// Captures the real source error for display and error chaining.
    pub fn source(mut self, source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        self.source = Some(Arc::from(source));
        self
    }

    /// Returns the structured diagnostic carried by this error context.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic.message)?;
        if let Some(cause) = &self.diagnostic.cause {
            write!(f, ": {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

/// Canonical event/log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Level threshold used by filtering surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

/// Caller-resolved process identity attached to observations and log events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProcessIdentity {
    pub hostname: Option<String>,
    pub pid: Option<u32>,
}

/// Policy describing how process identity is populated at runtime.
pub enum ProcessIdentityPolicy {
    Auto,
    Fixed {
        hostname: Option<String>,
        pid: Option<u32>,
    },
    Resolver(Arc<dyn ProcessIdentityResolver>),
}

impl std::fmt::Debug for ProcessIdentityPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("ProcessIdentityPolicy::Auto"),
            Self::Fixed { hostname, pid } => f
                .debug_struct("ProcessIdentityPolicy::Fixed")
                .field("hostname", hostname)
                .field("pid", pid)
                .finish(),
            Self::Resolver(_) => {
                f.write_str("ProcessIdentityPolicy::Resolver(<dyn ProcessIdentityResolver>)")
            }
        }
    }
}

/// Open resolver contract for caller-defined process identity lookup.
pub trait ProcessIdentityResolver: Send + Sync {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError>;
}

/// Validated 32-character lowercase hexadecimal trace identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    /// Creates a validated lowercase hexadecimal trace identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            constants::TRACE_ID_LEN,
            &error_codes::TRACE_ID_INVALID,
        )?;
        Ok(Self(value))
    }

    /// Returns the underlying lowercase hexadecimal trace identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated 16-character lowercase hexadecimal span identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(String);

impl SpanId {
    /// Creates a validated lowercase hexadecimal span identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            constants::SPAN_ID_LEN,
            &error_codes::SPAN_ID_INVALID,
        )?;
        Ok(Self(value))
    }

    /// Returns the underlying lowercase hexadecimal span identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_lower_hex(
    value: &str,
    expected_len: usize,
    code: &ErrorCode,
) -> Result<(), ValueValidationError> {
    if value.len() != expected_len {
        return Err(ValueValidationError::with_code(
            code.clone(),
            format!("value must be {expected_len} lowercase hex characters"),
        ));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ValueValidationError::with_code(
            code.clone(),
            "value must contain lowercase hex characters only",
        ))
    }
}

/// Generic trace correlation context shared by logs, spans, and observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
}

/// Typed description of an entity moving from one state to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransition {
    /// Stable category describing what changed, such as `task` or `subagent`.
    pub entity_kind: String,
    /// Optional caller-owned identifier for the entity that changed.
    pub entity_id: Option<String>,
    /// Previous stable state label.
    pub from_state: String,
    /// New stable state label.
    pub to_state: String,
    /// Optional human-readable explanation for why the transition occurred.
    pub reason: Option<String>,
    /// Optional action or event name that triggered the transition.
    pub trigger: Option<String>,
}

/// Marker trait for consumer-owned observation payloads.
pub trait Observable: Send + Sync + 'static {}

impl<T> Observable for T where T: Send + Sync + 'static {}

/// Shared envelope around a typed observation payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation<T>
where
    T: Observable,
{
    pub version: String,
    pub timestamp: Timestamp,
    pub service: ServiceName,
    pub identity: ProcessIdentity,
    pub trace: Option<TraceContext>,
    pub payload: T,
}

impl<T> Observation<T>
where
    T: Observable,
{
    /// Creates a new observation envelope using the current UTC timestamp.
    pub fn new(service: ServiceName, payload: T) -> Self {
        Self {
            version: constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::now_utc(),
            service,
            identity: ProcessIdentity::default(),
            trace: None,
            payload,
        }
    }
}

/// Structured log record emitted by the logging and routing layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub version: String,
    pub timestamp: Timestamp,
    pub level: Level,
    pub service: ServiceName,
    pub target: TargetCategory,
    pub action: ActionName,
    pub message: Option<String>,
    pub identity: ProcessIdentity,
    pub trace: Option<TraceContext>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub outcome: Option<String>,
    pub diagnostic: Option<Diagnostic>,
    pub state_transition: Option<StateTransition>,
    pub fields: Map<String, Value>,
}

/// Final span status for a completed span record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}

/// Typestate marker for a started-but-not-yet-ended span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanStarted;

/// Typestate marker for a completed span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanEnded;

/// Producer-facing span record whose lifecycle is encoded via typestate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanRecord<S> {
    timestamp: Timestamp,
    service: ServiceName,
    name: ActionName,
    trace: TraceContext,
    status: SpanStatus,
    diagnostic: Option<Diagnostic>,
    attributes: Map<String, Value>,
    duration_ms: Option<DurationMs>,
    marker: PhantomData<S>,
}

impl SpanRecord<SpanStarted> {
    /// Creates a new started span record.
    pub fn new(
        timestamp: Timestamp,
        service: ServiceName,
        name: ActionName,
        trace: TraceContext,
        attributes: Map<String, Value>,
    ) -> Self {
        Self {
            timestamp,
            service,
            name,
            trace,
            status: SpanStatus::Unset,
            diagnostic: None,
            attributes,
            duration_ms: None,
            marker: PhantomData,
        }
    }

    /// Attaches a diagnostic to the started span before completion.
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Consumes the started span and returns the only valid completed span form.
    pub fn end(self, status: SpanStatus, duration: DurationMs) -> SpanRecord<SpanEnded> {
        SpanRecord {
            timestamp: self.timestamp,
            service: self.service,
            name: self.name,
            trace: self.trace,
            status,
            diagnostic: self.diagnostic,
            attributes: self.attributes,
            duration_ms: Some(duration),
            marker: PhantomData,
        }
    }
}

impl<S> SpanRecord<S> {
    /// Returns the timestamp recorded for the span lifecycle event.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns the service that emitted the span.
    pub fn service(&self) -> &ServiceName {
        &self.service
    }

    /// Returns the stable action/name associated with the span.
    pub fn name(&self) -> &ActionName {
        &self.name
    }

    /// Returns the trace context for the span.
    pub fn trace(&self) -> &TraceContext {
        &self.trace
    }

    /// Returns the current typestate-derived span status.
    pub fn status(&self) -> SpanStatus {
        self.status
    }

    /// Returns the optional diagnostic attached to the span.
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    /// Returns immutable span attributes.
    pub fn attributes(&self) -> &Map<String, Value> {
        &self.attributes
    }
}

impl SpanRecord<SpanEnded> {
    /// Returns the final duration, available only on completed spans.
    pub fn duration_ms(&self) -> DurationMs {
        self.duration_ms
            .expect("SpanRecord<SpanEnded> always has duration_ms set by end()")
    }
}

/// Event attached to a span timeline without creating a child span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanEvent {
    pub timestamp: Timestamp,
    pub trace: TraceContext,
    pub name: ActionName,
    pub attributes: Map<String, Value>,
    pub diagnostic: Option<Diagnostic>,
}

/// Generic span lifecycle signal used by projectors and telemetry assembly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SpanSignal {
    Started(SpanRecord<SpanStarted>),
    Event(SpanEvent),
    Ended(SpanRecord<SpanEnded>),
}

/// Supported metric aggregation shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

/// Structured metric observation projected from routing or telemetry layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub timestamp: Timestamp,
    pub service: ServiceName,
    pub name: MetricName,
    pub kind: MetricKind,
    pub value: f64,
    /// Optional UCUM unit string, for example `ms`, `By`, or `1`.
    pub unit: Option<String>,
    pub attributes: Map<String, Value>,
}

/// Open subscriber contract for typed observations.
pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    fn handle(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
}

/// Open filter contract evaluated before subscriber or projector execution.
pub trait ObservationFilter<T>: Send + Sync
where
    T: Observable,
{
    fn accepts(&self, observation: &Observation<T>) -> bool;
}

/// Open projector contract from typed observations into log events.
pub trait LogProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError>;
}

/// Open projector contract from typed observations into span signals.
pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError>;
}

/// Open projector contract from typed observations into metric records.
pub trait MetricProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}

/// Construction-time registration for one typed observation subscriber.
#[derive(Clone)]
pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    pub subscriber: Arc<dyn ObservationSubscriber<T>>,
    pub filter: Option<Arc<dyn ObservationFilter<T>>>,
}

/// Construction-time registration for log/span/metric projection of a payload.
#[derive(Clone)]
pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    pub log_projector: Option<Arc<dyn LogProjector<T>>>,
    pub span_projector: Option<Arc<dyn SpanProjector<T>>>,
    pub metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    pub filter: Option<Arc<dyn ObservationFilter<T>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use time::{OffsetDateTime, UtcOffset};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct FixturePayload {
        name: String,
        count: u32,
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn target_category() -> TargetCategory {
        TargetCategory::new("routing.core").expect("valid target category")
    }

    fn action_name() -> ActionName {
        ActionName::new("observation.received").expect("valid action name")
    }

    fn metric_name() -> MetricName {
        MetricName::new("obs.events_total").expect("valid metric name")
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: Some(SpanId::new("fedcba9876543210").expect("valid parent span id")),
        }
    }

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: error_codes::DIAGNOSTIC_INVALID,
            message: "diagnostic invalid".to_string(),
            cause: Some("invalid example".to_string()),
            remediation: Remediation::recoverable(
                "fix the input",
                ["rerun the command", "review the docs"],
            ),
            docs: Some("https://example.test/docs".to_string()),
            details: Map::from_iter([("key".to_string(), json!("value"))]),
        }
    }

    #[test]
    fn remediation_construction_helpers_cover_both_variants() {
        let recoverable = Remediation::recoverable("fix the input", ["retry"]);
        let not_recoverable = Remediation::not_recoverable("manual intervention required");
        let first_only = RecoverableSteps::first("first");
        let all_steps = RecoverableSteps::all(["first", "second"]);

        match recoverable {
            Remediation::Recoverable { steps } => {
                assert_eq!(steps.first_step(), Some("fix the input"));
                assert_eq!(
                    steps.steps(),
                    ["fix the input".to_string(), "retry".to_string()]
                );
            }
            Remediation::NotRecoverable { .. } => panic!("expected recoverable remediation"),
        }

        match not_recoverable {
            Remediation::NotRecoverable { justification } => {
                assert_eq!(justification, "manual intervention required");
            }
            Remediation::Recoverable { .. } => panic!("expected non-recoverable remediation"),
        }

        assert_eq!(first_only.first_step(), Some("first"));
        assert_eq!(first_only.steps(), ["first".to_string()]);
        assert_eq!(
            all_steps.steps(),
            ["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn validated_name_newtypes_accept_expected_values() {
        assert_eq!(
            ToolName::new("codex-cli")
                .expect("valid tool name")
                .as_str(),
            "codex-cli"
        );
        assert_eq!(
            ToolName::new("codex-cli")
                .expect("valid tool name")
                .to_string(),
            "codex-cli"
        );
        assert_eq!(
            EnvPrefix::new("SC_OBSERVABILITY")
                .expect("valid env prefix")
                .as_str(),
            "SC_OBSERVABILITY"
        );
        assert_eq!(
            ServiceName::new("service.core")
                .expect("valid service name")
                .as_str(),
            "service.core"
        );
        assert_eq!(
            TargetCategory::new("pipeline-ingest")
                .expect("valid target category")
                .as_str(),
            "pipeline-ingest"
        );
        assert_eq!(
            ActionName::new("observation.received")
                .expect("valid action name")
                .as_str(),
            "observation.received"
        );
        assert_eq!(
            MetricName::new("obs/events_total")
                .expect("valid metric name")
                .as_str(),
            "obs/events_total"
        );
    }

    #[test]
    fn validated_name_newtypes_reject_invalid_values() {
        assert!(ToolName::new("").is_err());
        assert!(EnvPrefix::new("sc_observability").is_err());
        assert!(EnvPrefix::new("SC_OBSERVABILITY_").is_err());
        assert!(ServiceName::new("service core").is_err());
        assert!(TargetCategory::new("category/invalid").is_err());
        assert!(ActionName::new("action invalid").is_err());
        assert!(MetricName::new("metric name").is_err());
    }

    #[test]
    fn trace_and_span_ids_validate_w3c_shapes() {
        assert!(TraceId::new("0123456789abcdef0123456789abcdef").is_ok());
        let short_trace = TraceId::new("0123456789abcdef0123456789abcde")
            .expect_err("short trace id should fail");
        assert_eq!(short_trace.code(), &error_codes::TRACE_ID_INVALID);
        let uppercase_trace = TraceId::new("0123456789ABCDEF0123456789abcdef")
            .expect_err("uppercase trace id should fail");
        assert_eq!(uppercase_trace.code(), &error_codes::TRACE_ID_INVALID);

        assert!(SpanId::new("0123456789abcdef").is_ok());
        let short_span = SpanId::new("0123456789abcde").expect_err("short span id should fail");
        assert_eq!(short_span.code(), &error_codes::SPAN_ID_INVALID);
        let uppercase_span =
            SpanId::new("0123456789ABCDEf").expect_err("uppercase span id should fail");
        assert_eq!(uppercase_span.code(), &error_codes::SPAN_ID_INVALID);
    }

    #[test]
    fn error_context_display_includes_cause_when_present() {
        let error = ErrorContext::new(
            error_codes::DIAGNOSTIC_INVALID,
            "operation failed",
            Remediation::recoverable("fix the config", ["retry"]),
        )
        .cause("missing field");

        assert_eq!(error.to_string(), "operation failed: missing field");
    }

    #[test]
    fn error_context_builder_sets_docs_details_and_source() {
        let error = ErrorContext::new(
            error_codes::DIAGNOSTIC_INVALID,
            "operation failed",
            Remediation::not_recoverable("investigate manually"),
        )
        .docs("https://example.test/failure")
        .detail("attempt", json!(3))
        .source(Box::new(std::io::Error::other("disk full")));

        assert_eq!(
            error.diagnostic().docs.as_deref(),
            Some("https://example.test/failure")
        );
        assert_eq!(error.diagnostic().details.get("attempt"), Some(&json!(3)));
        assert_eq!(
            error.source.as_ref().map(ToString::to_string).as_deref(),
            Some("disk full")
        );
        assert_eq!(
            std::error::Error::source(&error)
                .map(ToString::to_string)
                .as_deref(),
            Some("disk full")
        );
    }

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
        assert_eq!(source.to_string(), "operation failed");
        assert_eq!(
            source.source().map(ToString::to_string).as_deref(),
            Some("disk full")
        );
    }

    #[test]
    fn diagnostic_round_trips_through_serde() {
        let original = diagnostic();
        let encoded = serde_json::to_string(&original).expect("serialize diagnostic");
        let decoded: Diagnostic = serde_json::from_str(&encoded).expect("deserialize diagnostic");
        assert_eq!(decoded, original);
    }

    #[test]
    fn diagnostic_summary_captures_code_and_message() {
        let summary = DiagnosticSummary::from(&diagnostic());

        assert_eq!(summary.code, Some(error_codes::DIAGNOSTIC_INVALID));
        assert_eq!(summary.message, "diagnostic invalid");
        assert!(summary.at <= Timestamp::now_utc());
    }

    #[test]
    fn observation_round_trips_through_serde() {
        let mut observation = Observation::new(
            service_name(),
            FixturePayload {
                name: "agent-info".to_string(),
                count: 2,
            },
        );
        observation.identity = ProcessIdentity {
            hostname: Some("host-1".to_string()),
            pid: Some(42),
        };
        observation.trace = Some(trace_context());

        let encoded = serde_json::to_string(&observation).expect("serialize observation");
        let decoded: Observation<FixturePayload> =
            serde_json::from_str(&encoded).expect("deserialize observation");
        assert_eq!(decoded, observation);
    }

    #[test]
    fn log_event_round_trips_through_serde() {
        let event = LogEvent {
            version: constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: target_category(),
            action: action_name(),
            message: Some("observation accepted".to_string()),
            identity: ProcessIdentity {
                hostname: Some("host-1".to_string()),
                pid: Some(7),
            },
            trace: Some(trace_context()),
            request_id: Some("req-1".to_string()),
            correlation_id: Some("corr-1".to_string()),
            outcome: Some("success".to_string()),
            diagnostic: Some(diagnostic()),
            state_transition: Some(StateTransition {
                entity_kind: "subagent".to_string(),
                entity_id: Some("agent-1".to_string()),
                from_state: "started".to_string(),
                to_state: "running".to_string(),
                reason: Some("hook received".to_string()),
                trigger: Some("subagent-start".to_string()),
            }),
            fields: Map::from_iter([("attempt".to_string(), json!(1))]),
        };

        let encoded = serde_json::to_string(&event).expect("serialize log event");
        let decoded: LogEvent = serde_json::from_str(&encoded).expect("deserialize log event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn span_signal_round_trips_through_serde() {
        let mut attributes = Map::new();
        attributes.insert("tool".to_string(), json!("rg"));

        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            attributes.clone(),
        )
        .with_diagnostic(diagnostic());

        let ended = started.clone().end(SpanStatus::Ok, DurationMs::from(123));
        let signal = SpanSignal::Ended(ended);
        let encoded = serde_json::to_value(&signal).expect("serialize span signal");

        assert_eq!(encoded["Ended"]["status"], "Ok");
        assert_eq!(encoded["Ended"]["duration_ms"], 123);
    }

    #[test]
    fn metric_record_round_trips_through_serde() {
        let metric = MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: service_name(),
            name: metric_name(),
            kind: MetricKind::Counter,
            value: 4.0,
            unit: Some("1".to_string()),
            attributes: Map::from_iter([("state".to_string(), json!("running"))]),
        };

        let encoded = serde_json::to_string(&metric).expect("serialize metric");
        let decoded: MetricRecord = serde_json::from_str(&encoded).expect("deserialize metric");
        assert_eq!(decoded, metric);
    }

    #[test]
    fn span_record_end_transitions_to_span_ended() {
        let span = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            Map::new(),
        );

        let ended = span.end(SpanStatus::Error, DurationMs::from(88));

        assert_eq!(ended.status(), SpanStatus::Error);
        assert_eq!(ended.duration_ms(), DurationMs::from(88));
        assert_eq!(ended.service().as_str(), "sc-observability");
    }

    #[test]
    fn observation_new_sets_defaults() {
        let observation = Observation::new(
            service_name(),
            FixturePayload {
                name: "payload".to_string(),
                count: 1,
            },
        );

        assert_eq!(observation.version, constants::OBSERVATION_ENVELOPE_VERSION);
        assert_eq!(observation.identity, ProcessIdentity::default());
        assert!(observation.trace.is_none());
    }

    #[test]
    fn span_record_accessors_preserve_started_values() {
        let mut attributes = Map::new();
        attributes.insert("count".to_string(), json!(2));
        let span = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            attributes.clone(),
        )
        .with_diagnostic(diagnostic());

        assert_eq!(span.timestamp(), Timestamp::UNIX_EPOCH);
        assert_eq!(span.name().as_str(), "observation.received");
        assert_eq!(
            span.trace().trace_id.as_str(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(span.status(), SpanStatus::Unset);
        assert_eq!(span.diagnostic(), Some(&diagnostic()));
        assert_eq!(span.attributes(), &attributes);
    }

    #[test]
    fn timestamp_serde_round_trips_as_utc_rfc3339() {
        let timestamp = Timestamp::from(
            OffsetDateTime::UNIX_EPOCH.to_offset(UtcOffset::from_hms(2, 0, 0).expect("offset")),
        );
        let encoded = serde_json::to_string(&timestamp).expect("serialize timestamp");
        let decoded: Timestamp = serde_json::from_str(&encoded).expect("deserialize timestamp");

        assert_eq!(encoded, "\"1970-01-01T00:00:00Z\"");
        assert_eq!(decoded, timestamp);
    }

    #[test]
    fn duration_ms_displays_in_milliseconds() {
        assert_eq!(DurationMs::from(250).to_string(), "250ms");
    }

    #[test]
    fn health_reports_round_trip_through_serde() {
        let sink = SinkHealth {
            name: "jsonl".to_string(),
            state: SinkHealthState::Healthy,
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };
        let logging = LoggingHealthReport {
            state: LoggingHealthState::Healthy,
            dropped_events_total: 0,
            flush_errors_total: 0,
            // fixture path: not accessed on disk
            active_log_path: std::path::PathBuf::from("/var/log/service/logs/service.log.jsonl"),
            sink_statuses: vec![sink],
            query: Some(QueryHealthReport {
                state: QueryHealthState::Healthy,
                last_error: None,
            }),
            last_error: None,
        };
        let telemetry = TelemetryHealthReport {
            state: TelemetryHealthState::Healthy,
            dropped_exports_total: 1,
            malformed_spans_total: 0,
            exporter_statuses: vec![ExporterHealth {
                name: "otlp".to_string(),
                state: ExporterHealthState::Degraded,
                last_error: Some(DiagnosticSummary::from(&diagnostic())),
            }],
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };
        let report = ObservabilityHealthReport {
            state: ObservationHealthState::Degraded,
            dropped_observations_total: 2,
            subscriber_failures_total: 3,
            projection_failures_total: 4,
            logging: Some(logging),
            telemetry: Some(telemetry),
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };

        let encoded = serde_json::to_string(&report).expect("serialize observability health");
        let decoded: ObservabilityHealthReport =
            serde_json::from_str(&encoded).expect("deserialize observability health");
        assert_eq!(decoded, report);
    }
}
