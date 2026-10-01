//! OTLP profiles v1development request encoder.

use super::{resource, values};
use sc_observability_types::otlp::{
    signals::{InstrumentationScope, Profile, Resource, ResourceRecord},
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
                    Value::Object(Map::from_iter([
                        ("resource".to_owned(), resource::resource(&group.resource)),
                        (
                            "scopeProfiles".to_owned(),
                            Value::Array(
                                group
                                    .scopes
                                    .into_iter()
                                    .map(|scope| {
                                        Value::Object(Map::from_iter([
                                            ("scope".to_owned(), resource::scope(&scope.scope)),
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
    camelize(
        serde_json::to_value(value)
            .expect("neutral profile dictionary serializes deterministically"),
    )
}

fn wire_profile(value: &Profile) -> Value {
    camelize(serde_json::to_value(value).expect("neutral profile serializes deterministically"))
}

fn camelize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(camelize).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let key = camel(&key);
                    let value = if key == "originalPayload" {
                        bytes(value)
                    } else {
                        camelize(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        value => value,
    }
}

fn camel(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut uppercase = false;
    for character in value.chars() {
        if character == '_' {
            uppercase = true;
        } else if uppercase {
            output.extend(character.to_uppercase());
            uppercase = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn bytes(value: Value) -> Value {
    let Value::Array(values) = value else {
        return value;
    };
    let bytes = values
        .into_iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .expect("profile payload bytes are u8")
        })
        .collect::<Vec<_>>();
    Value::String(values::base64(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_profile_dictionary_and_grouped_profile() {
        let envelope: SubmissionEnvelope = serde_json::from_str(include_str!(
            "../../../../sc-observability-types/tests/fixtures/otlp_submission/golden/profiles/expected.envelope.json"
        ))
        .expect("canonical profile fixture parses");
        let value = request(&[envelope]);
        assert!(value["dictionary"].is_object());
        assert!(value["resourceProfiles"][0]["scopeProfiles"][0]["profiles"][0].is_object());
    }
}
