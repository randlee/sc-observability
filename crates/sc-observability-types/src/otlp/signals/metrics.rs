//! Metric aggregation and point validation.
use super::{
    ExponentialHistogramDataPoint, HistogramDataPoint, NumberPoint, OtlpDouble,
    SignalValidationError, SummaryDataPoint, ValueAtQuantile,
};
use crate::{Timestamp, constants};
use serde::{Deserialize, Serialize};

/// Lossless signed integer or IEEE-754 measurement.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum NumberValue {
    /// Signed integer measurement.
    Int(i64),
    /// Floating-point measurement.
    Double(OtlpDouble),
}
/// All protocol datapoint flag bits, including `NO_RECORDED_VALUE`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataPointFlags(u32);
impl DataPointFlags {
    /// Preserves every input flag bit.
    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }
    /// Returns all flag bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}
/// Aggregation interval semantics.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationTemporality {
    /// Invalid on aggregating streams; retained to report invalid input.
    Unspecified,
    /// Measurements since the previous interval.
    Delta,
    /// Measurements since the cumulative start.
    Cumulative,
}

/// The five supported metric payload forms.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    try_from = "MetricDataRaw"
)]
pub enum MetricData {
    /// Instantaneous measurements.
    Gauge {
        /// Ordered measurements.
        points: Vec<NumberPoint>,
    },
    /// Aggregated sum.
    Sum {
        /// Ordered measurements.
        points: Vec<NumberPoint>,
        /// Interval semantics.
        temporality: AggregationTemporality,
        /// Whether the sum is nondecreasing.
        monotonic: bool,
    },
    /// Explicit-bound histograms.
    Histogram {
        /// Ordered histograms.
        points: Vec<HistogramDataPoint>,
        /// Interval semantics.
        temporality: AggregationTemporality,
    },
    /// Exponential-bucket histograms.
    ExponentialHistogram {
        /// Ordered histograms.
        points: Vec<ExponentialHistogramDataPoint>,
        /// Interval semantics.
        temporality: AggregationTemporality,
    },
    /// Imported statistical summaries.
    Summary {
        /// Ordered summaries.
        points: Vec<SummaryDataPoint>,
    },
}
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum MetricDataRaw {
    Gauge {
        points: Vec<NumberPoint>,
    },
    Sum {
        points: Vec<NumberPoint>,
        temporality: AggregationTemporality,
        monotonic: bool,
    },
    Histogram {
        points: Vec<HistogramDataPoint>,
        temporality: AggregationTemporality,
    },
    ExponentialHistogram {
        points: Vec<ExponentialHistogramDataPoint>,
        temporality: AggregationTemporality,
    },
    Summary {
        points: Vec<SummaryDataPoint>,
    },
}
impl TryFrom<MetricDataRaw> for MetricData {
    type Error = SignalValidationError;
    fn try_from(raw: MetricDataRaw) -> Result<Self, Self::Error> {
        let value = match raw {
            MetricDataRaw::Gauge { points } => Self::Gauge { points },
            MetricDataRaw::Sum {
                points,
                temporality,
                monotonic,
            } => Self::Sum {
                points,
                temporality,
                monotonic,
            },
            MetricDataRaw::Histogram {
                points,
                temporality,
            } => Self::Histogram {
                points,
                temporality,
            },
            MetricDataRaw::ExponentialHistogram {
                points,
                temporality,
            } => Self::ExponentialHistogram {
                points,
                temporality,
            },
            MetricDataRaw::Summary { points } => Self::Summary { points },
        };
        value.validate()?;
        Ok(value)
    }
}
impl MetricData {
    /// Validates every point and its aggregation semantics.
    /// # Errors
    /// Rejects unspecified temporality, negative monotonic sums, and invalid intervals or points.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        match self {
            Self::Gauge { points } => {
                for p in points {
                    p.validate()?;
                }
            }
            Self::Sum {
                points,
                temporality,
                monotonic,
            } => {
                temporal(*temporality, None, None)?;
                for p in points {
                    p.validate()?;
                    temporal(*temporality, p.start_time, Some(p.time))?;
                    if *monotonic
                        && match p.value {
                            NumberValue::Int(v) => v < 0,
                            NumberValue::Double(v) => v.get() < 0.0,
                        }
                    {
                        return Err(invalid("value", "monotonic sum is negative"));
                    }
                }
            }
            Self::Histogram {
                points,
                temporality,
            } => {
                temporal(*temporality, None, None)?;
                for p in points {
                    p.validate()?;
                    temporal(*temporality, p.start_time, Some(p.time))?;
                }
            }
            Self::ExponentialHistogram {
                points,
                temporality,
            } => {
                temporal(*temporality, None, None)?;
                for p in points {
                    p.validate()?;
                    temporal(*temporality, p.start_time, Some(p.time))?;
                }
            }
            Self::Summary { points } => {
                for p in points {
                    p.validate()?;
                }
            }
        }
        Ok(())
    }
    /// Checks a programmatically built variant before accepting it.
    /// # Errors
    /// Uses the same validation as deserialization.
    pub fn try_new(value: Self) -> Result<Self, SignalValidationError> {
        value.validate()?;
        Ok(value)
    }
}
fn invalid(path: &str, why: &str) -> SignalValidationError {
    SignalValidationError::invalid(path, why)
}
fn interval(start: Option<Timestamp>, end: Timestamp) -> Result<(), SignalValidationError> {
    if start.is_some_and(|s| s > end) {
        Err(invalid("start_time", "interval starts after point"))
    } else {
        Ok(())
    }
}
fn temporal(
    t: AggregationTemporality,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
) -> Result<(), SignalValidationError> {
    if t == AggregationTemporality::Unspecified {
        return Err(invalid(
            "temporality",
            "unspecified aggregation temporality",
        ));
    }
    if t == AggregationTemporality::Delta && end.is_some_and(|e| start.is_none_or(|s| s >= e)) {
        return Err(invalid("start_time", "delta interval must be nonempty"));
    }
    Ok(())
}
fn total(values: impl IntoIterator<Item = u64>) -> Result<u64, SignalValidationError> {
    values.into_iter().try_fold(0u64, |a, b| {
        a.checked_add(b)
            .ok_or_else(|| invalid("count", "bucket count overflow"))
    })
}
impl NumberPoint {
    /// Checks interval ordering.
    /// # Errors
    /// Rejects a start after the measurement time.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        interval(self.start_time, self.time)
    }
}
impl HistogramDataPoint {
    /// Checks bounds, buckets, counts, and interval ordering.
    /// # Errors
    /// Rejects NaN/non-increasing bounds, invalid bucket dimensions, count overflow or mismatch.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        interval(self.start_time, self.time)?;
        if self
            .explicit_bounds
            .iter()
            .any(|b| b.get().is_nan() || b.get() == f64::NEG_INFINITY)
            || self
                .explicit_bounds
                .windows(2)
                .any(|w| w[0].get() >= w[1].get())
        {
            return Err(invalid(
                "explicit_bounds",
                "bounds must increase without NaN or negative infinity",
            ));
        }
        if self.bucket_counts.len() != self.explicit_bounds.len() + 1 {
            return Err(invalid(
                "bucket_counts",
                "expected one more bucket than bounds",
            ));
        }
        if total(self.bucket_counts.iter().copied())? != self.count {
            return Err(invalid("count", "bucket sum differs from count"));
        }
        Ok(())
    }
}
impl ExponentialHistogramDataPoint {
    /// Checks scale, count totals, threshold, and interval.
    /// # Errors
    /// Rejects out-of-range scale, negative/NaN threshold, or mismatched counts.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        interval(self.start_time, self.time)?;
        if !(constants::OTLP_EXPONENTIAL_SCALE_MIN..=constants::OTLP_EXPONENTIAL_SCALE_MAX)
            .contains(&self.scale)
        {
            return Err(invalid("scale", "exponential scale out of range"));
        }
        if self.zero_threshold.get().is_nan() || self.zero_threshold.get() < 0.0 {
            return Err(invalid("zero_threshold", "threshold must be nonnegative"));
        }
        if total(
            self.positive
                .bucket_counts
                .iter()
                .chain(&self.negative.bucket_counts)
                .copied()
                .chain([self.zero_count]),
        )? != self.count
        {
            return Err(invalid("count", "bucket sum differs from count"));
        }
        Ok(())
    }
}
impl ValueAtQuantile {
    /// Checks the quantile domain.
    /// # Errors
    /// Rejects values outside [0, 1], including NaN.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        if (0.0..=1.0).contains(&self.quantile.get()) {
            Ok(())
        } else {
            Err(invalid("quantile", "quantile outside [0,1]"))
        }
    }
}
impl SummaryDataPoint {
    /// Checks the interval and strictly increasing quantiles.
    /// # Errors
    /// Rejects invalid, repeated or descending quantiles.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        interval(self.start_time, self.time)?;
        for q in &self.quantile_values {
            q.validate()?;
        }
        if self
            .quantile_values
            .windows(2)
            .any(|w| w[0].quantile.get() >= w[1].quantile.get())
        {
            return Err(invalid(
                "quantile_values",
                "quantiles must strictly increase",
            ));
        }
        Ok(())
    }
}
