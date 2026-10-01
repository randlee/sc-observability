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
