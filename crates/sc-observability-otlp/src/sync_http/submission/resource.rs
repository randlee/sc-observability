//! Shared resource and instrumentation-scope OTLP/JSON encoders.

use super::values::key_values;
use sc_observability_types::{
    Timestamp,
    otlp::signals::{InstrumentationScope, Resource},
};
use serde_json::{Map, Value};

pub(super) fn resource(value: &Resource) -> Map<String, Value> {
    let mut encoded = Map::new();
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(key_values(&value.attributes)),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(value.dropped_attributes_count),
    );
    encoded.insert(
        "entityRefs".to_owned(),
        Value::Array(
            value
                .entity_refs
                .iter()
                .map(|entity| {
                    let mut encoded = Map::new();
                    insert_string(&mut encoded, "schemaUrl", entity.schema_url.as_ref());
                    encoded.insert("type".to_owned(), Value::String(entity.r#type.clone()));
                    encoded.insert(
                        "idKeys".to_owned(),
                        Value::Array(entity.id_keys.iter().cloned().map(Value::String).collect()),
                    );
                    encoded.insert(
                        "descriptionKeys".to_owned(),
                        Value::Array(
                            entity
                                .description_keys
                                .iter()
                                .cloned()
                                .map(Value::String)
                                .collect(),
                        ),
                    );
                    Value::Object(encoded)
                })
                .collect(),
        ),
    );
    insert_string(&mut encoded, "schemaUrl", value.schema_url.as_ref());
    encoded
}

pub(super) fn scope(value: &InstrumentationScope) -> Map<String, Value> {
    let mut encoded = Map::new();
    encoded.insert("name".to_owned(), Value::String(value.name.clone()));
    insert_string(&mut encoded, "version", value.version.as_ref());
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(key_values(&value.attributes)),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(value.dropped_attributes_count),
    );
    insert_string(&mut encoded, "schemaUrl", value.schema_url.as_ref());
    encoded
}

/// Returns the group for a resource/scope pair, creating each layer once.
///
/// Signal encoders supply their record wrapper and encoder only; this keeps
/// the grouping semantics identical for logs, traces, metrics, and profiles.
#[expect(
    clippy::too_many_arguments,
    reason = "the four signal encoders supply their distinct group and scope wrappers"
)]
pub(super) fn resource_scope_group<'a, G, S>(
    groups: &'a mut Vec<G>,
    resource: &Resource,
    scope: &InstrumentationScope,
    resource_of: impl Fn(&G) -> &Resource,
    scopes: impl Fn(&mut G) -> &mut Vec<S>,
    scope_of: impl Fn(&S) -> &InstrumentationScope,
    new_resource_group: impl FnOnce(Resource) -> G,
    new_scope_group: impl FnOnce(InstrumentationScope) -> S,
) -> &'a mut S {
    let resource_index = groups
        .iter()
        .position(|group| resource_of(group) == resource)
        .unwrap_or_else(|| {
            groups.push(new_resource_group(resource.clone()));
            groups.len() - 1
        });
    let scoped_groups = scopes(&mut groups[resource_index]);
    let scope_index = scoped_groups
        .iter()
        .position(|group| scope_of(group) == scope)
        .unwrap_or_else(|| {
            scoped_groups.push(new_scope_group(scope.clone()));
            scoped_groups.len() - 1
        });
    &mut scoped_groups[scope_index]
}

pub(super) fn timestamp(value: &Timestamp) -> Value {
    Value::String(value.into_inner().unix_timestamp_nanos().to_string())
}

pub(super) fn insert_timestamp(map: &mut Map<String, Value>, key: &str, value: Option<&Timestamp>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), timestamp(value));
    }
}

pub(super) fn insert_string(map: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), Value::String(value.clone()));
    }
}
