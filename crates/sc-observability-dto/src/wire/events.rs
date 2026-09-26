//! Event, query, and diagnostic wire records.
use super::primitives::{
    LevelDto, LogOrderDto, ProcessIdentityDto, RemediationDto, StateTransitionDto,
    StoredDiagnosticDto, TraceContextDto, ValueDto, default_limit,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version-one LogEventDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LogEventDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// level.
    pub level: LevelDto,
    /// target.
    pub target: String,
    /// action.
    pub action: String,
    /// message.
    pub message: Option<String>,
    /// trace.
    pub trace: Option<TraceContextDto>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// outcome.
    pub outcome: Option<String>,
    /// fields.
    #[serde(default)]
    /// Wire fields.
    pub fields: BTreeMap<String, ValueDto>,
}

/// Version-one StoredEventDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StoredEventDto {
    /// version.
    pub version: String,
    /// timestamp.
    pub timestamp: String,
    /// service.
    pub service: String,
    /// identity.
    pub identity: ProcessIdentityDto,
    /// level.
    pub level: LevelDto,
    /// target.
    pub target: String,
    /// action.
    pub action: String,
    /// message.
    pub message: Option<String>,
    /// trace.
    pub trace: Option<TraceContextDto>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// outcome.
    pub outcome: Option<String>,
    /// fields.
    pub fields: BTreeMap<String, ValueDto>,
    /// diagnostic.
    pub diagnostic: Option<StoredDiagnosticDto>,
    /// state transition.
    pub state_transition: Option<StateTransitionDto>,
}

/// Version-one FieldMatchDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FieldMatchDto {
    /// field.
    pub field: String,
    /// value.
    pub value: ValueDto,
}

/// Version-one LogQueryDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LogQueryDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// service.
    pub service: Option<String>,
    /// levels.
    #[serde(default)]
    /// Wire levels.
    pub levels: Vec<LevelDto>,
    /// target.
    pub target: Option<String>,
    /// action.
    pub action: Option<String>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// since.
    pub since: Option<String>,
    /// until.
    pub until: Option<String>,
    /// field matches.
    #[serde(default)]
    /// Wire field matches.
    pub field_matches: Vec<FieldMatchDto>,
    /// limit.
    #[serde(default = "default_limit")]
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = crate::constants::MAX_QUERY_LIMIT)))]
    /// Wire limit.
    pub limit: usize,
    /// order.
    #[serde(default)]
    /// Wire order.
    pub order: LogOrderDto,
}

/// Version-one LogSnapshotDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LogSnapshotDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// events.
    pub events: Vec<StoredEventDto>,
    /// truncated.
    pub truncated: bool,
}

/// Version-one DiagnosticSummaryDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct DiagnosticSummaryDto {
    /// code.
    pub code: Option<String>,
    /// message.
    pub message: String,
    /// at.
    pub at: String,
}

/// Version-one Diagnostic wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct Diagnostic {
    /// at.
    pub at: String,
    /// code.
    pub code: String,
    /// message.
    pub message: String,
    /// remediation.
    pub remediation: RemediationDto,
}

/// Diagnostic retained by an unsuccessful transition event.
pub type OperationDiagnosticDto = Diagnostic;

/// Additive canonical diagnostic projection; source objects remain native.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct CanonicalDiagnosticDto {
    /// Existing required timestamp/code/message/remediation fields.
    #[serde(flatten)]
    pub diagnostic: Diagnostic,
    /// Already-redacted human-readable cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// Documentation reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    /// Bounded structured details using the lossless integer codec.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("default" = serde_json::json!({}))))]
    pub details: BTreeMap<String, ValueDto>,
}

/// Staged trace correlation including the complete W3C flags byte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TraceContextV2Dto {
    /// Lowercase W3C trace id.
    pub trace_id: String,
    /// Lowercase W3C span id.
    pub span_id: String,
    /// Optional parent span id.
    pub parent_span_id: Option<String>,
    /// All trace flags, including reserved bits.
    pub flags: u8,
}
/// Independent link correlation without a duplicate nested trace context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanLinkDto {
    /// Linked trace id.
    pub trace_id: String,
    /// Linked span id.
    pub span_id: String,
    /// Linked flags.
    pub flags: u8,
    /// Neutral attributes.
    pub attributes: BTreeMap<String, ValueDto>,
}
/// Staged span role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SpanKindDto {
    /// Internal work.
    Internal,
    /// Incoming request.
    Server,
    /// Outgoing request.
    Client,
    /// Message producer.
    Producer,
    /// Message consumer.
    Consumer,
}
/// Staged aggregation interval interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AggregationTemporalityDto {
    /// Nonempty collection interval.
    Delta,
    /// Sequence-start interval.
    Cumulative,
}
/// Explicit distribution; checked conversion validates cross-field invariants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistogramPointDto {
    /// Finite, strictly increasing bounds.
    pub explicit_bounds: Vec<f64>,
    /// One more count than bounds; canonical unsigned decimal strings.
    pub bucket_counts: Vec<super::primitives::DecimalDto>,
    /// Checked total sample count as a canonical unsigned decimal string.
    pub count: super::primitives::DecimalDto,
    /// Finite sample sum.
    pub sum: f64,
}
/// Discriminated aggregation, preserving its interval and all histogram data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MetricValueDto {
    /// Instantaneous finite scalar.
    Gauge(f64),
    /// Finite aggregated sum.
    Sum {
        /// Sum value.
        value: f64,
        /// Monotonic sequence indicator.
        monotonic: bool,
        /// Aggregation interpretation.
        temporality: AggregationTemporalityDto,
        /// UTC start of interval.
        start_time: String,
    },
    /// Full explicit histogram.
    Histogram {
        /// Validated distribution on checked conversion.
        point: HistogramPointDto,
        /// Aggregation interpretation.
        temporality: AggregationTemporalityDto,
        /// UTC start of interval.
        start_time: String,
    },
}
/// Staged metric point with lossless numeric and temporal projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricRecordDto {
    /// UTC point timestamp.
    pub timestamp: String,
    /// Validated service name.
    pub service: String,
    /// Validated metric name.
    pub name: String,
    /// Aggregated value.
    pub value: MetricValueDto,
    /// Optional validated unit.
    pub unit: Option<String>,
    /// Neutral attributes with tagged integer/text distinction.
    pub attributes: BTreeMap<String, ValueDto>,
}
/// Completed or active span outcome, matching native spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub enum SpanStatusDto {
    /// Successful completion.
    Ok,
    /// Failed completion.
    Error,
    /// No explicit outcome.
    Unset,
}
/// Raw span wire record; checked conversion replays the native typestate API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanRecordDto {
    /// UTC start timestamp.
    pub timestamp: String,
    /// Producing service.
    pub service: String,
    /// Span action.
    pub name: String,
    /// W3C correlation.
    pub trace: TraceContextV2Dto,
    /// Outcome; active spans require Unset.
    pub status: SpanStatusDto,
    /// Optional structured diagnostic.
    pub diagnostic: Option<StoredDiagnosticDto>,
    /// Neutral span attributes.
    pub attributes: BTreeMap<String, ValueDto>,
    /// Present only for ended spans, represented as unsigned decimal milliseconds.
    pub duration_ms: Option<super::primitives::DecimalDto>,
    /// Span role.
    pub kind: SpanKindDto,
    /// Independent links.
    pub links: Vec<SpanLinkDto>,
}
/// Span event without a lifecycle transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanEventDto {
    /// UTC event timestamp.
    pub timestamp: String,
    /// W3C correlation.
    pub trace: TraceContextV2Dto,
    /// Event action.
    pub name: String,
    /// Neutral attributes.
    pub attributes: BTreeMap<String, ValueDto>,
    /// Optional diagnostic.
    pub diagnostic: Option<StoredDiagnosticDto>,
}
/// Native-compatible external state tag; never accepts an unknown state as success.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub enum SpanSignalDto {
    /// Active span; no duration.
    Started(SpanRecordDto),
    /// Point event.
    Event(SpanEventDto),
    /// Completed span with required duration.
    Ended(SpanRecordDto),
}
