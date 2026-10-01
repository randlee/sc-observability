//! OTLP profiles v1development request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{
        Function, InstrumentationScope, KeyValueAndUnit, Line, Location, Mapping, Profile,
        ProfileLink, Resource, ResourceRecord, Sample, Stack, ValueType,
    },
    submission::SubmissionEnvelope,
};
use serde_json::{Map, Value};

struct ScopeProfiles {
    scope: InstrumentationScope,
    profiles: Vec<Value>,
}
struct ResourceProfiles {
    resource: Resource,
    scopes: Vec<ScopeProfiles>,
}

pub(super) fn request(envelopes: &[SubmissionEnvelope]) -> Value {
    let mut groups = Vec::new();
    let dictionary = envelopes
        .iter()
        .find_map(|envelope| envelope.profiles.as_ref())
        .map(|profiles| wire_dictionary(&profiles.dictionary));
    for envelope in envelopes {
        let Some(profiles) = &envelope.profiles else {
            continue;
        };
        for profile in &profiles.profiles {
            append(&mut groups, profile);
        }
    }
    let mut encoded = Map::new();
    if let Some(dictionary) = dictionary {
        encoded.insert("dictionary".to_owned(), dictionary);
    }
    encoded.insert(
        "resourceProfiles".to_owned(),
        Value::Array(
            groups
                .into_iter()
                .map(|group| {
                    let schema_url = group.resource.schema_url.clone();
                    let mut resource = resource::resource(&group.resource);
                    resource.remove("schemaUrl");
                    Value::Object(Map::from_iter([
                        ("resource".to_owned(), Value::Object(resource)),
                        ("schemaUrl".to_owned(), optional_string(schema_url)),
                        (
                            "scopeProfiles".to_owned(),
                            Value::Array(
                                group
                                    .scopes
                                    .into_iter()
                                    .map(|scope| {
                                        let schema_url = scope.scope.schema_url.clone();
                                        let mut scope_value = resource::scope(&scope.scope);
                                        scope_value.remove("schemaUrl");
                                        Value::Object(Map::from_iter([
                                            ("scope".to_owned(), Value::Object(scope_value)),
                                            ("schemaUrl".to_owned(), optional_string(schema_url)),
                                            ("profiles".to_owned(), Value::Array(scope.profiles)),
                                        ]))
                                    })
                                    .collect(),
                            ),
                        ),
                    ]))
                })
                .collect(),
        ),
    );
    Value::Object(encoded)
}

fn append(groups: &mut Vec<ResourceProfiles>, profile: &ResourceRecord<Profile>) {
    let resource_group = if let Some(group) = groups
        .iter_mut()
        .find(|group| group.resource == profile.resource)
    {
        group
    } else {
        groups.push(ResourceProfiles {
            resource: profile.resource.clone(),
            scopes: Vec::new(),
        });
        groups.last_mut().expect("pushed resource group")
    };
    let scope_group = if let Some(group) = resource_group
        .scopes
        .iter_mut()
        .find(|group| group.scope == profile.scope)
    {
        group
    } else {
        resource_group.scopes.push(ScopeProfiles {
            scope: profile.scope.clone(),
            profiles: Vec::new(),
        });
        resource_group
            .scopes
            .last_mut()
            .expect("pushed scope group")
    };
    scope_group.profiles.push(wire_profile(&profile.record));
}

fn wire_dictionary(value: &sc_observability_types::otlp::signals::ProfilesDictionary) -> Value {
    Value::Object(Map::from_iter([
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
            Value::Array(value.attribute_table.iter().map(attribute).collect()),
        ),
        (
            "stackTable".to_owned(),
            Value::Array(value.stack_table.iter().map(stack).collect()),
        ),
    ]))
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
        ("memoryStart".to_owned(), uint(value.memory_start)),
        ("memoryLimit".to_owned(), uint(value.memory_limit)),
        ("fileOffset".to_owned(), uint(value.file_offset)),
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
        ("address".to_owned(), uint(value.address)),
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
        ("startLine".to_owned(), int(value.start_line)),
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

fn attribute(value: &KeyValueAndUnit) -> Value {
    let mut encoded = Map::from_iter([
        ("keyStrindex".to_owned(), Value::from(value.key_strindex)),
        ("unitStrindex".to_owned(), Value::from(value.unit_strindex)),
    ]);
    insert_optional(
        &mut encoded,
        "value",
        value.value.as_ref().map(values::any_value),
    );
    Value::Object(encoded)
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
            Value::Array(value.values.iter().copied().map(int).collect()),
        ),
        (
            "timestampsUnixNano".to_owned(),
            Value::Array(
                value
                    .timestamps_unix_nano
                    .iter()
                    .copied()
                    .map(uint)
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
        ("line".to_owned(), int(value.line)),
        ("column".to_owned(), int(value.column)),
    ]))
}

fn indices(values: &[i32]) -> Value {
    Value::Array(values.iter().copied().map(Value::from).collect())
}

fn uint(value: u64) -> Value {
    values::uint64(value)
}

fn int(value: i64) -> Value {
    values::int64(value)
}

fn optional_string(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
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
        let value = request(&[envelope]);
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
