//! Resource, scope, log and span records plus their enumerations; checked
//! deserialization shares constructor validation.
use super::{AnyValue, KeyValues, SignalValidationError, TraceState};
use crate::{Level, SpanId, Timestamp, TraceId, constants};
use serde::{Deserialize, Serialize};

/// A signal with its resource and instrumentation context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceRecord<T> {
    /// Resource associated with the record.
    pub resource: Resource,
    /// Instrumentation associated with the record.
    pub scope: InstrumentationScope,
    /// Signal payload.
    pub record: T,
}
impl<T> ResourceRecord<T> {
    /// Attaches metadata without changing the signal.
    pub fn new(resource: Resource, scope: InstrumentationScope, record: T) -> Self {
        Self {
            resource,
            scope,
            record,
        }
    }
}

/// Neutral `Resource` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Resource {
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `entity_refs` field.
    pub entity_refs: Vec<EntityRef>,
    /// Protocol `schema_url` field.
    pub schema_url: Option<String>,
}
impl Resource {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        attributes: KeyValues,
        dropped_attributes_count: u32,
        entity_refs: Vec<EntityRef>,
        schema_url: Option<String>,
    ) -> Self {
        Self {
            attributes,
            dropped_attributes_count,
            entity_refs,
            schema_url,
        }
    }
}

/// Neutral `EntityRef` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EntityRef {
    /// Protocol `schema_url` field.
    pub schema_url: Option<String>,
    /// Protocol `type` field.
    pub r#type: String,
    /// Protocol `id_keys` field.
    pub id_keys: Vec<String>,
    /// Protocol `description_keys` field.
    pub description_keys: Vec<String>,
}
impl EntityRef {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        schema_url: Option<String>,
        r#type: String,
        id_keys: Vec<String>,
        description_keys: Vec<String>,
    ) -> Self {
        Self {
            schema_url,
            r#type,
            id_keys,
            description_keys,
        }
    }
}

/// Neutral `InstrumentationScope` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InstrumentationScope {
    /// Protocol `name` field.
    pub name: String,
    /// Protocol `version` field.
    pub version: Option<String>,
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `schema_url` field.
    pub schema_url: Option<String>,
}
impl InstrumentationScope {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        name: String,
        version: Option<String>,
        attributes: KeyValues,
        dropped_attributes_count: u32,
        schema_url: Option<String>,
    ) -> Self {
        Self {
            name,
            version,
            attributes,
            dropped_attributes_count,
            schema_url,
        }
    }
}

/// Neutral `LogPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogPoint {
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub time: Option<Timestamp>,
    /// Protocol `observed_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub observed_time: Timestamp,
    /// Protocol `severity_number` field.
    pub severity_number: SeverityNumber,
    /// Protocol `severity_text` field.
    pub severity_text: Option<String>,
    /// Protocol `event_name` field.
    pub event_name: Option<String>,
    /// Protocol `body` field.
    pub body: Option<AnyValue>,
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `flags` field.
    pub flags: u32,
    /// Protocol `trace_id` field.
    pub trace_id: Option<TraceId>,
    /// Protocol `span_id` field.
    pub span_id: Option<SpanId>,
}
impl LogPoint {
    /// Constructs this payload.
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors the complete neutral protocol record"
    )]
    #[must_use]
    pub fn new(
        time: Option<Timestamp>,
        observed_time: Timestamp,
        severity_number: SeverityNumber,
        severity_text: Option<String>,
        event_name: Option<String>,
        body: Option<AnyValue>,
        attributes: KeyValues,
        dropped_attributes_count: u32,
        flags: u32,
        trace_id: Option<TraceId>,
        span_id: Option<SpanId>,
    ) -> Self {
        Self {
            time,
            observed_time,
            severity_number,
            severity_text,
            event_name,
            body,
            attributes,
            dropped_attributes_count,
            flags,
            trace_id,
            span_id,
        }
    }
}

/// Neutral `SpanPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SpanPointRaw")]
pub struct SpanPoint {
    /// Protocol `trace_id` field.
    pub trace_id: TraceId,
    /// Protocol `span_id` field.
    pub span_id: SpanId,
    /// Protocol `trace_state` field.
    pub trace_state: Option<TraceState>,
    /// Protocol `parent_span_id` field.
    pub parent_span_id: Option<SpanId>,
    /// Protocol `flags` field.
    pub flags: u32,
    /// Protocol `name` field.
    pub name: String,
    /// Protocol `kind` field.
    pub kind: SpanKindPoint,
    /// Protocol `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub start_time: Timestamp,
    /// Protocol `end_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub end_time: Timestamp,
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `events` field.
    pub events: Vec<SpanEventPoint>,
    /// Protocol `dropped_events_count` field.
    pub dropped_events_count: u32,
    /// Protocol `links` field.
    pub links: Vec<SpanLinkPoint>,
    /// Protocol `dropped_links_count` field.
    pub dropped_links_count: u32,
    /// Protocol `status` field.
    pub status: SpanStatusPoint,
}
impl SpanPoint {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors the complete neutral protocol record"
    )]
    pub fn try_new(
        trace_id: TraceId,
        span_id: SpanId,
        trace_state: Option<TraceState>,
        parent_span_id: Option<SpanId>,
        flags: u32,
        name: String,
        kind: SpanKindPoint,
        start_time: Timestamp,
        end_time: Timestamp,
        attributes: KeyValues,
        dropped_attributes_count: u32,
        events: Vec<SpanEventPoint>,
        dropped_events_count: u32,
        links: Vec<SpanLinkPoint>,
        dropped_links_count: u32,
        status: SpanStatusPoint,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            trace_id,
            span_id,
            trace_state,
            parent_span_id,
            flags,
            name,
            kind,
            start_time,
            end_time,
            attributes,
            dropped_attributes_count,
            events,
            dropped_events_count,
            links,
            dropped_links_count,
            status,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpanPointRaw {
    trace_id: TraceId,
    span_id: SpanId,
    trace_state: Option<TraceState>,
    parent_span_id: Option<SpanId>,
    flags: u32,
    name: String,
    kind: SpanKindPoint,
    start_time: Timestamp,
    end_time: Timestamp,
    attributes: KeyValues,
    dropped_attributes_count: u32,
    events: Vec<SpanEventPoint>,
    dropped_events_count: u32,
    links: Vec<SpanLinkPoint>,
    dropped_links_count: u32,
    status: SpanStatusPoint,
}
impl TryFrom<SpanPointRaw> for SpanPoint {
    type Error = SignalValidationError;
    fn try_from(raw: SpanPointRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.trace_id,
            raw.span_id,
            raw.trace_state,
            raw.parent_span_id,
            raw.flags,
            raw.name,
            raw.kind,
            raw.start_time,
            raw.end_time,
            raw.attributes,
            raw.dropped_attributes_count,
            raw.events,
            raw.dropped_events_count,
            raw.links,
            raw.dropped_links_count,
            raw.status,
        )
    }
}

/// Neutral `SpanEventPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanEventPoint {
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `name` field.
    pub name: String,
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
}
impl SpanEventPoint {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        time: Timestamp,
        name: String,
        attributes: KeyValues,
        dropped_attributes_count: u32,
    ) -> Self {
        Self {
            time,
            name,
            attributes,
            dropped_attributes_count,
        }
    }
}

/// Neutral `SpanLinkPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanLinkPoint {
    /// Protocol `trace_id` field.
    pub trace_id: TraceId,
    /// Protocol `span_id` field.
    pub span_id: SpanId,
    /// Protocol `trace_state` field.
    pub trace_state: Option<TraceState>,
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `flags` field.
    pub flags: u32,
}
impl SpanLinkPoint {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        trace_state: Option<TraceState>,
        attributes: KeyValues,
        dropped_attributes_count: u32,
        flags: u32,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            trace_state,
            attributes,
            dropped_attributes_count,
            flags,
        }
    }
}

/// Neutral `SpanStatusPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpanStatusPoint {
    /// Protocol `code` field.
    pub code: StatusCode,
    /// Protocol `message` field.
    pub message: Option<String>,
}
impl SpanStatusPoint {
    /// Constructs this payload.
    #[must_use]
    pub fn new(code: StatusCode, message: Option<String>) -> Self {
        Self { code, message }
    }
}

/// Span role in the distributed operation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKindPoint {
    /// No role specified.
    #[default]
    Unspecified,
    /// Internal operation.
    Internal,
    /// Server request.
    Server,
    /// Client request.
    Client,
    /// Message producer.
    Producer,
    /// Message consumer.
    Consumer,
}
/// Completion status reported by the producer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCode {
    /// No status set.
    #[default]
    Unset,
    /// Successful operation.
    Ok,
    /// Failed operation.
    Error,
}
/// Numeric OTLP severity in the inclusive range 0 through 24.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8")]
pub struct SeverityNumber(u8);
impl SeverityNumber {
    /// No severity specified.
    pub const UNSPECIFIED: Self = Self(0);
    /// Constructs a numeric OTLP severity.
    /// # Errors
    /// Rejects values outside the protocol range.
    pub fn try_new(value: u8) -> Result<Self, SignalValidationError> {
        if value > constants::OTLP_SEVERITY_MAX {
            Err(SignalValidationError::invalid(
                "severity_number",
                "out of range",
            ))
        } else {
            Ok(Self(value))
        }
    }
    /// Returns the numeric severity.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
    /// The single owner of the `Level` to OTLP severity mapping: the first
    /// number of each OTLP severity range and its upper-case short name,
    /// matching the text the OTLP exporters emit.
    pub(crate) const fn from_level(level: Level) -> (Self, &'static str) {
        match level {
            Level::Trace => (Self(constants::OTLP_SEVERITY_TRACE), "TRACE"),
            Level::Debug => (Self(constants::OTLP_SEVERITY_DEBUG), "DEBUG"),
            Level::Info => (Self(constants::OTLP_SEVERITY_INFO), "INFO"),
            Level::Warn => (Self(constants::OTLP_SEVERITY_WARN), "WARN"),
            Level::Error => (Self(constants::OTLP_SEVERITY_ERROR), "ERROR"),
        }
    }
}
impl TryFrom<u8> for SeverityNumber {
    type Error = SignalValidationError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Self::try_new(v)
    }
}
impl SpanPoint {
    /// Checks span timing.
    /// # Errors
    /// Rejects an end before the actual start.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        if self.end_time < self.start_time {
            return Err(SignalValidationError::invalid(
                "end_time",
                "span ends before start",
            ));
        }
        Ok(())
    }
}
