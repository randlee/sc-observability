//! Shared resource and instrumentation-scope OTLP/JSON encoders.

use super::values::key_values;
use sc_observability_types::{
    Timestamp,
    otlp::signals::{InstrumentationScope, Resource, ResourceRecord},
    v2::ExportError,
};
use serde_json::{Map, Value};

pub(super) fn resource(value: &Resource) -> Result<Map<String, Value>, ExportError> {
    let mut encoded = Map::new();
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(key_values(&value.attributes)?),
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
    Ok(encoded)
}

pub(super) fn scope(value: &InstrumentationScope) -> Result<Map<String, Value>, ExportError> {
    let mut encoded = Map::new();
    encoded.insert("name".to_owned(), Value::String(value.name.clone()));
    insert_string(&mut encoded, "version", value.version.as_ref());
    encoded.insert(
        "attributes".to_owned(),
        Value::Array(key_values(&value.attributes)?),
    );
    encoded.insert(
        "droppedAttributesCount".to_owned(),
        Value::from(value.dropped_attributes_count),
    );
    insert_string(&mut encoded, "schemaUrl", value.schema_url.as_ref());
    Ok(encoded)
}

struct ResourceScopeGroup {
    resource: Resource,
    scopes: Vec<InstrumentationScopeGroup>,
}

struct InstrumentationScopeGroup {
    scope: InstrumentationScope,
    records: Vec<Value>,
}

/// Groups records by resource and instrumentation scope, preserving first-seen
/// order and propagating record encoding errors.
pub(super) fn group_by_resource_scope<'a, T: 'a>(
    records: impl IntoIterator<Item = &'a ResourceRecord<T>>,
    mut encode_record: impl FnMut(&T) -> Result<Value, ExportError>,
    scopes_key: &str,
    records_key: &str,
) -> Result<Vec<Value>, ExportError> {
    let mut groups: Vec<ResourceScopeGroup> = Vec::new();
    for record in records {
        let resource_index = groups
            .iter()
            .position(|group| group.resource == record.resource)
            .unwrap_or_else(|| {
                groups.push(ResourceScopeGroup {
                    resource: record.resource.clone(),
                    scopes: Vec::new(),
                });
                groups.len() - 1
            });
        let resource_group = &mut groups[resource_index];
        let scope_index = resource_group
            .scopes
            .iter()
            .position(|group| group.scope == record.scope)
            .unwrap_or_else(|| {
                resource_group.scopes.push(InstrumentationScopeGroup {
                    scope: record.scope.clone(),
                    records: Vec::new(),
                });
                resource_group.scopes.len() - 1
            });
        resource_group.scopes[scope_index]
            .records
            .push(encode_record(&record.record)?);
    }
    groups
        .into_iter()
        .map(|group| {
            let mut encoded = Map::new();
            let mut resource = resource(&group.resource)?;
            if let Some(schema_url) = resource.remove("schemaUrl") {
                encoded.insert("schemaUrl".to_owned(), schema_url);
            }
            encoded.insert("resource".to_owned(), Value::Object(resource));
            encoded.insert(
                scopes_key.to_owned(),
                Value::Array(
                    group
                        .scopes
                        .into_iter()
                        .map(|scope| {
                            let mut encoded = Map::new();
                            let mut scope_value = self::scope(&scope.scope)?;
                            if let Some(schema_url) = scope_value.remove("schemaUrl") {
                                encoded.insert("schemaUrl".to_owned(), schema_url);
                            }
                            encoded.insert("scope".to_owned(), Value::Object(scope_value));
                            encoded.insert(records_key.to_owned(), Value::Array(scope.records));
                            Ok(Value::Object(encoded))
                        })
                        .collect::<Result<Vec<_>, ExportError>>()?,
                ),
            );
            Ok(Value::Object(encoded))
        })
        .collect()
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
