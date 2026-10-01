//! OTLP/JSON metric submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{
        AggregationTemporality, Exemplar, InstrumentationScope, MetricData, MetricStream,
        NumberPoint, NumberValue, Resource, ResourceRecord,
    },
    submission::SubmissionEnvelope,
};
use serde_json::{Map, Value};

struct ScopeMetrics {
    scope: InstrumentationScope,
    records: Vec<Value>,
}
struct ResourceMetrics {
    resource: Resource,
    scopes: Vec<ScopeMetrics>,
}

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Value {
    let mut resources = Vec::new();
    for envelope in envelopes {
        for record in &envelope.metrics {
            append(&mut resources, record);
        }
    }
    Value::Object(Map::from_iter([(
        "resourceMetrics".to_owned(),
        Value::Array(
            resources
                .into_iter()
                .map(|group| {
                    Value::Object(Map::from_iter([
                        (
                            "resource".to_owned(),
                            Value::Object(resource::resource(&group.resource)),
                        ),
                        (
                            "scopeMetrics".to_owned(),
                            Value::Array(
                                group
                                    .scopes
                                    .into_iter()
                                    .map(|scope| {
                                        Value::Object(Map::from_iter([
                                            (
                                                "scope".to_owned(),
                                                Value::Object(resource::scope(&scope.scope)),
                                            ),
                                            ("metrics".to_owned(), Value::Array(scope.records)),
                                        ]))
                                    })
                                    .collect(),
                            ),
                        ),
                    ]))
                })
                .collect(),
        ),
    )]))
}

fn append(groups: &mut Vec<ResourceMetrics>, record: &ResourceRecord<MetricStream>) {
    let resource_group = if let Some(group) = groups
        .iter_mut()
        .find(|group| group.resource == record.resource)
    {
        group
    } else {
        groups.push(ResourceMetrics {
            resource: record.resource.clone(),
            scopes: Vec::new(),
        });
        groups.last_mut().expect("pushed resource group")
    };
    let scope_group = if let Some(group) = resource_group
        .scopes
        .iter_mut()
        .find(|group| group.scope == record.scope)
    {
        group
    } else {
        resource_group.scopes.push(ScopeMetrics {
            scope: record.scope.clone(),
            records: Vec::new(),
        });
        resource_group
            .scopes
            .last_mut()
            .expect("pushed scope group")
    };
    scope_group.records.push(metric(&record.record));
}

fn metric(value: &MetricStream) -> Value {
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
            Value::Array(values::key_values(&value.metadata)),
        );
    }
    match &value.data {
        MetricData::Gauge { points } => {
            encoded.insert("gauge".to_owned(), points_value(points));
        }
        MetricData::Sum {
            points,
            temporality,
            monotonic,
        } => {
            encoded.insert(
                "sum".to_owned(),
                Value::Object(Map::from_iter([
                    (
                        "aggregationTemporality".to_owned(),
                        Value::from(temporality_value(*temporality)),
                    ),
                    ("isMonotonic".to_owned(), Value::Bool(*monotonic)),
                    (
                        "dataPoints".to_owned(),
                        Value::Array(points.iter().map(number_point).collect()),
                    ),
                ])),
            );
        }
        MetricData::Histogram {
            points,
            temporality,
        } => {
            encoded.insert(
                "histogram".to_owned(),
                Value::Object(Map::from_iter([
                    (
                        "aggregationTemporality".to_owned(),
                        Value::from(temporality_value(*temporality)),
                    ),
                    (
                        "dataPoints".to_owned(),
                        Value::Array(points.iter().map(histogram_point).collect()),
                    ),
                ])),
            );
        }
        MetricData::ExponentialHistogram {
            points,
            temporality,
        } => {
            encoded.insert(
                "exponentialHistogram".to_owned(),
                Value::Object(Map::from_iter([
                    (
                        "aggregationTemporality".to_owned(),
                        Value::from(temporality_value(*temporality)),
                    ),
                    (
                        "dataPoints".to_owned(),
                        Value::Array(points.iter().map(exponential_point).collect()),
                    ),
                ])),
            );
        }
        MetricData::Summary { points } => {
            encoded.insert(
                "summary".to_owned(),
                Value::Object(Map::from_iter([(
                    "dataPoints".to_owned(),
                    Value::Array(points.iter().map(summary_point).collect()),
                )])),
            );
        }
        _ => unreachable!("new MetricData variants require an explicit OTLP/JSON mapping"),
    }
    Value::Object(encoded)
}

fn points_value(points: &[NumberPoint]) -> Value {
    Value::Object(Map::from_iter([(
        "dataPoints".to_owned(),
        Value::Array(points.iter().map(number_point).collect()),
    )]))
}

fn number_point(value: &NumberPoint) -> Value {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    );
    number_value(&mut encoded, &value.value);
    encoded.insert(
        "exemplars".to_owned(),
        Value::Array(value.exemplars.iter().map(exemplar).collect()),
    );
    Value::Object(encoded)
}

fn histogram_point(value: &sc_observability_types::otlp::signals::HistogramDataPoint) -> Value {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    );
    encoded.insert("count".to_owned(), uint(value.count));
    insert_double(&mut encoded, "sum", value.sum);
    encoded.insert(
        "bucketCounts".to_owned(),
        Value::Array(value.bucket_counts.iter().copied().map(uint).collect()),
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
        Value::Array(value.exemplars.iter().map(exemplar).collect()),
    );
    insert_double(&mut encoded, "min", value.min);
    insert_double(&mut encoded, "max", value.max);
    Value::Object(encoded)
}

fn exponential_point(
    value: &sc_observability_types::otlp::signals::ExponentialHistogramDataPoint,
) -> Value {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    );
    encoded.insert("count".to_owned(), uint(value.count));
    insert_double(&mut encoded, "sum", value.sum);
    encoded.insert("scale".to_owned(), Value::from(value.scale));
    encoded.insert("zeroCount".to_owned(), uint(value.zero_count));
    encoded.insert(
        "zeroThreshold".to_owned(),
        values::double(value.zero_threshold),
    );
    encoded.insert("positive".to_owned(), buckets(&value.positive));
    encoded.insert("negative".to_owned(), buckets(&value.negative));
    encoded.insert(
        "exemplars".to_owned(),
        Value::Array(value.exemplars.iter().map(exemplar).collect()),
    );
    insert_double(&mut encoded, "min", value.min);
    insert_double(&mut encoded, "max", value.max);
    Value::Object(encoded)
}

fn summary_point(value: &sc_observability_types::otlp::signals::SummaryDataPoint) -> Value {
    let mut encoded = common_point(
        &value.attributes,
        value.start_time.as_ref(),
        &value.time,
        value.flags.bits(),
    );
    encoded.insert("count".to_owned(), uint(value.count));
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
    Value::Object(encoded)
}

fn common_point(
    attributes: &sc_observability_types::otlp::signals::KeyValues,
    start: Option<&sc_observability_types::Timestamp>,
    time: &sc_observability_types::Timestamp,
    flags: u32,
) -> Map<String, Value> {
    let mut encoded = Map::from_iter([
        (
            "attributes".to_owned(),
            Value::Array(values::key_values(attributes)),
        ),
        ("timeUnixNano".to_owned(), resource::timestamp(time)),
        ("flags".to_owned(), Value::from(flags)),
    ]);
    resource::insert_timestamp(&mut encoded, "startTimeUnixNano", start);
    encoded
}

fn exemplar(value: &Exemplar) -> Value {
    let mut encoded = Map::from_iter([
        (
            "filteredAttributes".to_owned(),
            Value::Array(values::key_values(&value.filtered_attributes)),
        ),
        ("timeUnixNano".to_owned(), resource::timestamp(&value.time)),
    ]);
    number_value(&mut encoded, &value.value);
    if let Some(trace_id) = &value.trace_id {
        encoded.insert("traceId".to_owned(), Value::String(trace_id.to_string()));
    }
    if let Some(span_id) = &value.span_id {
        encoded.insert("spanId".to_owned(), Value::String(span_id.to_string()));
    }
    Value::Object(encoded)
}

fn number_value(encoded: &mut Map<String, Value>, value: &NumberValue) {
    match value {
        NumberValue::Int(value) => {
            encoded.insert("asInt".to_owned(), values::int64(*value));
        }
        NumberValue::Double(value) => {
            encoded.insert("asDouble".to_owned(), values::double(*value));
        }
        _ => unreachable!("new NumberValue variants require an explicit OTLP/JSON mapping"),
    }
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
            Value::Array(value.bucket_counts.iter().copied().map(uint).collect()),
        ),
    ]))
}

fn temporality_value(value: AggregationTemporality) -> u8 {
    match value {
        AggregationTemporality::Delta => 1,
        AggregationTemporality::Cumulative => 2,
        _ => 0,
    }
}

fn uint(value: u64) -> Value {
    values::uint64(value)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let value = request(&[envelope]);
            assert!(value["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0].is_object());
        }
    }
}
