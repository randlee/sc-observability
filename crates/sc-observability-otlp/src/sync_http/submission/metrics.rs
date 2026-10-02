//! OTLP/JSON metric submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{
        AggregationTemporality, Exemplar, MetricData, MetricStream, NumberPoint, NumberValue,
    },
    submission::SubmissionEnvelope,
};
use sc_observability_types::v2::ExportError;
use serde_json::{Map, Value};

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Result<Value, ExportError> {
    let resources = resource::group_by_resource_scope(
        envelopes.iter().flat_map(|envelope| &envelope.metrics),
        metric,
        "scopeMetrics",
        "metrics",
    )?;
    Ok(Value::Object(Map::from_iter([(
        "resourceMetrics".to_owned(),
        Value::Array(resources),
    )])))
}

fn metric(value: &MetricStream) -> Result<Value, ExportError> {
    let mut encoded = Map::new();
    encoded.insert(
        "name".to_owned(),
        Value::String(value.name.as_str().to_owned()),
    );
    resource::insert_string(&mut encoded, "description", value.description.as_ref());
    resource::insert_string(&mut encoded, "unit", value.unit.as_ref());
    if !value.metadata.entries().is_empty() {
        encoded.insert(
            "metadata".to_owned(),
            Value::Array(values::key_values(&value.metadata)?),
        );
    }
    let (key, data) = metric_data(&value.data)?;
    encoded.insert(key.to_owned(), data);
    Ok(Value::Object(encoded))
}

fn metric_data(value: &MetricData) -> Result<(&'static str, Value), ExportError> {
    match value {
        MetricData::Gauge { points } => Ok(("gauge", points_value(points)?)),
        MetricData::Sum {
            points,
            temporality,
            monotonic,
        } => Ok((
            "sum",
            Value::Object(Map::from_iter([
                (
                    "aggregationTemporality".to_owned(),
                    Value::from(temporality_value(*temporality)?),
                ),
                ("isMonotonic".to_owned(), Value::Bool(*monotonic)),
                (
                    "dataPoints".to_owned(),
                    Value::Array(
                        points
                            .iter()
                            .map(number_point)
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ),
            ])),
        )),
        MetricData::Histogram {
            points,
            temporality,
        } => Ok((
            "histogram",
            Value::Object(Map::from_iter([
                (
                    "aggregationTemporality".to_owned(),
                    Value::from(temporality_value(*temporality)?),
                ),
                (
                    "dataPoints".to_owned(),
                    Value::Array(
                        points
                            .iter()
                            .map(histogram_point)
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ),
            ])),
        )),
        MetricData::ExponentialHistogram {
            points,
            temporality,
        } => Ok((
            "exponentialHistogram",
            Value::Object(Map::from_iter([
                (
                    "aggregationTemporality".to_owned(),
                    Value::from(temporality_value(*temporality)?),
                ),
                (
                    "dataPoints".to_owned(),
                    Value::Array(
                        points
                            .iter()
                            .map(exponential_point)
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ),
            ])),
        )),
        MetricData::Summary { points } => Ok((
            "summary",
            Value::Object(Map::from_iter([(
                "dataPoints".to_owned(),
                Value::Array(
                    points
                        .iter()
                        .map(summary_point)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            )])),
        )),
        _ => Err(unsupported_metric_data()),
    }
}

fn points_value(points: &[NumberPoint]) -> Result<Value, ExportError> {
    Ok(Value::Object(Map::from_iter([(
        "dataPoints".to_owned(),
        Value::Array(
            points
                .iter()
                .map(number_point)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    )])))
}

fn number_point(value: &NumberPoint) -> Result<Value, ExportError> {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    )?;
    number_value(&mut encoded, &value.value)?;
    encoded.insert(
        "exemplars".to_owned(),
        Value::Array(
            value
                .exemplars
                .iter()
                .map(exemplar)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    Ok(Value::Object(encoded))
}

fn histogram_point(
    value: &sc_observability_types::otlp::signals::HistogramDataPoint,
) -> Result<Value, ExportError> {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    )?;
    encoded.insert("count".to_owned(), values::uint64(value.count));
    insert_double(&mut encoded, "sum", value.sum);
    encoded.insert(
        "bucketCounts".to_owned(),
        Value::Array(
            value
                .bucket_counts
                .iter()
                .copied()
                .map(values::uint64)
                .collect(),
        ),
    );
    encoded.insert(
        "explicitBounds".to_owned(),
        Value::Array(
            value
                .explicit_bounds
                .iter()
                .copied()
                .map(values::double)
                .collect(),
        ),
    );
    encoded.insert(
        "exemplars".to_owned(),
        Value::Array(
            value
                .exemplars
                .iter()
                .map(exemplar)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    insert_double(&mut encoded, "min", value.min);
    insert_double(&mut encoded, "max", value.max);
    Ok(Value::Object(encoded))
}

fn exponential_point(
    value: &sc_observability_types::otlp::signals::ExponentialHistogramDataPoint,
) -> Result<Value, ExportError> {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    )?;
    encoded.insert("count".to_owned(), values::uint64(value.count));
    insert_double(&mut encoded, "sum", value.sum);
    encoded.insert("scale".to_owned(), Value::from(value.scale));
    encoded.insert("zeroCount".to_owned(), values::uint64(value.zero_count));
    encoded.insert(
        "zeroThreshold".to_owned(),
        values::double(value.zero_threshold),
    );
    encoded.insert("positive".to_owned(), buckets(&value.positive));
    encoded.insert("negative".to_owned(), buckets(&value.negative));
    encoded.insert(
        "exemplars".to_owned(),
        Value::Array(
            value
                .exemplars
                .iter()
                .map(exemplar)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    insert_double(&mut encoded, "min", value.min);
    insert_double(&mut encoded, "max", value.max);
    Ok(Value::Object(encoded))
}

fn summary_point(
    value: &sc_observability_types::otlp::signals::SummaryDataPoint,
) -> Result<Value, ExportError> {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    )?;
    encoded.insert("count".to_owned(), values::uint64(value.count));
    encoded.insert("sum".to_owned(), values::double(value.sum));
    encoded.insert(
        "quantileValues".to_owned(),
        Value::Array(
            value
                .quantile_values
                .iter()
                .map(|quantile| {
                    Value::Object(Map::from_iter([
                        ("quantile".to_owned(), values::double(quantile.quantile)),
                        ("value".to_owned(), values::double(quantile.value)),
                    ]))
                })
                .collect(),
        ),
    );
    Ok(Value::Object(encoded))
}

fn common_point(
    attributes: &sc_observability_types::otlp::signals::KeyValues,
    start: Option<&sc_observability_types::Timestamp>,
    time: &sc_observability_types::Timestamp,
    flags: u32,
) -> Result<Map<String, Value>, ExportError> {
    let mut encoded = Map::from_iter([
        (
            "attributes".to_owned(),
            Value::Array(values::key_values(attributes)?),
        ),
        ("timeUnixNano".to_owned(), resource::timestamp(time)),
        ("flags".to_owned(), Value::from(flags)),
    ]);
    resource::insert_timestamp(&mut encoded, "startTimeUnixNano", start);
    Ok(encoded)
}

fn exemplar(value: &Exemplar) -> Result<Value, ExportError> {
    let mut encoded = Map::from_iter([
        (
            "filteredAttributes".to_owned(),
            Value::Array(values::key_values(&value.filtered_attributes)?),
        ),
        ("timeUnixNano".to_owned(), resource::timestamp(&value.time)),
    ]);
    number_value(&mut encoded, &value.value)?;
    if let Some(trace_id) = &value.trace_id {
        encoded.insert("traceId".to_owned(), Value::String(trace_id.to_string()));
    }
    if let Some(span_id) = &value.span_id {
        encoded.insert("spanId".to_owned(), Value::String(span_id.to_string()));
    }
    Ok(Value::Object(encoded))
}

fn number_value(encoded: &mut Map<String, Value>, value: &NumberValue) -> Result<(), ExportError> {
    match value {
        NumberValue::Int(value) => {
            encoded.insert("asInt".to_owned(), values::int64(*value));
        }
        NumberValue::Double(value) => {
            encoded.insert("asDouble".to_owned(), values::double(*value));
        }
        _ => return Err(unsupported_number_value()),
    }
    Ok(())
}

fn insert_double(
    encoded: &mut Map<String, Value>,
    name: &str,
    value: Option<sc_observability_types::otlp::signals::OtlpDouble>,
) {
    if let Some(value) = value {
        encoded.insert(name.to_owned(), values::double(value));
    }
}

fn buckets(value: &sc_observability_types::otlp::signals::ExponentialBuckets) -> Value {
    Value::Object(Map::from_iter([
        ("offset".to_owned(), Value::from(value.offset)),
        (
            "bucketCounts".to_owned(),
            Value::Array(
                value
                    .bucket_counts
                    .iter()
                    .copied()
                    .map(values::uint64)
                    .collect(),
            ),
        ),
    ]))
}

fn temporality_value(value: AggregationTemporality) -> Result<u8, ExportError> {
    match value {
        AggregationTemporality::Delta => Ok(1),
        AggregationTemporality::Cumulative => Ok(2),
        _ => Err(unsupported_aggregation_temporality()),
    }
}

// These helpers are the exact wildcard-arm seams. Tests cannot construct a
// future variant of a non-exhaustive neutral enum, so they exercise the same
// coded failure each wildcard arm selects without adding a fake public value.
fn unsupported_metric_data() -> ExportError {
    values::unsupported_variant("MetricData")
}

fn unsupported_number_value() -> ExportError {
    values::unsupported_variant("NumberValue")
}

fn unsupported_aggregation_temporality() -> ExportError {
    values::unsupported_variant("AggregationTemporality")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::submission::SubmissionExportFailure;
    use crate::error_codes::OTLP_EXPORT_TERMINAL;

    fn assert_terminal(error: ExportError) {
        assert_eq!(error.code(), OTLP_EXPORT_TERMINAL);
        assert!(matches!(
            super::super::classify(error),
            SubmissionExportFailure::Terminal(ExportError::TerminalExportFailure { .. })
        ));
    }

    #[test]
    fn unsupported_metric_data_is_coded_terminal() {
        assert_terminal(unsupported_metric_data());
    }

    #[test]
    fn unsupported_number_value_is_coded_terminal() {
        assert_terminal(unsupported_number_value());
    }

    #[test]
    fn unsupported_aggregation_temporality_is_coded_terminal() {
        assert_terminal(unsupported_aggregation_temporality());
    }

    #[test]
    fn encodes_each_metric_fixture_variant() {
        for fixture in [
            "metric_gauge",
            "metric_sum",
            "metric_histogram",
            "metric_exponential_histogram",
            "metric_summary",
        ] {
            let contents = super::super::golden_fixture(fixture, "expected.envelope.json");
            let envelope: SubmissionEnvelope =
                serde_json::from_str(&contents).expect("fixture parses");
            let value = request(&[envelope]).expect("fixture encodes");
            assert!(value["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0].is_object());
        }
    }
}
