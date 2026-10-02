use sc_observability_types::otlp::signals::{
    AnyValue, AttributeKey, KeyValues, OtlpDouble, StringIndex,
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum NonFiniteDouble {
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

/// OTLP JSON forms the pinned protobuf decoder cannot faithfully deserialize.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct DecodedForms {
    pub(super) bytes: Vec<Vec<u8>>,
    pub(super) non_finite_doubles: Vec<NonFiniteDouble>,
    pub(super) string_value_indices: Vec<u32>,
    pub(super) key_indices: Vec<u32>,
}

/// Reads the unsupported OTLP JSON values directly from a captured request.
pub(super) fn decode_forms(body: &Value) -> Result<DecodedForms, String> {
    let mut forms = DecodedForms::default();
    visit(body, &mut forms)?;
    Ok(forms)
}

/// Decodes the protobuf-JSON spelling of an OTLP value back into the neutral
/// canonical value. Unsigned and signed integer input intentionally shares
/// protobuf's `intValue` spelling, so it decodes as the protocol's signed
/// representation.
pub(super) fn decode_any_value(value: &Value) -> Result<AnyValue, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "OTLP AnyValue must be an object".to_owned())?;
    if object.len() != 1 {
        return Err("OTLP AnyValue must contain exactly one oneof field".to_owned());
    }
    let (name, value) = object
        .iter()
        .next()
        .ok_or_else(|| "OTLP AnyValue cannot be empty".to_owned())?;
    match name.as_str() {
        "stringValue" => Ok(AnyValue::String(string(value, name)?.to_owned())),
        "boolValue" => value
            .as_bool()
            .map(AnyValue::Bool)
            .ok_or_else(|| "boolValue must be a JSON boolean".to_owned()),
        "intValue" => string(value, name)?
            .parse()
            .map(AnyValue::Int)
            .map_err(|error| format!("intValue must be an i64: {error}")),
        "doubleValue" => decode_double(value).map(AnyValue::Double),
        "bytesValue" => decode_base64(string(value, name)?).map(AnyValue::Bytes),
        "stringValueStrindex" => Ok(AnyValue::StringIndex(decode_index(value, name)?)),
        "arrayValue" => value
            .get("values")
            .and_then(Value::as_array)
            .ok_or_else(|| "arrayValue.values must be an array".to_owned())?
            .iter()
            .map(decode_any_value)
            .collect::<Result<Vec<_>, _>>()
            .map(AnyValue::Array),
        "kvlistValue" => decode_key_values(
            value
                .get("values")
                .ok_or_else(|| "kvlistValue.values is required".to_owned())?,
        )
        .map(AnyValue::KvList),
        _ => Err(format!("unsupported OTLP AnyValue field {name}")),
    }
}

/// Decodes ordered protobuf-JSON key/value entries into their canonical form.
pub(super) fn decode_key_values(value: &Value) -> Result<KeyValues, String> {
    let values = value
        .as_array()
        .ok_or_else(|| "OTLP key/value collection must be an array".to_owned())?;
    let entries = values
        .iter()
        .map(|entry| {
            let object = entry
                .as_object()
                .ok_or_else(|| "OTLP key/value entry must be an object".to_owned())?;
            let key = match (object.get("key"), object.get("keyStrindex")) {
                (Some(value), None) => AttributeKey::from(string(value, "key")?),
                (None, Some(value)) => AttributeKey::Index(decode_index(value, "keyStrindex")?),
                _ => return Err("OTLP key/value entry must contain one key field".to_owned()),
            };
            let value = decode_any_value(
                object
                    .get("value")
                    .ok_or_else(|| "OTLP key/value entry requires value".to_owned())?,
            )?;
            Ok((key, value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    KeyValues::try_from_iter(entries).map_err(|error| error.to_string())
}

fn decode_index(value: &Value, field: &str) -> Result<StringIndex, String> {
    let value = index(value, field)?;
    let value = i32::try_from(value).map_err(|_| format!("{field} exceeds i32"))?;
    StringIndex::try_new(value).map_err(|error| error.to_string())
}

fn decode_double(value: &Value) -> Result<OtlpDouble, String> {
    let value = match value {
        Value::Number(value) => value
            .as_f64()
            .ok_or_else(|| "doubleValue must be representable as f64".to_owned())?,
        Value::String(value) => match value.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            _ => return Err("doubleValue has an invalid non-finite spelling".to_owned()),
        },
        _ => return Err("doubleValue must be a JSON number or non-finite spelling".to_owned()),
    };
    Ok(OtlpDouble::new(value))
}

fn visit(value: &Value, forms: &mut DecodedForms) -> Result<(), String> {
    match value {
        Value::Array(values) => values.iter().try_for_each(|value| visit(value, forms)),
        Value::Object(values) => {
            for (key, value) in values {
                match key.as_str() {
                    "bytesValue" => forms.bytes.push(decode_base64(string(value, key)?)?),
                    "doubleValue" => {
                        if let Some(value) = non_finite(value) {
                            forms.non_finite_doubles.push(value);
                        }
                    }
                    "stringValueStrindex" => forms.string_value_indices.push(index(value, key)?),
                    "keyStrindex" => forms.key_indices.push(index(value, key)?),
                    _ => visit(value, forms)?,
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{field} must be a JSON string"))
}

fn index(value: &Value, field: &str) -> Result<u32, String> {
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{field} must be an unsigned integer"))?;
    u32::try_from(value).map_err(|_| format!("{field} exceeds u32"))
}

fn non_finite(value: &Value) -> Option<NonFiniteDouble> {
    match value.as_str() {
        Some("NaN") => Some(NonFiniteDouble::NaN),
        Some("Infinity") => Some(NonFiniteDouble::PositiveInfinity),
        Some("-Infinity") => Some(NonFiniteDouble::NegativeInfinity),
        _ => None,
    }
}

pub(super) fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(4) {
        return Err("base64 length is not divisible by four".to_owned());
    }
    let mut bytes = Vec::with_capacity(value.len() / 4 * 3);
    for (offset, chunk) in value.as_bytes().chunks(4).enumerate() {
        let padding = usize::from(chunk[2] == b'=') + usize::from(chunk[3] == b'=');
        if padding > 2 || (padding > 0 && offset + 1 != value.len() / 4) {
            return Err("invalid base64 padding".to_owned());
        }
        let first = base64_digit(chunk[0])?;
        let second = base64_digit(chunk[1])?;
        let third = if chunk[2] == b'=' {
            0
        } else {
            base64_digit(chunk[2])?
        };
        let fourth = if chunk[3] == b'=' {
            0
        } else {
            base64_digit(chunk[3])?
        };
        bytes.push((first << 2) | (second >> 4));
        if padding < 2 {
            bytes.push((second << 4) | (third >> 2));
        }
        if padding == 0 {
            bytes.push((third << 6) | fourth);
        }
    }
    Ok(bytes)
}

fn base64_digit(value: u8) -> Result<u8, String> {
    match value {
        b'A'..=b'Z' => Ok(value - b'A'),
        b'a'..=b'z' => Ok(value - b'a' + 26),
        b'0'..=b'9' => Ok(value - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(format!("invalid base64 character {value:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reader_decodes_base64_non_finite_and_index_forms() {
        let decoded = decode_forms(&json!({
            "attributes": [{"keyStrindex": 7, "value": {"bytesValue": "Zm8="}}],
            "values": [
                {"stringValueStrindex": 3},
                {"doubleValue": "NaN"},
                {"doubleValue": "Infinity"},
                {"doubleValue": "-Infinity"}
            ]
        }))
        .expect("reader accepts supported proto JSON forms");
        assert_eq!(decoded.bytes, vec![b"fo".to_vec()]);
        assert_eq!(decoded.string_value_indices, vec![3]);
        assert_eq!(decoded.key_indices, vec![7]);
        assert_eq!(
            decoded.non_finite_doubles,
            vec![
                NonFiniteDouble::NaN,
                NonFiniteDouble::PositiveInfinity,
                NonFiniteDouble::NegativeInfinity,
            ]
        );
    }

    #[test]
    fn reader_restores_nested_canonical_values_and_attribute_keys() {
        let expected = KeyValues::try_from_iter([
            (
                AttributeKey::from("text"),
                AnyValue::String("hello".to_owned()),
            ),
            (AttributeKey::from("count"), AnyValue::Int(-42)),
            (
                AttributeKey::Index(StringIndex::try_new(3).expect("index")),
                AnyValue::Array(vec![
                    AnyValue::Bool(true),
                    AnyValue::Double(OtlpDouble::new(1.5)),
                    AnyValue::Bytes(vec![0, 1, 2]),
                ]),
            ),
        ])
        .expect("unique attribute keys");
        let encoded = json!([
            {"key": "text", "value": {"stringValue": "hello"}},
            {"key": "count", "value": {"intValue": "-42"}},
            {"keyStrindex": 3, "value": {"arrayValue": {"values": [
                {"boolValue": true}, {"doubleValue": 1.5}, {"bytesValue": "AAEC"}
            ]}}}
        ]);
        assert_eq!(
            decode_key_values(&encoded).expect("decode values"),
            expected
        );
    }

    #[test]
    fn reader_preserves_non_finite_classes_but_not_nan_payload_bits() {
        let decoded = decode_any_value(&json!({"doubleValue": "NaN"})).expect("decode NaN");
        let AnyValue::Double(value) = decoded else {
            panic!("decoded value must be a double");
        };
        // Protobuf JSON spells every NaN as the same token, so the wire format
        // preserves its class but cannot preserve its payload bits.
        assert!(value.get().is_nan());
    }
}
