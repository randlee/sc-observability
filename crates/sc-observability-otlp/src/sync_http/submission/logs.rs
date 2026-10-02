//! OTLP/JSON log submission request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{signals::LogPoint, submission::SubmissionEnvelope};
use sc_observability_types::v2::ExportError;
use serde_json::{Map, Value};

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Result<Value, ExportError> {
    let resources = resource::group_by_resource_scope(
        envelopes.iter().flat_map(|envelope| &envelope.logs),
        log_record,
        "scopeLogs",
        "logRecords",
    )?;
    Ok(Value::Object(Map::from_iter([(
        "resourceLogs".to_owned(),
        Value::Array(resources),
    )])))
}

fn log_record(record: &LogPoint) -> Result<Value, ExportError> {
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
        encoded.insert("body".to_owned(), values::any_value(body)?);
    }
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(values::key_values(&record.attributes)?),
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
    Ok(Value::Object(encoded))
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
        let value = request(&[envelope]).expect("fixture encodes");
        let record = &value["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert!(record["observedTimeUnixNano"].is_string());
        assert!(record["severityNumber"].is_number());
    }
}
