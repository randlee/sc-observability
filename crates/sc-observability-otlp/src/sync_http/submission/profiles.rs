//! OTLP profiles v1development request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{
        Function, KeyValueAndUnit, Line, Location, Mapping, Profile, ProfileLink, Sample, Stack,
        ValueType,
    },
    submission::SubmissionEnvelope,
};
use sc_observability_types::v2::ExportError;
use serde_json::{Map, Value};

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Result<Value, ExportError> {
    let dictionary = envelopes
        .iter()
        .find_map(|envelope| envelope.profiles.as_ref())
        .map(|profiles| wire_dictionary(&profiles.dictionary))
        .transpose()?;
    let groups = resource::group_by_resource_scope(
        envelopes
            .iter()
            .flat_map(|envelope| envelope.profiles.iter())
            .flat_map(|profiles| &profiles.profiles),
        |profile| Ok(wire_profile(profile)),
        "scopeProfiles",
        "profiles",
    )?;
    let mut groups = groups;
    for group in &mut groups {
        if let Some(group) = group.as_object_mut() {
            if let Some(resource) = group.get_mut("resource").and_then(Value::as_object_mut) {
                let schema_url = resource.remove("schemaUrl").unwrap_or(Value::Null);
                group.insert("schemaUrl".to_owned(), schema_url);
            }
            if let Some(scopes) = group.get_mut("scopeProfiles").and_then(Value::as_array_mut) {
                for scope in scopes {
                    if let Some(scope) = scope.as_object_mut()
                        && let Some(scope_value) =
                            scope.get_mut("scope").and_then(Value::as_object_mut)
                    {
                        let schema_url = scope_value.remove("schemaUrl").unwrap_or(Value::Null);
                        scope.insert("schemaUrl".to_owned(), schema_url);
                    }
                }
            }
        }
    }
    let mut encoded = Map::new();
    if let Some(dictionary) = dictionary {
        encoded.insert("dictionary".to_owned(), dictionary);
    }
    encoded.insert("resourceProfiles".to_owned(), Value::Array(groups));
    Ok(Value::Object(encoded))
}

fn wire_dictionary(
    value: &sc_observability_types::otlp::signals::ProfilesDictionary,
) -> Result<Value, ExportError> {
    Ok(Value::Object(Map::from_iter([
        (
            "mappingTable".to_owned(),
            Value::Array(value.mapping_table.iter().map(mapping).collect()),
        ),
        (
            "locationTable".to_owned(),
            Value::Array(value.location_table.iter().map(location).collect()),
        ),
        (
            "functionTable".to_owned(),
            Value::Array(value.function_table.iter().map(function).collect()),
        ),
        (
            "linkTable".to_owned(),
            Value::Array(value.link_table.iter().map(link).collect()),
        ),
        (
            "stringTable".to_owned(),
            Value::Array(
                value
                    .string_table
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        ),
        (
            "attributeTable".to_owned(),
            Value::Array(
                value
                    .attribute_table
                    .iter()
                    .map(attribute)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ),
        (
            "stackTable".to_owned(),
            Value::Array(value.stack_table.iter().map(stack).collect()),
        ),
    ])))
}

fn wire_profile(value: &Profile) -> Value {
    let mut encoded = Map::from_iter([
        (
            "samples".to_owned(),
            Value::Array(value.samples.iter().map(sample).collect()),
        ),
        ("timeUnixNano".to_owned(), resource::timestamp(&value.time)),
        (
            "durationNano".to_owned(),
            values::uint64(value.duration_nanos),
        ),
        ("period".to_owned(), values::int64(value.period)),
        (
            "profileId".to_owned(),
            Value::String(values::base64(&value.profile_id)),
        ),
        (
            "droppedAttributesCount".to_owned(),
            Value::from(value.dropped_attributes_count),
        ),
        (
            "originalPayload".to_owned(),
            Value::String(values::base64(&value.original_payload)),
        ),
        (
            "attributeIndices".to_owned(),
            indices(&value.attribute_indices),
        ),
    ]);
    insert_optional(
        &mut encoded,
        "sampleType",
        value.sample_type.as_ref().map(value_type),
    );
    insert_optional(
        &mut encoded,
        "periodType",
        value.period_type.as_ref().map(value_type),
    );
    insert_optional(
        &mut encoded,
        "originalPayloadFormat",
        value.original_payload_format.clone().map(Value::String),
    );
    Value::Object(encoded)
}

fn mapping(value: &Mapping) -> Value {
    Value::Object(Map::from_iter([
        ("memoryStart".to_owned(), values::uint64(value.memory_start)),
        ("memoryLimit".to_owned(), values::uint64(value.memory_limit)),
        ("fileOffset".to_owned(), values::uint64(value.file_offset)),
        (
            "filenameStrindex".to_owned(),
            Value::from(value.filename_strindex),
        ),
        (
            "attributeIndices".to_owned(),
            indices(&value.attribute_indices),
        ),
    ]))
}

fn location(value: &Location) -> Value {
    Value::Object(Map::from_iter([
        ("mappingIndex".to_owned(), Value::from(value.mapping_index)),
        ("address".to_owned(), values::uint64(value.address)),
        (
            "lines".to_owned(),
            Value::Array(value.lines.iter().map(line).collect()),
        ),
        (
            "attributeIndices".to_owned(),
            indices(&value.attribute_indices),
        ),
    ]))
}

fn function(value: &Function) -> Value {
    Value::Object(Map::from_iter([
        ("nameStrindex".to_owned(), Value::from(value.name_strindex)),
        (
            "systemNameStrindex".to_owned(),
            Value::from(value.system_name_strindex),
        ),
        (
            "filenameStrindex".to_owned(),
            Value::from(value.filename_strindex),
        ),
        ("startLine".to_owned(), values::int64(value.start_line)),
    ]))
}

fn link(value: &ProfileLink) -> Value {
    Value::Object(Map::from_iter([
        (
            "traceId".to_owned(),
            Value::String(values::base64(&value.trace_id)),
        ),
        (
            "spanId".to_owned(),
            Value::String(values::base64(&value.span_id)),
        ),
    ]))
}

fn attribute(value: &KeyValueAndUnit) -> Result<Value, ExportError> {
    let mut encoded = Map::from_iter([
        ("keyStrindex".to_owned(), Value::from(value.key_strindex)),
        ("unitStrindex".to_owned(), Value::from(value.unit_strindex)),
    ]);
    if let Some(value) = &value.value {
        encoded.insert("value".to_owned(), values::any_value(value)?);
    }
    Ok(Value::Object(encoded))
}

fn stack(value: &Stack) -> Value {
    Value::Object(Map::from_iter([(
        "locationIndices".to_owned(),
        indices(&value.location_indices),
    )]))
}

fn sample(value: &Sample) -> Value {
    Value::Object(Map::from_iter([
        ("stackIndex".to_owned(), Value::from(value.stack_index)),
        (
            "attributeIndices".to_owned(),
            indices(&value.attribute_indices),
        ),
        ("linkIndex".to_owned(), Value::from(value.link_index)),
        (
            "values".to_owned(),
            Value::Array(value.values.iter().copied().map(values::int64).collect()),
        ),
        (
            "timestampsUnixNano".to_owned(),
            Value::Array(
                value
                    .timestamps_unix_nano
                    .iter()
                    .copied()
                    .map(values::uint64)
                    .collect(),
            ),
        ),
    ]))
}

fn value_type(value: &ValueType) -> Value {
    Value::Object(Map::from_iter([
        ("typeStrindex".to_owned(), Value::from(value.type_strindex)),
        ("unitStrindex".to_owned(), Value::from(value.unit_strindex)),
    ]))
}

fn line(value: &Line) -> Value {
    Value::Object(Map::from_iter([
        (
            "functionIndex".to_owned(),
            Value::from(value.function_index),
        ),
        ("line".to_owned(), values::int64(value.line)),
        ("column".to_owned(), values::int64(value.column)),
    ]))
}

fn indices(values: &[i32]) -> Value {
    Value::Array(values.iter().copied().map(Value::from).collect())
}

fn insert_optional(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_profile_dictionary_and_grouped_profile() {
        let envelope: SubmissionEnvelope = serde_json::from_str(&super::super::golden_fixture(
            "profiles",
            "expected.envelope.json",
        ))
        .expect("canonical profile fixture parses");
        let value = request(&[envelope]).expect("fixture encodes");
        assert!(value["dictionary"].is_object());
        let profile = &value["resourceProfiles"][0]["scopeProfiles"][0]["profiles"][0];
        assert_eq!(profile["timeUnixNano"], "0");
        assert_eq!(profile["durationNano"], "10");
        assert_eq!(profile["profileId"], "EREREREREREREREREREREQ==");
        assert_eq!(
            value["dictionary"]["linkTable"][0]["traceId"],
            "AAAAAAAAAAAAAAAAAAAAAA=="
        );
        assert!(
            value["dictionary"]["attributeTable"][0]
                .get("value")
                .is_none()
        );
    }
}
