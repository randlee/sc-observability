//! Legacy OTLP record conversion and JSON wire serialization.
//!
//! Kept separate from transport, worker lifecycle, and retry policy. The
//! transplanted encoding behavior is unchanged by this module boundary.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::contracts::{CompleteSpan, ExportRecord, LogRecord, Resource};
use sc_observability_types::otlp::{
    OtlpLogRecord, OtlpResource, group_records_by_resource_and_scope,
};
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, MetricRecord, MetricValue, SpanKind, SpanStatus,
    TraceFlags,
};
use sc_observability_types::{LogEvent, Timestamp};

pub(super) fn log_record(event: &LogEvent) -> ExportRecord<LogRecord> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(event.service.as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: OtlpLogRecord {
            event: event.clone(),
            trace_flags: event
                .trace
                .as_ref()
                .map_or(TraceFlags::default(), |_| TraceFlags::default()),
            attributes: BTreeMap::new(),
        },
    }
}

pub(super) fn span_record(span: &CompleteSpan) -> ExportRecord<CompleteSpan> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(span.record.service().as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: span.clone(),
    }
}

pub(super) fn metric_record(metric: &MetricRecord) -> ExportRecord<MetricRecord> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(metric.service().as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: metric.clone(),
    }
}

pub(super) fn build_logs_payload(records: &[ExportRecord<LogRecord>]) -> Value {
    let resource_logs = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeLogs": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "logRecords": scope.records.into_iter().map(log_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceLogs": resource_logs })
}

pub(super) fn build_traces_payload(records: &[ExportRecord<CompleteSpan>]) -> Value {
    let resource_spans = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeSpans": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "spans": scope.records.into_iter().map(span_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceSpans": resource_spans })
}

pub(super) fn build_metrics_payload(records: &[ExportRecord<MetricRecord>]) -> Value {
    let resource_metrics = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeMetrics": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "metrics": scope.records.iter().map(metric_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceMetrics": resource_metrics })
}

fn resource_json(resource: &OtlpResource) -> Value {
    json!({
        "attributes": resource.attributes.iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "schemaUrl": resource.schema_url,
    })
}

fn scope_json(scope: &crate::contracts::InstrumentationScope) -> Value {
    json!({
        "name": scope.name,
        "version": scope.version,
        "attributes": scope.attributes.iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "schemaUrl": scope.schema_url,
    })
}

fn log_json(record: LogRecord) -> Value {
    let event = record.event;
    let mut attributes = record
        .attributes
        .iter()
        .map(|(key, value)| json!({ "key": key, "value": attribute_json(value) }))
        .collect::<Vec<_>>();
    attributes.push(json!({
        "key": "log.target",
        "value": { "stringValue": event.target.as_str() },
    }));
    attributes.push(json!({
        "key": "event.name",
        "value": { "stringValue": event.action.as_str() },
    }));
    if let Some(correlation_id) = event.correlation_id {
        attributes.push(json!({
            "key": "sc.observability.log.correlation_id",
            "value": { "stringValue": correlation_id.as_str() },
        }));
    }
    for (key, value) in event.fields {
        attributes.push(json!({ "key": key, "value": json_value_to_otlp_any(&value) }));
    }
    if let Some(trace) = event.trace {
        attributes.push(json!({
            "key": "trace_id",
            "value": { "stringValue": trace.trace_id.as_str() },
        }));
        attributes.push(json!({
            "key": "span_id",
            "value": { "stringValue": trace.span_id.as_str() },
        }));
    }
    let (severity_number, severity_text) = severity_fields(event.level);
    json!({
        "timeUnixNano": timestamp_nanos(event.timestamp),
        "body": { "stringValue": event.message.unwrap_or_else(|| event.action.as_str().to_owned()) },
        "severityNumber": severity_number,
        "severityText": severity_text,
        "attributes": attributes,
    })
}

fn span_json(span: CompleteSpan) -> Value {
    let record = span.record;
    let trace = record.trace();
    let mut result = json!({
        "traceId": trace.trace_id.as_str(),
        "spanId": trace.span_id.as_str(),
        "name": record.name().as_str(),
        "kind": span_kind_number(record.kind()),
        "startTimeUnixNano": timestamp_nanos(record.timestamp()),
        "endTimeUnixNano": timestamp_nanos_with_duration(record.timestamp(), record.duration_ms().as_u64()),
        "status": { "code": span_status_number(record.status()) },
        "attributes": record.attributes().iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "events": span.events.into_iter().map(|event| json!({
            "timeUnixNano": timestamp_nanos(event.timestamp),
            "name": event.name.as_str(),
            "attributes": event.attributes.iter().map(|(key, value)| json!({
                "key": key,
                "value": attribute_json(value),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    if let Some(parent) = &trace.parent_span_id {
        result["parentSpanId"] = json!(parent.as_str());
    }
    result["flags"] = json!(u32::from(trace.flags.bits()));
    result["links"] = record
        .links()
        .iter()
        .map(|link| {
            json!({
                "traceId": link.trace_id.as_str(),
                "spanId": link.span_id.as_str(),
                "flags": u32::from(link.flags.bits()),
                "attributes": link.attributes.iter().map(|(key, value)| json!({
                    "key": key,
                    "value": attribute_json(value),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    result
}

fn metric_json(metric: &MetricRecord) -> Value {
    let attrs = metric
        .attributes()
        .iter()
        .map(|(key, value)| json!({ "key": key, "value": attribute_json(value) }))
        .collect::<Vec<_>>();
    let point_time = timestamp_nanos(metric.timestamp());
    let data_point =
        |value: f64| json!({ "attributes": attrs, "timeUnixNano": point_time, "asDouble": value });
    let mut result = json!({
        "name": metric.name().as_str(),
        "unit": metric.unit().map(ToString::to_string).unwrap_or_default(),
    });
    match metric.value() {
        MetricValue::Gauge(value) => {
            result["gauge"] = json!({ "dataPoints": [data_point(value.get())] });
        }
        MetricValue::Sum {
            value,
            monotonic,
            temporality,
            start_time,
        } => {
            let mut point = data_point(value.get());
            point["startTimeUnixNano"] = json!(timestamp_nanos(*start_time));
            result["sum"] = json!({
                "aggregationTemporality": temporality_number(*temporality),
                "isMonotonic": monotonic,
                "dataPoints": [point],
            });
        }
        MetricValue::Histogram {
            point,
            temporality,
            start_time,
        } => {
            result["histogram"] = json!({
                "aggregationTemporality": temporality_number(*temporality),
                "dataPoints": [{
                    "attributes": attrs,
                    "startTimeUnixNano": timestamp_nanos(*start_time),
                    "timeUnixNano": point_time,
                    "count": point.count(),
                    "sum": point.sum().get(),
                    "bucketCounts": point.bucket_counts(),
                    "explicitBounds": point.explicit_bounds().iter().map(|bound| bound.get()).collect::<Vec<_>>(),
                }],
            });
        }
        _ => {}
    }
    result
}

fn attribute_json(value: &AttributeValue) -> Value {
    match value {
        AttributeValue::Bool(value) => json!({ "boolValue": value }),
        AttributeValue::Int(value) => json!({ "intValue": value.to_string() }),
        AttributeValue::UInt(value) => json!({ "intValue": value.to_string() }),
        AttributeValue::Float(value) => json!({ "doubleValue": value.get() }),
        AttributeValue::String(value) => json!({ "stringValue": value }),
        AttributeValue::Null => json!({ "stringValue": "null" }),
        AttributeValue::Array(values) => json!({
            "arrayValue": { "values": values.iter().map(attribute_json).collect::<Vec<_>>() }
        }),
        AttributeValue::Object(values) => json!({
            "kvlistValue": { "values": values.iter().map(|(key, value)| json!({
                "key": key,
                "value": attribute_json(value),
            })).collect::<Vec<_>>() }
        }),
        _ => json!({ "stringValue": "unsupported" }),
    }
}

fn json_value_to_otlp_any(value: &Value) -> Value {
    match value {
        Value::Null => json!({ "stringValue": "null" }),
        Value::Bool(value) => json!({ "boolValue": value }),
        Value::Number(value) if value.is_i64() => {
            json!({ "intValue": value.as_i64().unwrap_or_default().to_string() })
        }
        Value::Number(value) if value.is_u64() => {
            json!({ "intValue": value.as_u64().unwrap_or_default().to_string() })
        }
        Value::Number(value) => json!({ "doubleValue": value.as_f64().unwrap_or_default() }),
        Value::String(value) => json!({ "stringValue": value }),
        Value::Array(values) => json!({
            "arrayValue": { "values": values.iter().map(json_value_to_otlp_any).collect::<Vec<_>>() }
        }),
        Value::Object(values) => json!({
            "kvlistValue": { "values": values.iter().map(|(key, value)| json!({
                "key": key,
                "value": json_value_to_otlp_any(value),
            })).collect::<Vec<_>>() }
        }),
    }
}

fn timestamp_nanos(timestamp: Timestamp) -> String {
    timestamp.into_inner().unix_timestamp_nanos().to_string()
}

fn timestamp_nanos_with_duration(timestamp: Timestamp, duration_ms: u64) -> String {
    (timestamp.into_inner().unix_timestamp_nanos() + i128::from(duration_ms) * 1_000_000)
        .to_string()
}

fn severity_fields(level: sc_observability_types::Level) -> (u32, &'static str) {
    match level {
        sc_observability_types::Level::Trace => (1, "TRACE"),
        sc_observability_types::Level::Debug => (5, "DEBUG"),
        sc_observability_types::Level::Info => (9, "INFO"),
        sc_observability_types::Level::Warn => (13, "WARN"),
        sc_observability_types::Level::Error => (17, "ERROR"),
    }
}

fn span_kind_number(kind: SpanKind) -> u8 {
    match kind {
        SpanKind::Internal => 1,
        SpanKind::Server => 2,
        SpanKind::Client => 3,
        SpanKind::Producer => 4,
        SpanKind::Consumer => 5,
        _ => 0,
    }
}

fn span_status_number(status: SpanStatus) -> &'static str {
    match status {
        SpanStatus::Unset => "STATUS_CODE_UNSET",
        SpanStatus::Ok => "STATUS_CODE_OK",
        SpanStatus::Error => "STATUS_CODE_ERROR",
    }
}

fn temporality_number(temporality: AggregationTemporality) -> u8 {
    match temporality {
        AggregationTemporality::Delta => 1,
        AggregationTemporality::Cumulative => 2,
        _ => 0,
    }
}
