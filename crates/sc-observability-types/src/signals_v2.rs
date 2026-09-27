//! Neutral 2.0 span and metric values with checked construction and serde input.
use crate::errors_v2::MetricModelError;
use crate::{
    ActionName, Diagnostic, DurationMs, ErrorContext, MetricName, MetricUnit, Remediation,
    ServiceName, SpanEnded, SpanId, SpanStarted, SpanStatus, Timestamp, TraceId,
    ValueValidationError, error_codes,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::marker::PhantomData;

/// Finite floating-point value; serde applies the same validation as construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct FiniteF64(f64);
impl FiniteF64 {
    /// Validates a floating-point value.
    ///
    /// # Errors
    /// Rejects NaN and positive or negative infinity.
    pub fn new(value: f64) -> Result<Self, ValueValidationError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(ValueValidationError::with_code(
                error_codes::SC_METRIC_NON_FINITE,
                "numeric values must be finite",
            ))
        }
    }
    /// Returns the finite value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}
impl TryFrom<f64> for FiniteF64 {
    type Error = ValueValidationError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<FiniteF64> for f64 {
    fn from(value: FiniteF64) -> Self {
        value.0
    }
}
impl std::fmt::Display for FiniteF64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Neutral attribute values without transport or JSON-library types in the API.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    /// Boolean value.
    Bool(bool),
    /// Signed integer, preserved without floating-point coercion.
    Int(i64),
    /// Unsigned integer, preserved without floating-point coercion.
    UInt(u64),
    /// Finite floating-point value.
    Float(FiniteF64),
    /// Text value.
    String(String),
    /// Ordered neutral values.
    Array(Vec<AttributeValue>),
    /// Named neutral values.
    Object(Attributes),
    /// Explicit null value.
    Null,
}
/// Named neutral attributes with deterministic key ordering.
pub type Attributes = BTreeMap<String, AttributeValue>;

/// W3C trace flags, retaining every supplied bit across serialization.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceFlags(u8);
impl TraceFlags {
    /// Preserves the trace flags byte without discarding future bits.
    #[must_use]
    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }
    /// Returns the original flags byte.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
    /// Returns whether the W3C sampled bit is set.
    #[must_use]
    pub const fn sampled(self) -> bool {
        self.0 & crate::constants::TRACE_FLAG_SAMPLED != 0
    }
}
/// Generic W3C correlation; application metadata belongs outside this context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    /// Validated W3C trace identifier.
    pub trace_id: TraceId,
    /// Validated W3C span identifier.
    pub span_id: SpanId,
    /// Optional parent span identifier.
    pub parent_span_id: Option<SpanId>,
    /// W3C flags, including sampling.
    pub flags: TraceFlags,
}
impl TraceContext {
    /// Creates trace correlation with explicit flags and no parent.
    #[must_use]
    pub fn new(trace_id: TraceId, span_id: SpanId, flags: TraceFlags) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
            flags,
        }
    }
    /// Sets the optional parent correlation.
    #[must_use]
    pub fn with_parent(mut self, parent: SpanId) -> Self {
        self.parent_span_id = Some(parent);
        self
    }
}
/// Relationship between a span and its callers or downstream services.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    /// Internal work.
    Internal,
    /// Incoming synchronous request.
    Server,
    /// Outgoing synchronous request.
    Client,
    /// Message production.
    Producer,
    /// Message consumption.
    Consumer,
}
/// Link to another span with a single authoritative set of identifiers and flags.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanLink {
    /// Linked trace identifier.
    pub trace_id: TraceId,
    /// Linked span identifier.
    pub span_id: SpanId,
    /// Linked trace flags.
    pub flags: TraceFlags,
    /// Link attributes.
    pub attributes: Attributes,
}
impl SpanLink {
    /// Creates a link without redundant nested trace context.
    #[must_use]
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        flags: TraceFlags,
        attributes: Attributes,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            flags,
            attributes,
        }
    }
}
/// Time window interpretation for aggregated metric points.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationTemporality {
    /// A nonempty interval since the preceding collection.
    Delta,
    /// An interval since the sequence start, possibly its initial zero-length point.
    Cumulative,
}
/// Validated explicit histogram distribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistogramInput")]
pub struct HistogramPoint {
    explicit_bounds: Vec<f64>,
    bucket_counts: Vec<u64>,
    count: u64,
    sum: FiniteF64,
}
#[derive(Deserialize)]
struct HistogramInput {
    explicit_bounds: Vec<f64>,
    bucket_counts: Vec<u64>,
    count: u64,
    sum: FiniteF64,
}
impl TryFrom<HistogramInput> for HistogramPoint {
    type Error = MetricModelError;
    fn try_from(v: HistogramInput) -> Result<Self, Self::Error> {
        Self::try_new(v.explicit_bounds, v.bucket_counts, v.count, v.sum)
    }
}
impl HistogramPoint {
    /// Checks ordered finite bounds, bucket cardinality, count and empty-point sum.
    ///
    /// # Errors
    /// Returns `InvalidHistogram` for non-finite or unordered bounds, mismatched
    /// bucket lengths, overflowing/mismatched counts, or nonzero sum with zero count.
    pub fn try_new(
        explicit_bounds: Vec<f64>,
        bucket_counts: Vec<u64>,
        count: u64,
        sum: FiniteF64,
    ) -> Result<Self, MetricModelError> {
        if explicit_bounds.iter().any(|b| !b.is_finite())
            || explicit_bounds.windows(2).any(|b| b[0] >= b[1])
            || explicit_bounds.len().checked_add(1) != Some(bucket_counts.len())
            || bucket_counts
                .iter()
                .try_fold(0u64, |total, n| total.checked_add(*n))
                != Some(count)
            || (count == 0 && sum.get() != 0.0)
        {
            return Err(MetricModelError::InvalidHistogram {
                context: Box::new(ErrorContext::new(
                    error_codes::SC_METRIC_INVALID_HISTOGRAM,
                    "invalid histogram distribution",
                    Remediation::recoverable(
                        "Provide finite increasing bounds, one more bucket than bounds, and matching count/sum",
                        [] as [&str; 0],
                    ),
                )),
            });
        }
        Ok(Self {
            explicit_bounds,
            bucket_counts,
            count,
            sum,
        })
    }
    /// Returns strictly increasing finite bounds.
    #[must_use]
    pub fn explicit_bounds(&self) -> &[f64] {
        &self.explicit_bounds
    }
    /// Returns counts, including the final unbounded bucket.
    #[must_use]
    pub fn bucket_counts(&self) -> &[u64] {
        &self.bucket_counts
    }
    /// Returns the checked total sample count.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }
    /// Returns the finite sum of samples.
    #[must_use]
    pub const fn sum(&self) -> FiniteF64 {
        self.sum
    }
}
/// Discriminated metric aggregation with no scalar histogram placeholders.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum MetricValue {
    /// Instantaneous finite measurement.
    Gauge(FiniteF64),
    /// Aggregated sum with explicit interval semantics.
    Sum {
        /// Finite sum.
        value: FiniteF64,
        /// Whether values grow monotonically within a sequence.
        monotonic: bool,
        /// Delta or cumulative aggregation.
        temporality: AggregationTemporality,
        /// Interval start, relative to the record timestamp.
        start_time: Timestamp,
    },
    /// Explicit histogram with its aggregation interval.
    Histogram {
        /// Validated sample distribution.
        point: HistogramPoint,
        /// Delta or cumulative aggregation.
        temporality: AggregationTemporality,
        /// Interval start, relative to the record timestamp.
        start_time: Timestamp,
    },
}
impl MetricValue {
    /// Validates interval semantics against the point's end timestamp.
    ///
    /// # Errors
    /// `InvalidInterval` means start is after end. `InvalidTemporality` means
    /// an empty delta interval or a negative monotonic sum. Cumulative points
    /// may start at their end timestamp, representing the initial point.
    pub fn validate_at(&self, timestamp: Timestamp) -> Result<(), MetricModelError> {
        let (temporality, start_time) = match self {
            Self::Gauge(_) => return Ok(()),
            Self::Sum {
                value,
                monotonic: true,
                ..
            } if value.get() < 0.0 => {
                return Err(invalid_temporality("monotonic sums cannot be negative"));
            }
            Self::Sum {
                temporality,
                start_time,
                ..
            }
            | Self::Histogram {
                temporality,
                start_time,
                ..
            } => (*temporality, *start_time),
        };
        if start_time > timestamp {
            return Err(MetricModelError::InvalidInterval {
                context: Box::new(ErrorContext::new(
                    error_codes::SC_METRIC_INVALID_INTERVAL,
                    "metric start is after its timestamp",
                    Remediation::recoverable(
                        "Set start_time at or before the point timestamp",
                        [] as [&str; 0],
                    ),
                )),
            });
        }
        if temporality == AggregationTemporality::Delta && start_time == timestamp {
            return Err(invalid_temporality("delta interval must be nonempty"));
        }
        Ok(())
    }
}
fn invalid_temporality(message: &str) -> MetricModelError {
    MetricModelError::InvalidTemporality {
        context: Box::new(ErrorContext::new(
            error_codes::SC_METRIC_INVALID_TEMPORALITY,
            message,
            Remediation::recoverable(
                "Provide a positive delta interval and nonnegative monotonic sum",
                [] as [&str; 0],
            ),
        )),
    }
}
/// Metric point whose interval is checked during construction and deserialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MetricInput")]
pub struct MetricRecord {
    timestamp: Timestamp,
    service: ServiceName,
    name: MetricName,
    value: MetricValue,
    unit: Option<MetricUnit>,
    attributes: Attributes,
}
#[derive(Deserialize)]
struct MetricInput {
    timestamp: Timestamp,
    service: ServiceName,
    name: MetricName,
    value: MetricValue,
    unit: Option<MetricUnit>,
    attributes: Attributes,
}
impl TryFrom<MetricInput> for MetricRecord {
    type Error = MetricModelError;
    fn try_from(v: MetricInput) -> Result<Self, Self::Error> {
        Ok(Self::try_new(v.timestamp, v.service, v.name, v.value)?
            .with_unit(v.unit)
            .with_attributes(v.attributes))
    }
}
impl MetricRecord {
    /// Creates a metric point with validated temporal semantics.
    ///
    /// # Errors
    /// Returns the interval or temporality error reported by `MetricValue::validate_at`.
    pub fn try_new(
        timestamp: Timestamp,
        service: ServiceName,
        name: MetricName,
        value: MetricValue,
    ) -> Result<Self, MetricModelError> {
        value.validate_at(timestamp)?;
        Ok(Self {
            timestamp,
            service,
            name,
            value,
            unit: None,
            attributes: Attributes::new(),
        })
    }
    /// Sets a validated optional unit.
    #[must_use]
    pub fn with_unit(mut self, unit: Option<MetricUnit>) -> Self {
        self.unit = unit;
        self
    }
    /// Sets neutral attributes.
    #[must_use]
    pub fn with_attributes(mut self, attributes: Attributes) -> Self {
        self.attributes = attributes;
        self
    }
    /// Returns the point timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
    /// Returns the producing service.
    #[must_use]
    pub fn service(&self) -> &ServiceName {
        &self.service
    }
    /// Returns the metric name.
    #[must_use]
    pub fn name(&self) -> &MetricName {
        &self.name
    }
    /// Returns the validated aggregation.
    #[must_use]
    pub fn value(&self) -> &MetricValue {
        &self.value
    }
    /// Returns the optional unit.
    #[must_use]
    pub fn unit(&self) -> Option<&MetricUnit> {
        self.unit.as_ref()
    }
    /// Returns the neutral attributes.
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

/// Producer-facing span record whose lifecycle is encoded via typestate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanRecord<S> {
    timestamp: Timestamp,
    service: ServiceName,
    name: ActionName,
    trace: TraceContext,
    status: SpanStatus,
    diagnostic: Option<Diagnostic>,
    attributes: Attributes,
    duration_ms: Option<DurationMs>,
    kind: SpanKind,
    links: Vec<SpanLink>,
    #[serde(skip)]
    marker: PhantomData<S>,
}

impl SpanRecord<SpanStarted> {
    /// Creates a new started span record.
    #[must_use]
    pub fn new(
        timestamp: Timestamp,
        service: ServiceName,
        name: ActionName,
        trace: TraceContext,
        attributes: Attributes,
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
            kind: SpanKind::Internal,
            links: Vec::new(),
            marker: PhantomData,
        }
    }

    /// Sets the span's role.
    #[must_use]
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }
    /// Sets links to other spans.
    #[must_use]
    pub fn with_links(mut self, links: Vec<SpanLink>) -> Self {
        self.links = links;
        self
    }
    /// Attaches a diagnostic to the started span before completion.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Consumes the started span and returns the only valid completed span form.
    #[must_use]
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
            kind: self.kind,
            links: self.links,
            marker: PhantomData,
        }
    }
}

impl<S> SpanRecord<S> {
    /// Returns the span role.
    #[must_use]
    pub fn kind(&self) -> SpanKind {
        self.kind
    }
    /// Returns links to other spans.
    #[must_use]
    pub fn links(&self) -> &[SpanLink] {
        &self.links
    }

    /// Returns the timestamp recorded for the span lifecycle event.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns the service that emitted the span.
    #[must_use]
    pub fn service(&self) -> &ServiceName {
        &self.service
    }

    /// Returns the stable action/name associated with the span.
    #[must_use]
    pub fn name(&self) -> &ActionName {
        &self.name
    }

    /// Returns the trace context for the span.
    #[must_use]
    pub fn trace(&self) -> &TraceContext {
        &self.trace
    }

    /// Returns the current typestate-derived span status.
    #[must_use]
    pub fn status(&self) -> SpanStatus {
        self.status
    }

    /// Returns the optional diagnostic attached to the span.
    #[must_use]
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    /// Returns immutable span attributes.
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

impl SpanRecord<SpanEnded> {
    /// Returns the final duration recorded for the completed span.
    ///
    /// Only `end` constructs this typestate, so a duration is always present.
    #[must_use]
    pub fn duration_ms(&self) -> Option<DurationMs> {
        self.duration_ms
    }
}

/// Event attached to a span timeline without creating a child span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanEvent {
    /// UTC event timestamp.
    pub timestamp: Timestamp,
    /// Trace/span correlation for the event.
    pub trace: TraceContext,
    /// Stable event name.
    pub name: ActionName,
    /// Structured event attributes.
    pub attributes: Attributes,
    /// Optional diagnostic attached to the event.
    pub diagnostic: Option<Diagnostic>,
}

/// Generic span lifecycle signal used by projectors and telemetry assembly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SpanSignal {
    /// Started span record.
    Started(SpanRecord<SpanStarted>),
    /// Point-in-time event on an existing span.
    Event(SpanEvent),
    /// Completed span record.
    Ended(SpanRecord<SpanEnded>),
}
