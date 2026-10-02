//! OTLP/JSON representations shared by all submission signal encoders.

use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::signals::{AnyValue, AttributeKey, KeyValues, OtlpDouble},
    v2::ExportError,
};
use serde_json::{Map, Number, Value};

/// Encodes ordered OTLP attributes without passing the neutral tagged shape
/// through to the wire format.
pub(super) fn key_values(values: &KeyValues) -> Result<Vec<Value>, ExportError> {
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
                _ => return Err(unsupported_variant("AttributeKey")),
            }
            item.insert("value".to_owned(), any_value(value)?);
            Ok(Value::Object(item))
        })
        .collect()
}

/// Encodes a neutral value in the OTLP protobuf JSON oneof spelling.
pub(super) fn any_value(value: &AnyValue) -> Result<Value, ExportError> {
    let mut encoded = Map::new();
    match value {
        AnyValue::String(value) => {
            encoded.insert("stringValue".to_owned(), Value::String(value.clone()));
        }
        AnyValue::Bool(value) => {
            encoded.insert("boolValue".to_owned(), Value::Bool(*value));
        }
        AnyValue::Int(value) => {
            encoded.insert("intValue".to_owned(), int64(*value));
        }
        AnyValue::UInt(value) => {
            encoded.insert("intValue".to_owned(), uint64(*value));
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
                    Value::Array(
                        values
                            .iter()
                            .map(any_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                )])),
            );
        }
        AnyValue::KvList(values) => {
            encoded.insert(
                "kvlistValue".to_owned(),
                Value::Object(Map::from_iter([(
                    "values".to_owned(),
                    Value::Array(key_values(values)?),
                )])),
            );
        }
        _ => return Err(unsupported_variant("AnyValue")),
    }
    Ok(Value::Object(encoded))
}

/// Emits the terminal coded error required when a newer neutral variant has
/// no safe OTLP/JSON representation in this installed exporter.
pub(super) fn unsupported_variant(variant: &str) -> ExportError {
    ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            crate::error_codes::OTLP_EXPORT_TERMINAL,
            format!("this sc-observability-otlp version cannot encode {variant}"),
            Remediation::not_recoverable(
                "upgrade sc-observability-otlp to a version that encodes this signal",
            ),
        )),
    }
}

/// Encodes signed 64-bit values with the protobuf JSON decimal-string rule.
pub(super) fn int64(value: i64) -> Value {
    Value::String(value.to_string())
}

/// Encodes unsigned 64-bit values with the protobuf JSON decimal-string rule.
pub(super) fn uint64(value: u64) -> Value {
    Value::String(value.to_string())
}

/// Uses the protobuf JSON spellings for non-finite IEEE-754 values.
pub(super) fn double(value: OtlpDouble) -> Value {
    let value = value.get();
    if value.is_finite() {
        return Number::from_f64(value).map_or_else(
            || Value::String(non_finite_spelling(value).to_owned()),
            Value::Number,
        );
    }
    Value::String(non_finite_spelling(value).to_owned())
}

fn non_finite_spelling(value: f64) -> &'static str {
    if value.is_nan() {
        "NaN"
    } else if value.is_sign_positive() {
        "Infinity"
    } else {
        "-Infinity"
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
    fn base64_encodes_rfc_4648_section_10_vectors() {
        for (plain, encoded) in [
            (b"".as_slice(), ""),
            (b"f".as_slice(), "Zg=="),
            (b"fo".as_slice(), "Zm8="),
            (b"foo".as_slice(), "Zm9v"),
            (b"foob".as_slice(), "Zm9vYg=="),
            (b"fooba".as_slice(), "Zm9vYmE="),
            (b"foobar".as_slice(), "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain), encoded);
        }
    }
}
