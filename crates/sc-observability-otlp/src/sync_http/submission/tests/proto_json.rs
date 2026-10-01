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

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
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
}
