//! OTLP/JSON trace submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{
        InstrumentationScope, Resource, ResourceRecord, SpanEventPoint, SpanKindPoint,
        SpanLinkPoint, SpanPoint, StatusCode,
    },
    submission::SubmissionEnvelope,
};
use serde_json::{Map, Value};

struct ScopeSpans {
    scope: InstrumentationScope,
    records: Vec<Value>,
}

struct ResourceSpans {
    resource: Resource,
    scopes: Vec<ScopeSpans>,
}

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Value {
    let mut resources = Vec::new();
    for envelope in envelopes {
        for record in &envelope.spans {
            append(&mut resources, record);
        }
    }
    Value::Object(Map::from_iter([(
        "resourceSpans".to_owned(),
        Value::Array(
            resources
                .into_iter()
                .map(|resource_spans| {
                    Value::Object(Map::from_iter([
                        (
                            "resource".to_owned(),
                            resource::resource(&resource_spans.resource),
                        ),
                        (
                            "scopeSpans".to_owned(),
                            Value::Array(
                                resource_spans
                                    .scopes
                                    .into_iter()
                                    .map(|scope_spans| {
                                        Value::Object(Map::from_iter([
                                            (
                                                "scope".to_owned(),
                                                resource::scope(&scope_spans.scope),
                                            ),
                                            ("spans".to_owned(), Value::Array(scope_spans.records)),
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

fn append(groups: &mut Vec<ResourceSpans>, record: &ResourceRecord<SpanPoint>) {
    let resource = if let Some(group) = groups
        .iter_mut()
        .find(|group| group.resource == record.resource)
    {
        group
    } else {
        groups.push(ResourceSpans {
            resource: record.resource.clone(),
            scopes: Vec::new(),
        });
        groups.last_mut().expect("pushed resource group")
    };
    let scope = if let Some(group) = resource
        .scopes
        .iter_mut()
        .find(|group| group.scope == record.scope)
    {
        group
    } else {
        resource.scopes.push(ScopeSpans {
            scope: record.scope.clone(),
            records: Vec::new(),
        });
        resource.scopes.last_mut().expect("pushed scope group")
    };
    scope.records.push(span(&record.record));
}

fn span(value: &SpanPoint) -> Value {
    let mut encoded = Map::new();
    encoded.insert(
        "traceId".to_owned(),
        Value::String(value.trace_id.to_string()),
    );
    encoded.insert(
        "spanId".to_owned(),
        Value::String(value.span_id.to_string()),
    );
    if let Some(trace_state) = &value.trace_state {
        encoded.insert(
            "traceState".to_owned(),
            Value::String(trace_state.as_str().to_owned()),
        );
    }
    if let Some(parent_span_id) = &value.parent_span_id {
        encoded.insert(
            "parentSpanId".to_owned(),
            Value::String(parent_span_id.to_string()),
        );
    }
    encoded.insert("flags".to_owned(), Value::from(value.flags));
    encoded.insert("name".to_owned(), Value::String(value.name.clone()));
    encoded.insert("kind".to_owned(), Value::from(kind(value.kind)));
    encoded.insert(
        "startTimeUnixNano".to_owned(),
        resource::timestamp(&value.start_time),
    );
    encoded.insert(
        "endTimeUnixNano".to_owned(),
        resource::timestamp(&value.end_time),
    );
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(values::key_values(&value.attributes)),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(value.dropped_attributes_count),
    );
    encoded.insert(
        "events".to_owned(),
        Value::Array(value.events.iter().map(event).collect()),
    );
    encoded.insert(
        "droppedEventsCount".to_owned(),
        Value::from(value.dropped_events_count),
    );
    encoded.insert(
        "links".to_owned(),
        Value::Array(value.links.iter().map(link).collect()),
    );
    encoded.insert(
        "droppedLinksCount".to_owned(),
        Value::from(value.dropped_links_count),
    );
    encoded.insert("status".to_owned(), status(value));
    Value::Object(encoded)
}

fn event(value: &SpanEventPoint) -> Value {
    Value::Object(Map::from_iter([
        ("timeUnixNano".to_owned(), resource::timestamp(&value.time)),
        ("name".to_owned(), Value::String(value.name.clone())),
        (
            "attributes".to_owned(),
            Value::Array(values::key_values(&value.attributes)),
        ),
        (
            "droppedAttributesCount".to_owned(),
            Value::from(value.dropped_attributes_count),
        ),
    ]))
}

fn link(value: &SpanLinkPoint) -> Value {
    let mut encoded = Map::from_iter([
        (
            "traceId".to_owned(),
            Value::String(value.trace_id.to_string()),
        ),
        (
            "spanId".to_owned(),
            Value::String(value.span_id.to_string()),
        ),
        (
            "attributes".to_owned(),
            Value::Array(values::key_values(&value.attributes)),
        ),
        (
            "droppedAttributesCount".to_owned(),
            Value::from(value.dropped_attributes_count),
        ),
        ("flags".to_owned(), Value::from(value.flags)),
    ]);
    if let Some(trace_state) = &value.trace_state {
        encoded.insert(
            "traceState".to_owned(),
            Value::String(trace_state.as_str().to_owned()),
        );
    }
    Value::Object(encoded)
}

fn status(value: &SpanPoint) -> Value {
    let mut encoded = Map::from_iter([(
        "code".to_owned(),
        Value::from(status_code(value.status.code)),
    )]);
    resource::insert_string(&mut encoded, "message", value.status.message.as_ref());
    Value::Object(encoded)
}

fn kind(value: SpanKindPoint) -> u8 {
    match value {
        SpanKindPoint::Internal => 1,
        SpanKindPoint::Server => 2,
        SpanKindPoint::Client => 3,
        SpanKindPoint::Producer => 4,
        SpanKindPoint::Consumer => 5,
        _ => 0,
    }
}

fn status_code(value: StatusCode) -> u8 {
    match value {
        StatusCode::Ok => 1,
        StatusCode::Error => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_trace_fixture_with_events_links_and_status() {
        let envelope: SubmissionEnvelope = serde_json::from_str(include_str!(
            "../../../../sc-observability-types/tests/fixtures/otlp_submission/golden/traces/expected.envelope.json"
        ))
        .expect("canonical trace fixture parses");
        let value = request(&[envelope]);
        let span = &value["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert!(span["startTimeUnixNano"].is_string());
        assert!(span["status"]["code"].is_number());
        assert!(span["events"].is_array());
        assert!(span["links"].is_array());
    }
}
