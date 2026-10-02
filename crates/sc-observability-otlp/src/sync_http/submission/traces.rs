//! OTLP/JSON trace submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{SpanEventPoint, SpanKindPoint, SpanLinkPoint, SpanPoint, StatusCode},
    submission::SubmissionEnvelope,
};
use sc_observability_types::v2::ExportError;
use serde_json::{Map, Value};

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Result<Value, ExportError> {
    let resources = resource::group_by_resource_scope(
        envelopes.iter().flat_map(|envelope| &envelope.spans),
        span,
        "scopeSpans",
        "spans",
    )?;
    Ok(Value::Object(Map::from_iter([(
        "resourceSpans".to_owned(),
        Value::Array(resources),
    )])))
}

fn span(value: &SpanPoint) -> Result<Value, ExportError> {
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
    encoded.insert("kind".to_owned(), Value::from(kind(value.kind)?));
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
        Value::Array(values::key_values(&value.attributes)?),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(value.dropped_attributes_count),
    );
    encoded.insert(
        "events".to_owned(),
        Value::Array(
            value
                .events
                .iter()
                .map(event)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    encoded.insert(
        "droppedEventsCount".to_owned(),
        Value::from(value.dropped_events_count),
    );
    encoded.insert(
        "links".to_owned(),
        Value::Array(
            value
                .links
                .iter()
                .map(link)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    encoded.insert(
        "droppedLinksCount".to_owned(),
        Value::from(value.dropped_links_count),
    );
    encoded.insert("status".to_owned(), status(value)?);
    Ok(Value::Object(encoded))
}

fn event(value: &SpanEventPoint) -> Result<Value, ExportError> {
    Ok(Value::Object(Map::from_iter([
        ("timeUnixNano".to_owned(), resource::timestamp(&value.time)),
        ("name".to_owned(), Value::String(value.name.clone())),
        (
            "attributes".to_owned(),
            Value::Array(values::key_values(&value.attributes)?),
        ),
        (
            "droppedAttributesCount".to_owned(),
            Value::from(value.dropped_attributes_count),
        ),
    ])))
}

fn link(value: &SpanLinkPoint) -> Result<Value, ExportError> {
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
            Value::Array(values::key_values(&value.attributes)?),
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
    Ok(Value::Object(encoded))
}

fn status(value: &SpanPoint) -> Result<Value, ExportError> {
    let mut encoded = Map::from_iter([(
        "code".to_owned(),
        Value::from(status_code(value.status.code)?),
    )]);
    resource::insert_string(&mut encoded, "message", value.status.message.as_ref());
    Ok(Value::Object(encoded))
}

fn kind(value: SpanKindPoint) -> Result<u8, ExportError> {
    Ok(match value {
        SpanKindPoint::Unspecified => 0,
        SpanKindPoint::Internal => 1,
        SpanKindPoint::Server => 2,
        SpanKindPoint::Client => 3,
        SpanKindPoint::Producer => 4,
        SpanKindPoint::Consumer => 5,
        _ => return Err(values::unsupported_variant("SpanKindPoint")),
    })
}

fn status_code(value: StatusCode) -> Result<u8, ExportError> {
    Ok(match value {
        StatusCode::Unset => 0,
        StatusCode::Ok => 1,
        StatusCode::Error => 2,
        _ => return Err(values::unsupported_variant("StatusCode")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_trace_fixture_with_events_links_and_status() {
        let envelope: SubmissionEnvelope = serde_json::from_str(&super::super::golden_fixture(
            "traces",
            "expected.envelope.json",
        ))
        .expect("canonical trace fixture parses");
        let value = request(&[envelope]).expect("fixture encodes");
        let span = &value["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert!(span["startTimeUnixNano"].is_string());
        assert!(span["status"]["code"].is_number());
        assert!(span["events"].is_array());
        assert!(span["links"].is_array());
    }
}
