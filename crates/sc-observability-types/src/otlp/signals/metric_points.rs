//! Metric streams, data points and exemplars; checked deserialization shares
//! constructor validation.
use super::metrics::{DataPointFlags, MetricData, NumberValue};
use super::{KeyValues, OtlpDouble, SignalValidationError};
use crate::{MetricName, SpanId, Timestamp, TraceId};
use serde::{Deserialize, Serialize};

/// Neutral `MetricStream` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MetricStreamRaw")]
pub struct MetricStream {
    /// Protocol `name` field.
    pub name: MetricName,
    /// Protocol `description` field.
    pub description: Option<String>,
    /// Protocol `unit` field.
    pub unit: Option<String>,
    /// Protocol `metadata` field.
    pub metadata: KeyValues,
    /// Protocol `data` field.
    pub data: MetricData,
}
impl MetricStream {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    pub fn try_new(
        name: MetricName,
        description: Option<String>,
        unit: Option<String>,
        metadata: KeyValues,
        data: MetricData,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            name,
            description,
            unit,
            metadata,
            data,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricStreamRaw {
    name: MetricName,
    description: Option<String>,
    unit: Option<String>,
    metadata: KeyValues,
    data: MetricData,
}
impl TryFrom<MetricStreamRaw> for MetricStream {
    type Error = SignalValidationError;
    fn try_from(raw: MetricStreamRaw) -> Result<Self, Self::Error> {
        Self::try_new(raw.name, raw.description, raw.unit, raw.metadata, raw.data)
    }
}

/// Neutral `NumberPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NumberPointRaw")]
pub struct NumberPoint {
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub start_time: Option<Timestamp>,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `value` field.
    pub value: NumberValue,
    /// Protocol `exemplars` field.
    pub exemplars: Vec<Exemplar>,
    /// Protocol `flags` field.
    pub flags: DataPointFlags,
}
impl NumberPoint {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    pub fn try_new(
        attributes: KeyValues,
        start_time: Option<Timestamp>,
        time: Timestamp,
        value: NumberValue,
        exemplars: Vec<Exemplar>,
        flags: DataPointFlags,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            attributes,
            start_time,
            time,
            value,
            exemplars,
            flags,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberPointRaw {
    attributes: KeyValues,
    start_time: Option<Timestamp>,
    time: Timestamp,
    value: NumberValue,
    exemplars: Vec<Exemplar>,
    flags: DataPointFlags,
}
impl TryFrom<NumberPointRaw> for NumberPoint {
    type Error = SignalValidationError;
    fn try_from(raw: NumberPointRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.attributes,
            raw.start_time,
            raw.time,
            raw.value,
            raw.exemplars,
            raw.flags,
        )
    }
}

/// Neutral `HistogramDataPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistogramDataPointRaw")]
pub struct HistogramDataPoint {
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub start_time: Option<Timestamp>,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `count` field.
    pub count: u64,
    /// Protocol `sum` field.
    pub sum: Option<OtlpDouble>,
    /// Protocol `bucket_counts` field.
    pub bucket_counts: Vec<u64>,
    /// Protocol `explicit_bounds` field.
    pub explicit_bounds: Vec<OtlpDouble>,
    /// Protocol `exemplars` field.
    pub exemplars: Vec<Exemplar>,
    /// Protocol `flags` field.
    pub flags: DataPointFlags,
    /// Protocol `min` field.
    pub min: Option<OtlpDouble>,
    /// Protocol `max` field.
    pub max: Option<OtlpDouble>,
}
impl HistogramDataPoint {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors the complete neutral protocol record"
    )]
    pub fn try_new(
        attributes: KeyValues,
        start_time: Option<Timestamp>,
        time: Timestamp,
        count: u64,
        sum: Option<OtlpDouble>,
        bucket_counts: Vec<u64>,
        explicit_bounds: Vec<OtlpDouble>,
        exemplars: Vec<Exemplar>,
        flags: DataPointFlags,
        min: Option<OtlpDouble>,
        max: Option<OtlpDouble>,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            attributes,
            start_time,
            time,
            count,
            sum,
            bucket_counts,
            explicit_bounds,
            exemplars,
            flags,
            min,
            max,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistogramDataPointRaw {
    attributes: KeyValues,
    start_time: Option<Timestamp>,
    time: Timestamp,
    count: u64,
    sum: Option<OtlpDouble>,
    bucket_counts: Vec<u64>,
    explicit_bounds: Vec<OtlpDouble>,
    exemplars: Vec<Exemplar>,
    flags: DataPointFlags,
    min: Option<OtlpDouble>,
    max: Option<OtlpDouble>,
}
impl TryFrom<HistogramDataPointRaw> for HistogramDataPoint {
    type Error = SignalValidationError;
    fn try_from(raw: HistogramDataPointRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.attributes,
            raw.start_time,
            raw.time,
            raw.count,
            raw.sum,
            raw.bucket_counts,
            raw.explicit_bounds,
            raw.exemplars,
            raw.flags,
            raw.min,
            raw.max,
        )
    }
}

/// Neutral `ExponentialBuckets` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExponentialBuckets {
    /// Protocol `offset` field.
    pub offset: i32,
    /// Protocol `bucket_counts` field.
    pub bucket_counts: Vec<u64>,
}
impl ExponentialBuckets {
    /// Constructs this payload.
    #[must_use]
    pub fn new(offset: i32, bucket_counts: Vec<u64>) -> Self {
        Self {
            offset,
            bucket_counts,
        }
    }
}

/// Neutral `ExponentialHistogramDataPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ExponentialHistogramDataPointRaw")]
pub struct ExponentialHistogramDataPoint {
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub start_time: Option<Timestamp>,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `count` field.
    pub count: u64,
    /// Protocol `sum` field.
    pub sum: Option<OtlpDouble>,
    /// Protocol `scale` field.
    pub scale: i32,
    /// Protocol `zero_count` field.
    pub zero_count: u64,
    /// Protocol `zero_threshold` field.
    pub zero_threshold: OtlpDouble,
    /// Protocol `positive` field.
    pub positive: ExponentialBuckets,
    /// Protocol `negative` field.
    pub negative: ExponentialBuckets,
    /// Protocol `exemplars` field.
    pub exemplars: Vec<Exemplar>,
    /// Protocol `flags` field.
    pub flags: DataPointFlags,
    /// Protocol `min` field.
    pub min: Option<OtlpDouble>,
    /// Protocol `max` field.
    pub max: Option<OtlpDouble>,
}
impl ExponentialHistogramDataPoint {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors the complete neutral protocol record"
    )]
    pub fn try_new(
        attributes: KeyValues,
        start_time: Option<Timestamp>,
        time: Timestamp,
        count: u64,
        sum: Option<OtlpDouble>,
        scale: i32,
        zero_count: u64,
        zero_threshold: OtlpDouble,
        positive: ExponentialBuckets,
        negative: ExponentialBuckets,
        exemplars: Vec<Exemplar>,
        flags: DataPointFlags,
        min: Option<OtlpDouble>,
        max: Option<OtlpDouble>,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            attributes,
            start_time,
            time,
            count,
            sum,
            scale,
            zero_count,
            zero_threshold,
            positive,
            negative,
            exemplars,
            flags,
            min,
            max,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExponentialHistogramDataPointRaw {
    attributes: KeyValues,
    start_time: Option<Timestamp>,
    time: Timestamp,
    count: u64,
    sum: Option<OtlpDouble>,
    scale: i32,
    zero_count: u64,
    zero_threshold: OtlpDouble,
    positive: ExponentialBuckets,
    negative: ExponentialBuckets,
    exemplars: Vec<Exemplar>,
    flags: DataPointFlags,
    min: Option<OtlpDouble>,
    max: Option<OtlpDouble>,
}
impl TryFrom<ExponentialHistogramDataPointRaw> for ExponentialHistogramDataPoint {
    type Error = SignalValidationError;
    fn try_from(raw: ExponentialHistogramDataPointRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.attributes,
            raw.start_time,
            raw.time,
            raw.count,
            raw.sum,
            raw.scale,
            raw.zero_count,
            raw.zero_threshold,
            raw.positive,
            raw.negative,
            raw.exemplars,
            raw.flags,
            raw.min,
            raw.max,
        )
    }
}

/// Neutral `ValueAtQuantile` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ValueAtQuantileRaw")]
pub struct ValueAtQuantile {
    /// Protocol `quantile` field.
    pub quantile: OtlpDouble,
    /// Protocol `value` field.
    pub value: OtlpDouble,
}
impl ValueAtQuantile {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    pub fn try_new(quantile: OtlpDouble, value: OtlpDouble) -> Result<Self, SignalValidationError> {
        let value = Self { quantile, value };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueAtQuantileRaw {
    quantile: OtlpDouble,
    value: OtlpDouble,
}
impl TryFrom<ValueAtQuantileRaw> for ValueAtQuantile {
    type Error = SignalValidationError;
    fn try_from(raw: ValueAtQuantileRaw) -> Result<Self, Self::Error> {
        Self::try_new(raw.quantile, raw.value)
    }
}

/// Neutral `SummaryDataPoint` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SummaryDataPointRaw")]
pub struct SummaryDataPoint {
    /// Protocol `attributes` field.
    pub attributes: KeyValues,
    /// Protocol `start_time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::optional")]
    pub start_time: Option<Timestamp>,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `count` field.
    pub count: u64,
    /// Protocol `sum` field.
    pub sum: OtlpDouble,
    /// Protocol `quantile_values` field.
    pub quantile_values: Vec<ValueAtQuantile>,
    /// Protocol `flags` field.
    pub flags: DataPointFlags,
}
impl SummaryDataPoint {
    /// Constructs this payload after validating its invariants.
    /// # Errors
    /// Rejects invalid intervals, bounds, counts, or metric semantics.
    pub fn try_new(
        attributes: KeyValues,
        start_time: Option<Timestamp>,
        time: Timestamp,
        count: u64,
        sum: OtlpDouble,
        quantile_values: Vec<ValueAtQuantile>,
        flags: DataPointFlags,
    ) -> Result<Self, SignalValidationError> {
        let value = Self {
            attributes,
            start_time,
            time,
            count,
            sum,
            quantile_values,
            flags,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SummaryDataPointRaw {
    attributes: KeyValues,
    start_time: Option<Timestamp>,
    time: Timestamp,
    count: u64,
    sum: OtlpDouble,
    quantile_values: Vec<ValueAtQuantile>,
    flags: DataPointFlags,
}
impl TryFrom<SummaryDataPointRaw> for SummaryDataPoint {
    type Error = SignalValidationError;
    fn try_from(raw: SummaryDataPointRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.attributes,
            raw.start_time,
            raw.time,
            raw.count,
            raw.sum,
            raw.quantile_values,
            raw.flags,
        )
    }
}

/// Neutral `Exemplar` payload from the pinned protocol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exemplar {
    /// Protocol `filtered_attributes` field.
    pub filtered_attributes: KeyValues,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `value` field.
    pub value: NumberValue,
    /// Protocol `trace_id` field.
    pub trace_id: Option<TraceId>,
    /// Protocol `span_id` field.
    pub span_id: Option<SpanId>,
}
impl Exemplar {
    /// Constructs this payload.
    #[must_use]
    pub fn new(
        filtered_attributes: KeyValues,
        time: Timestamp,
        value: NumberValue,
        trace_id: Option<TraceId>,
        span_id: Option<SpanId>,
    ) -> Self {
        Self {
            filtered_attributes,
            time,
            value,
            trace_id,
            span_id,
        }
    }
}
impl MetricStream {
    /// Checks all metric data, including mutable point fields.
    /// # Errors
    /// Rejects invalid point or aggregation semantics.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        self.data.validate()
    }
}
