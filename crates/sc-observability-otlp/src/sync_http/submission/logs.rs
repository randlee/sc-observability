//! OTLP/JSON log submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{InstrumentationScope, LogPoint, Resource, ResourceRecord},
    submission::SubmissionEnvelope,
};
use serde_json::{Map, Value};

struct ScopeLogs {
    scope: InstrumentationScope,
    records: Vec<Value>,
}

struct ResourceLogs {
    resource: Resource,
    scopes: Vec<ScopeLogs>,
}

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Value {
    let mut resources: Vec<ResourceLogs> = Vec::new();
    for envelope in envelopes {
        for record in &envelope.logs {
            append(&mut resources, record);
        }
    }
    Value::Object(Map::from_iter([(
        "resourceLogs".to_owned(),
        Value::Array(
            resources
                .into_iter()
                .map(|resource_logs| {
                    Value::Object(Map::from_iter([
                        (
                            "resource".to_owned(),
                            Value::Object(resource::resource(&resource_logs.resource)),
                        ),
                        (
                            "scopeLogs".to_owned(),
                            Value::Array(
                                resource_logs
                                    .scopes
                                    .into_iter()
                                    .map(|scope_logs| {
                                        Value::Object(Map::from_iter([
                                            (
                                                "scope".to_owned(),
                                                Value::Object(resource::scope(&scope_logs.scope)),
                                            ),
                                            (
                                                "logRecords".to_owned(),
                                                Value::Array(scope_logs.records),
                                            ),
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

fn append(groups: &mut Vec<ResourceLogs>, record: &ResourceRecord<LogPoint>) {
    let scope = resource::resource_scope_group(
        groups,
        &record.resource,
        &record.scope,
        |group| &group.resource,
        |group| &mut group.scopes,
        |group| &group.scope,
        |resource| ResourceLogs {
            resource,
            scopes: Vec::new(),
        },
        |scope| ScopeLogs {
            scope,
            records: Vec::new(),
        },
    );
    scope.records.push(log_record(&record.record));
}

fn log_record(record: &LogPoint) -> Value {
    let mut encoded = Map::new();
    resource::insert_timestamp(&mut encoded, "timeUnixNano", record.time.as_ref());
    encoded.insert(
        "observedTimeUnixNano".to_owned(),
        resource::timestamp(&record.observed_time),
    );
    encoded.insert(
        "severityNumber".to_owned(),
        Value::from(record.severity_number.get()),
    );
    resource::insert_string(&mut encoded, "severityText", record.severity_text.as_ref());
    resource::insert_string(&mut encoded, "eventName", record.event_name.as_ref());
    if let Some(body) = &record.body {
        encoded.insert("body".to_owned(), values::any_value(body));
    }
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(values::key_values(&record.attributes)),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(record.dropped_attributes_count),
    );
    encoded.insert("flags".to_owned(), Value::from(record.flags));
    if let Some(trace_id) = &record.trace_id {
        encoded.insert("traceId".to_owned(), Value::String(trace_id.to_string()));
    }
    if let Some(span_id) = &record.span_id {
        encoded.insert("spanId".to_owned(), Value::String(span_id.to_string()));
    }
    Value::Object(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_log_fixture_with_proto_json_timestamps_and_ids() {
        let envelope: SubmissionEnvelope = serde_json::from_str(&super::super::golden_fixture(
            "logs",
            "expected.envelope.json",
        ))
        .expect("canonical log fixture parses");
        let value = request(&[envelope]);
        let record = &value["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert!(record["observedTimeUnixNano"].is_string());
        assert!(record["severityNumber"].is_number());
    }
}
