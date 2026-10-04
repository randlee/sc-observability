//! Input documents and canonical durable envelopes.
use super::{EnvelopeVersion, RecordKey};
use crate::otlp::signals::{
    AnyValue, InstrumentationScope, KeyValues, LogPoint, MetricStream, Profile, ProfilesDictionary,
    Resource, ResourceRecord, SeverityNumber, SpanEventPoint, SpanKindPoint, SpanLinkPoint,
    SpanPoint, SpanStatusPoint,
};
use crate::{SpanId, Timestamp, TraceId};
use serde::{Deserialize, Serialize};

/// `SubmissionInput` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionInput {
    /// Submission `version` field.
    pub version: EnvelopeVersion,
    /// Submission `record_key` field.
    pub record_key: Option<RecordKey>,
    /// Submission `resource` field.
    pub resource: Option<Resource>,
    /// Submission `scope` field.
    pub scope: Option<InstrumentationScope>,
    /// Submission `logs` field.
    #[serde(default)]
    pub logs: Vec<LogInput>,
    /// Submission `spans` field.
    #[serde(default)]
    pub spans: Vec<SpanInput>,
    /// Submission `metrics` field.
    #[serde(default)]
    pub metrics: Vec<MetricInput>,
    /// Submission `profiles` field.
    pub profiles: Option<ProfilesInput>,
}

/// `LogInput` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogInput {
    /// Submission `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub time: Option<Timestamp>,
    /// Submission `observed_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub observed_time: Option<Timestamp>,
    /// Submission `severity_number` field.
    pub severity_number: Option<SeverityNumber>,
    /// Submission `severity_text` field.
    pub severity_text: Option<String>,
    /// Submission `event_name` field.
    pub event_name: Option<String>,
    /// Submission `body` field.
    pub body: Option<AnyValue>,
    /// Submission `attributes` field.
    #[serde(default)]
    pub attributes: KeyValues,
    /// Submission `dropped_attributes_count` field.
    #[serde(default)]
    pub dropped_attributes_count: u32,
    /// Submission `flags` field.
    #[serde(default)]
    pub flags: u32,
    /// Submission `trace_id` field.
    pub trace_id: Option<TraceId>,
    /// Submission `span_id` field.
    pub span_id: Option<SpanId>,
    /// Submission `correlation_id` field.
    pub correlation_id: Option<String>,
    /// Submission `resource` field.
    pub resource: Option<Resource>,
    /// Submission `scope` field.
    pub scope: Option<InstrumentationScope>,
}

/// `SpanInput` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanInput {
    /// Submission `trace_id` field.
    pub trace_id: Option<TraceId>,
    /// Submission `span_id` field.
    pub span_id: Option<SpanId>,
    /// Submission `trace_state` field.
    pub trace_state: Option<String>,
    /// Submission `parent_span_id` field.
    pub parent_span_id: Option<SpanId>,
    /// Submission `flags` field.
    #[serde(default)]
    pub flags: u32,
    /// Submission `name` field.
    pub name: String,
    /// Submission `kind` field.
    pub kind: Option<SpanKindPoint>,
    /// Submission `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub start_time: Timestamp,
    /// Submission `end_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub end_time: Option<Timestamp>,
    /// Submission `duration_nanos` field.
    pub duration_nanos: Option<u64>,
    /// Submission `attributes` field.
    #[serde(default)]
    pub attributes: KeyValues,
    /// Submission `dropped_attributes_count` field.
    #[serde(default)]
    pub dropped_attributes_count: u32,
    /// Submission `events` field.
    #[serde(default)]
    pub events: Vec<SpanEventPoint>,
    /// Submission `dropped_events_count` field.
    #[serde(default)]
    pub dropped_events_count: u32,
    /// Submission `links` field.
    #[serde(default)]
    pub links: Vec<SpanLinkPoint>,
    /// Submission `dropped_links_count` field.
    #[serde(default)]
    pub dropped_links_count: u32,
    /// Submission `status` field.
    pub status: Option<SpanStatusPoint>,
    /// Submission `correlation_id` field.
    pub correlation_id: Option<String>,
    /// Submission `resource` field.
    pub resource: Option<Resource>,
    /// Submission `scope` field.
    pub scope: Option<InstrumentationScope>,
}

/// `ProfilesInput` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesInput {
    /// Submission `dictionary` field.
    pub dictionary: ProfilesDictionary,
    /// Submission `profiles` field.
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// Submission `resource` field.
    pub resource: Option<Resource>,
    /// Submission `scope` field.
    pub scope: Option<InstrumentationScope>,
}

/// `SubmissionEnvelope` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EnvelopeRaw")]
pub struct SubmissionEnvelope {
    /// Submission `version` field.
    pub version: EnvelopeVersion,
    /// Submission `record_key` field.
    pub record_key: Option<RecordKey>,
    /// Submission `logs` field.
    pub logs: Vec<ResourceRecord<LogPoint>>,
    /// Submission `spans` field.
    pub spans: Vec<ResourceRecord<SpanPoint>>,
    /// Submission `metrics` field.
    pub metrics: Vec<ResourceRecord<MetricStream>>,
    /// Submission `profiles` field.
    pub profiles: Option<ProfilesSubmission>,
}

/// `ProfilesSubmission` shared by Rust, Python and the CLI.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesSubmission {
    /// Submission `dictionary` field.
    pub dictionary: ProfilesDictionary,
    /// Submission `profiles` field.
    pub profiles: Vec<ResourceRecord<Profile>>,
}

impl SubmissionInput {
    /// Creates an empty current-version input, ready for signal records.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: EnvelopeVersion::CURRENT,
            record_key: None,
            resource: None,
            scope: None,
            logs: vec![],
            spans: vec![],
            metrics: vec![],
            profiles: None,
        }
    }
}
impl Default for SubmissionInput {
    fn default() -> Self {
        Self::new()
    }
}
impl LogInput {
    /// Creates a log with no invented event time or correlation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            time: None,
            observed_time: None,
            severity_number: None,
            severity_text: None,
            event_name: None,
            body: None,
            attributes: KeyValues::default(),
            dropped_attributes_count: 0,
            flags: 0,
            trace_id: None,
            span_id: None,
            correlation_id: None,
            resource: None,
            scope: None,
        }
    }
}
impl Default for LogInput {
    fn default() -> Self {
        Self::new()
    }
}
impl SpanInput {
    /// Creates an input with an actual start; callers must set end or duration.
    #[must_use]
    pub fn new(name: String, start_time: Timestamp) -> Self {
        Self {
            trace_id: None,
            span_id: None,
            trace_state: None,
            parent_span_id: None,
            flags: 0,
            name,
            kind: None,
            start_time,
            end_time: None,
            duration_nanos: None,
            attributes: KeyValues::default(),
            dropped_attributes_count: 0,
            events: vec![],
            dropped_events_count: 0,
            links: vec![],
            dropped_links_count: 0,
            status: None,
            correlation_id: None,
            resource: None,
            scope: None,
        }
    }
}
impl ProfilesInput {
    /// Creates a profiles input using envelope resource/scope defaults.
    #[must_use]
    pub fn new(dictionary: ProfilesDictionary, profiles: Vec<Profile>) -> Self {
        Self {
            dictionary,
            profiles,
            resource: None,
            scope: None,
        }
    }
}
/// Metric stream plus optional per-record resource and scope overrides.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricInput {
    /// Complete metric payload.
    #[serde(flatten)]
    pub stream: MetricStream,
    /// Resource override.
    pub resource: Option<Resource>,
    /// Instrumentation override.
    pub scope: Option<InstrumentationScope>,
}
impl MetricInput {
    /// Creates an input that inherits envelope metadata.
    #[must_use]
    pub fn new(stream: MetricStream) -> Self {
        Self {
            stream,
            resource: None,
            scope: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeRaw {
    version: EnvelopeVersion,
    record_key: Option<RecordKey>,
    logs: Vec<ResourceRecord<LogPoint>>,
    spans: Vec<ResourceRecord<SpanPoint>>,
    metrics: Vec<ResourceRecord<MetricStream>>,
    profiles: Option<ProfilesSubmission>,
}
impl TryFrom<EnvelopeRaw> for SubmissionEnvelope {
    type Error = super::SubmissionError;
    fn try_from(raw: EnvelopeRaw) -> Result<Self, Self::Error> {
        let envelope = Self {
            version: raw.version,
            record_key: raw.record_key,
            logs: raw.logs,
            spans: raw.spans,
            metrics: raw.metrics,
            profiles: raw.profiles,
        };
        envelope.validate()?;
        Ok(envelope)
    }
}
