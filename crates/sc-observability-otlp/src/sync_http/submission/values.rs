//! OTLP/JSON representations shared by all submission signal encoders.

use sc_observability_types::otlp::signals::{AnyValue, AttributeKey, KeyValues, OtlpDouble};
use serde_json::{Map, Number, Value};

/// Encodes ordered OTLP attributes without passing the neutral tagged shape
/// through to the wire format.
pub(super) fn key_values(values: &KeyValues) -> Vec<Value> {
    values
        .entries()
        .iter()
        .map(|(key, value)| {
            let mut item = Map::new();
            match key {
                AttributeKey::Name(key) => {
                    item.insert("key".to_owned(), Value::String(key.clone()));
                }
                AttributeKey::Index(index) => {
                    item.insert("keyStrindex".to_owned(), Value::from(index.get()));
                }
                _ => {
                    unreachable!("new AttributeKey variants require an explicit OTLP/JSON mapping")
                }
            }
            item.insert("value".to_owned(), any_value(value));
            Value::Object(item)
        })
        .collect()
}

/// Encodes a neutral value in the OTLP protobuf JSON oneof spelling.
pub(super) fn any_value(value: &AnyValue) -> Value {
    let mut encoded = Map::new();
    match value {
        AnyValue::String(value) => {
            encoded.insert("stringValue".to_owned(), Value::String(value.clone()));
        }
        AnyValue::Bool(value) => {
            encoded.insert("boolValue".to_owned(), Value::Bool(*value));
        }
        AnyValue::Int(value) => {
            encoded.insert("intValue".to_owned(), Value::String(value.to_string()));
        }
        AnyValue::UInt(value) => {
            encoded.insert("intValue".to_owned(), Value::String(value.to_string()));
        }
        AnyValue::Double(value) => {
            encoded.insert("doubleValue".to_owned(), double(*value));
        }
        AnyValue::Bytes(value) => {
            encoded.insert("bytesValue".to_owned(), Value::String(base64(value)));
        }
        AnyValue::StringIndex(value) => {
            encoded.insert("stringValueStrindex".to_owned(), Value::from(value.get()));
        }
        AnyValue::Array(values) => {
            encoded.insert(
                "arrayValue".to_owned(),
                Value::Object(Map::from_iter([(
                    "values".to_owned(),
                    Value::Array(values.iter().map(any_value).collect()),
                )])),
            );
        }
        AnyValue::KvList(values) => {
            encoded.insert(
                "kvlistValue".to_owned(),
                Value::Object(Map::from_iter([(
                    "values".to_owned(),
                    Value::Array(key_values(values)),
                )])),
            );
        }
        _ => unreachable!("new AnyValue variants require an explicit OTLP/JSON mapping"),
    }
    Value::Object(encoded)
}

/// Uses the protobuf JSON spellings for non-finite IEEE-754 values.
pub(super) fn double(value: OtlpDouble) -> Value {
    match value.get() {
        value if value.is_nan() => Value::String("NaN".to_owned()),
        f64::INFINITY => Value::String("Infinity".to_owned()),
        f64::NEG_INFINITY => Value::String("-Infinity".to_owned()),
        value => Value::Number(Number::from_f64(value).expect("finite f64 is a JSON number")),
    }
}

/// Encodes bytes using the RFC 4648 standard alphabet without a new dependency.
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from(((first & 0b0000_0011) << 4) | (second >> 4))],
        ));
        output.push(if chunk.len() > 1 {
            char::from(ALPHABET[usize::from(((second & 0b0000_1111) << 2) | (third >> 6))])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(ALPHABET[usize::from(third & 0b0011_1111)])
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_each_padding_width() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }
}
