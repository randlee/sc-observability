pub mod constants;
pub mod error_codes;

use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use time::OffsetDateTime;

pub type Timestamp = OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorCode(Cow<'static, str>);

impl ErrorCode {
    pub const fn new_static(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    pub fn new_owned(code: impl Into<String>) -> Self {
        Self(Cow::Owned(code.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("{message}")]
pub struct ValueValidationError {
    message: String,
}

impl ValueValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

macro_rules! validated_name_type {
    ($name:ident, $doc:literal, $validator:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
                let value = value.into();
                $validator(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableSteps {
    steps: Vec<String>,
}

impl RecoverableSteps {
    pub fn first(&self) -> Option<&str> {
        self.steps.first().map(String::as_str)
    }

    pub fn all(&self) -> &[String] {
        &self.steps
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Remediation {
    Recoverable { steps: RecoverableSteps },
    NotRecoverable { justification: String },
}

impl Remediation {
    pub fn recoverable(
        first: impl Into<String>,
        rest: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut steps = vec![first.into()];
        for value in rest {
            steps.push(value.into());
        }
        Self::Recoverable {
            steps: RecoverableSteps { steps },
        }
    }

    pub fn not_recoverable(justification: impl Into<String>) -> Self {
        Self::NotRecoverable {
            justification: justification.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
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

pub trait DiagnosticInfo {
    fn diagnostic(&self) -> &Diagnostic;
}

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
            at: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorContext {
    diagnostic: Diagnostic,
    #[serde(skip)]
    source: Option<Box<str>>,
}

impl ErrorContext {
    pub fn new(code: ErrorCode, message: impl Into<String>, remediation: Remediation) -> Self {
        Self {
            diagnostic: Diagnostic {
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

    pub fn cause(mut self, cause: impl Into<String>) -> Self {
        self.diagnostic.cause = Some(cause.into());
        self
    }

    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.diagnostic.docs = Some(docs.into());
        self
    }

    pub fn detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.diagnostic.details.insert(key.into(), value);
        self
    }

    pub fn source(mut self, source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        self.source = Some(source.to_string().into_boxed_str());
        self
    }

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
#[error("{0}")]
pub struct IdentityError(pub Box<ErrorContext>);

impl DiagnosticInfo for IdentityError {
    fn diagnostic(&self) -> &Diagnostic {
        self.0.diagnostic()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProcessIdentity {
    pub hostname: Option<String>,
    pub pid: Option<u32>,
}

pub enum ProcessIdentityPolicy {
    Auto,
    Fixed {
        hostname: Option<String>,
        pid: Option<u32>,
    },
    Resolver(Arc<dyn ProcessIdentityResolver>),
}

pub trait ProcessIdentityResolver: Send + Sync {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            constants::TRACE_ID_LEN,
            error_codes::TRACE_ID_INVALID.as_str(),
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(String);

impl SpanId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            constants::SPAN_ID_LEN,
            error_codes::SPAN_ID_INVALID.as_str(),
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_lower_hex(
    value: &str,
    expected_len: usize,
    _code: &str,
) -> Result<(), ValueValidationError> {
    if value.len() != expected_len {
        return Err(ValueValidationError::new(format!(
            "value must be {expected_len} lowercase hex characters"
        )));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "value must contain lowercase hex characters only",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
}

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

pub trait Observable: Send + Sync + 'static {}

impl<T> Observable for T where T: Send + Sync + 'static {}

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
    pub fn new(service: ServiceName, payload: T) -> Self {
        Self {
            version: constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: OffsetDateTime::now_utc(),
            service,
            identity: ProcessIdentity::default(),
            trace: None,
            payload,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanStarted;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanEnded;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanRecord<S> {
    timestamp: Timestamp,
    service: ServiceName,
    name: ActionName,
    trace: TraceContext,
    status: SpanStatus,
    diagnostic: Option<Diagnostic>,
    attributes: Map<String, Value>,
    duration_ms: Option<u64>,
    marker: PhantomData<S>,
}

impl SpanRecord<SpanStarted> {
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

    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    pub fn end(self, status: SpanStatus, duration_ms: u64) -> SpanRecord<SpanEnded> {
        SpanRecord {
            timestamp: self.timestamp,
            service: self.service,
            name: self.name,
            trace: self.trace,
            status,
            diagnostic: self.diagnostic,
            attributes: self.attributes,
            duration_ms: Some(duration_ms),
            marker: PhantomData,
        }
    }
}

impl<S> SpanRecord<S> {
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    pub fn service(&self) -> &ServiceName {
        &self.service
    }

    pub fn name(&self) -> &ActionName {
        &self.name
    }

    pub fn trace(&self) -> &TraceContext {
        &self.trace
    }

    pub fn status(&self) -> SpanStatus {
        self.status
    }

    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    pub fn attributes(&self) -> &Map<String, Value> {
        &self.attributes
    }
}

impl SpanRecord<SpanEnded> {
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms.unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanEvent {
    pub timestamp: Timestamp,
    pub trace: TraceContext,
    pub name: ActionName,
    pub attributes: Map<String, Value>,
    pub diagnostic: Option<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpanSignal {
    Started(SpanRecord<SpanStarted>),
    Event(SpanEvent),
    Ended(SpanRecord<SpanEnded>),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoggingHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SinkHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinkHealth {
    pub name: String,
    pub state: SinkHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingHealthReport {
    pub state: LoggingHealthState,
    pub dropped_events_total: u64,
    pub active_log_path: std::path::PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservabilityHealthReport {
    pub state: ObservationHealthState,
    pub dropped_observations_total: u64,
    pub subscriber_failures_total: u64,
    pub projection_failures_total: u64,
    pub logging: Option<LoggingHealthReport>,
    pub telemetry: Option<TelemetryHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryHealthState {
    Disabled,
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExporterHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExporterHealth {
    pub name: String,
    pub state: ExporterHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryHealthReport {
    pub state: TelemetryHealthState,
    pub dropped_exports_total: u64,
    pub exporter_statuses: Vec<ExporterHealth>,
    pub last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompleteSpan {
    pub record: SpanRecord<SpanEnded>,
    pub events: Vec<SpanEvent>,
}

pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    fn handle(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
}

pub trait ObservationFilter<T>: Send + Sync
where
    T: Observable,
{
    fn accepts(&self, observation: &Observation<T>) -> bool;
}

pub trait LogProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError>;
}

pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError>;
}

pub trait MetricProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}

#[derive(Clone)]
pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    pub subscriber: Arc<dyn ObservationSubscriber<T>>,
    pub filter: Option<Arc<dyn ObservationFilter<T>>>,
}

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

macro_rules! error_wrapper {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
        #[error("{0}")]
        pub struct $name(pub Box<ErrorContext>);

        impl DiagnosticInfo for $name {
            fn diagnostic(&self) -> &Diagnostic {
                self.0.diagnostic()
            }
        }
    };
}

error_wrapper!(InitError);
error_wrapper!(EventError);
error_wrapper!(FlushError);
error_wrapper!(ShutdownError);
error_wrapper!(ProjectionError);
error_wrapper!(SubscriberError);
error_wrapper!(LogSinkError);
error_wrapper!(ExportError);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum ObservationError {
    #[error("observation runtime is shut down")]
    Shutdown,
    #[error("{0}")]
    QueueFull(Box<ErrorContext>),
    #[error("{0}")]
    RoutingFailure(Box<ErrorContext>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum TelemetryError {
    #[error("telemetry runtime is shut down")]
    Shutdown,
    #[error("{0}")]
    ExportFailure(Box<ErrorContext>),
}
