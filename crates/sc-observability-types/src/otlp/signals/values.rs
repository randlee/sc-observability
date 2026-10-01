//! Lossless values shared by the four signal families.
use std::{collections::HashSet, fmt};

use crate::{ErrorCode, ErrorContext, Remediation, constants};
use sc_lint_attributes::sc_lint;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};

/// A neutral signal violates its data-model constraints.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SignalValidationError {
    /// The named field failed validation; context includes the reason.
    #[error("invalid signal field {path}: {context}")]
    Validation {
        /// Path within the payload.
        path: String,
        /// Original diagnostic and remediation.
        context: Box<ErrorContext>,
    },
}
impl SignalValidationError {
    pub(crate) fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            path: path.into(),
            context: Box::new(ErrorContext::new(
                crate::error_codes::SIGNAL_VALIDATION,
                message,
                Remediation::not_recoverable("Correct the named payload field and resubmit."),
            )),
        }
    }
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        match self {
            Self::Validation { context, .. } => &context.diagnostic().code,
        }
    }
}

/// An IEEE-754 double, with proto-JSON spellings for non-finite values.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct OtlpDouble(f64);
impl OtlpDouble {
    /// Retains the value without coercion.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }
    /// Returns the original value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}
impl From<f64> for OtlpDouble {
    fn from(value: f64) -> Self {
        Self(value)
    }
}
impl PartialEq for OtlpDouble {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Serialize for OtlpDouble {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_nan() {
            serializer.serialize_str("NaN")
        } else if self.0 == f64::INFINITY {
            serializer.serialize_str("Infinity")
        } else if self.0 == f64::NEG_INFINITY {
            serializer.serialize_str("-Infinity")
        } else {
            serializer.serialize_f64(self.0)
        }
    }
}
impl<'de> Deserialize<'de> for OtlpDouble {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DoubleVisitor;
        impl Visitor<'_> for DoubleVisitor {
            type Value = OtlpDouble;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or NaN/Infinity/-Infinity")
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(OtlpDouble(value))
            }
            #[allow(
                clippy::cast_precision_loss,
                reason = "caller explicitly selected a double field"
            )]
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(OtlpDouble(value as f64))
            }
            #[allow(
                clippy::cast_precision_loss,
                reason = "caller explicitly selected a double field"
            )]
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(OtlpDouble(value as f64))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "NaN" => Ok(OtlpDouble(f64::NAN)),
                    "Infinity" => Ok(OtlpDouble(f64::INFINITY)),
                    "-Infinity" => Ok(OtlpDouble(f64::NEG_INFINITY)),
                    _ => Err(E::custom("invalid double spelling")),
                }
            }
        }
        deserializer.deserialize_any(DoubleVisitor)
    }
}

/// Nonnegative index into a profiles dictionary's string table.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i32")]
pub struct StringIndex(i32);
impl StringIndex {
    /// Validates a signed protocol index.
    /// # Errors
    /// Rejects negative indices.
    pub fn try_new(value: i32) -> Result<Self, SignalValidationError> {
        if value < 0 {
            Err(SignalValidationError::invalid(
                "string_index",
                "index must be nonnegative",
            ))
        } else {
            Ok(Self(value))
        }
    }
    /// Returns the protocol index.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}
impl TryFrom<i32> for StringIndex {
    type Error = SignalValidationError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

/// A literal attribute name, or a profiles-only dictionary index.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeKey {
    /// Literal key.
    Name(String),
    /// Profiles dictionary key.
    Index(StringIndex),
}
impl From<String> for AttributeKey {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}
impl From<&str> for AttributeKey {
    fn from(value: &str) -> Self {
        Self::Name(value.to_owned())
    }
}

/// An ordered collection of unique attribute keys.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[sc_lint(boundary.allow("cycle.recursive_value_container"))]
pub struct KeyValues(Vec<(AttributeKey, AnyValue)>);
impl KeyValues {
    /// Retains insertion order and rejects duplicate keys.
    /// # Errors
    /// Returns validation failure for duplicate literal names or indices.
    pub fn try_from_iter(
        items: impl IntoIterator<Item = (AttributeKey, AnyValue)>,
    ) -> Result<Self, SignalValidationError> {
        let mut seen = HashSet::new();
        let mut values = Vec::new();
        for (key, value) in items {
            if !seen.insert(key.clone()) {
                return Err(SignalValidationError::invalid(
                    "attributes",
                    "duplicate key",
                ));
            }
            values.push((key, value));
        }
        Ok(Self(values))
    }
    /// Returns the ordered pairs.
    #[must_use]
    pub fn entries(&self) -> &[(AttributeKey, AnyValue)] {
        &self.0
    }
}

/// Lossless signal value; serialization always uses the canonical tagged form.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
#[sc_lint(boundary.allow("cycle.recursive_value_container"))]
pub enum AnyValue {
    /// UTF-8 text.
    String(String),
    /// Boolean value.
    Bool(bool),
    /// Signed integer.
    Int(i64),
    /// Unsigned input, range checked before admission.
    #[serde(rename = "uint")]
    UInt(u64),
    /// IEEE-754 value.
    Double(OtlpDouble),
    /// Binary data encoded as lowercase hex in neutral JSON.
    Bytes(#[serde(serialize_with = "serialize_bytes")] Vec<u8>),
    /// Profiles-only string-table reference.
    StringIndex(StringIndex),
    /// Ordered nested values.
    Array(Vec<AnyValue>),
    /// Unique-key nested attributes.
    KvList(KeyValues),
}
fn serialize_bytes<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").map_err(serde::ser::Error::custom)?;
    }
    serializer.serialize_str(&hex)
}

// Preserve duplicate object keys until validation; serde_json::Value would erase them.
#[derive(Debug)]
enum InputValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Double(f64),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}
impl<'de> Deserialize<'de> for InputValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct InputVisitor;
        impl<'de> Visitor<'de> for InputVisitor {
            type Value = InputValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a telemetry value")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(InputValue::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(InputValue::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(InputValue::Int(v))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(InputValue::UInt(v))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(InputValue::Double(v))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(InputValue::String(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(InputValue::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = seq.next_element()? {
                    v.push(x);
                }
                Ok(InputValue::Array(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = map.next_entry()? {
                    v.push(x);
                }
                Ok(InputValue::Object(v))
            }
        }
        d.deserialize_any(InputVisitor)
    }
}
/// RFC 6901 JSON pointer into one deserialized value, rendered in URI-fragment
/// form (`#`, `#/attrs/0/data`) so the root is never an empty path.
struct Pointer(String);
impl Pointer {
    fn root() -> Self {
        Self(String::from("#"))
    }
    /// Runs `f` with `segment` appended, escaping `~` and `/` per RFC 6901.
    fn with<T>(&mut self, segment: impl fmt::Display, f: impl FnOnce(&mut Self) -> T) -> T {
        let len = self.0.len();
        self.0.push('/');
        self.0
            .push_str(&segment.to_string().replace('~', "~0").replace('/', "~1"));
        let result = f(self);
        self.0.truncate(len);
        result
    }
    fn invalid(&self, message: impl Into<String>) -> SignalValidationError {
        SignalValidationError::invalid(self.0.clone(), message)
    }
}
fn key_values(
    input: InputValue,
    at: &mut Pointer,
    depth: usize,
) -> Result<KeyValues, SignalValidationError> {
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    match input {
        InputValue::Object(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in entries {
                let value = at.with(&key, |at| {
                    let key = AttributeKey::Name(key.clone());
                    if seen.contains(&key) {
                        return Err(at.invalid("duplicate attribute key"));
                    }
                    any_value(value, at, depth).map(|value| (key, value))
                })?;
                seen.insert(value.0.clone());
                values.push(value);
            }
        }
        InputValue::Array(entries) => {
            for (i, entry) in entries.into_iter().enumerate() {
                let pair = at.with(i, |at| {
                    let InputValue::Array(mut pair) = entry else {
                        return Err(at.invalid("expected a [key, value] attribute pair"));
                    };
                    if pair.len() != 2 {
                        return Err(at.invalid("attribute pair must have two elements"));
                    }
                    let value = at.with(1, |at| match pair.pop() {
                        Some(value) => any_value(value, at, depth),
                        None => Err(at.invalid("missing attribute value")),
                    })?;
                    let key = at.with(0, |at| {
                        let index = |i: Result<i32, _>| {
                            i.map_err(|_| at.invalid("string index exceeds int32"))
                                .and_then(|i| {
                                    StringIndex::try_new(i)
                                        .map_err(|_| at.invalid("string index must be nonnegative"))
                                })
                                .map(AttributeKey::Index)
                        };
                        let key = match pair.pop() {
                            Some(InputValue::String(k)) => AttributeKey::Name(k),
                            Some(InputValue::Int(i)) => index(i32::try_from(i))?,
                            Some(InputValue::UInt(i)) => index(i32::try_from(i))?,
                            _ => return Err(at.invalid("attribute key must be a string or index")),
                        };
                        if seen.contains(&key) {
                            return Err(at.invalid("duplicate attribute key"));
                        }
                        Ok(key)
                    })?;
                    Ok((key, value))
                })?;
                seen.insert(pair.0.clone());
                values.push(pair);
            }
        }
        _ => return Err(at.invalid("expected an attributes object or pair list")),
    }
    Ok(KeyValues(values))
}
fn any_value(
    input: InputValue,
    at: &mut Pointer,
    depth: usize,
) -> Result<AnyValue, SignalValidationError> {
    if depth > constants::ANY_VALUE_MAX_DEPTH {
        return Err(at.invalid(format!(
            "value nesting exceeds {} levels",
            constants::ANY_VALUE_MAX_DEPTH
        )));
    }
    Ok(match input {
        InputValue::Null => return Err(at.invalid("null is not a telemetry value")),
        InputValue::Bool(v) => AnyValue::Bool(v),
        InputValue::Int(v) => AnyValue::Int(v),
        InputValue::UInt(v) => i64::try_from(v).map_or(AnyValue::UInt(v), AnyValue::Int),
        InputValue::Double(v) => AnyValue::Double(v.into()),
        InputValue::String(v) => AnyValue::String(v),
        InputValue::Array(v) => AnyValue::Array(array(v, at, depth)?),
        InputValue::Object(mut entries) => {
            let tag = if entries.len() == 2 && entries.iter().any(|(k, _)| k == "data") {
                entries.iter().find_map(|(k, v)| match (k.as_str(), v) {
                    ("kind", InputValue::String(s)) => Some(s.clone()),
                    _ => None,
                })
            } else {
                None
            };
            if let Some(tag) = tag.filter(|s| {
                matches!(
                    s.as_str(),
                    "string"
                        | "bool"
                        | "int"
                        | "uint"
                        | "double"
                        | "bytes"
                        | "string_index"
                        | "array"
                        | "kv_list"
                )
            }) {
                let position = entries
                    .iter()
                    .position(|(k, _)| k == "data")
                    .ok_or_else(|| at.invalid("missing tagged data"))?;
                let data = entries.swap_remove(position).1;
                at.with("data", |at| tagged(&tag, data, at, depth))?
            } else {
                AnyValue::KvList(key_values(InputValue::Object(entries), at, depth + 1)?)
            }
        }
    })
}
fn array(
    values: Vec<InputValue>,
    at: &mut Pointer,
    depth: usize,
) -> Result<Vec<AnyValue>, SignalValidationError> {
    values
        .into_iter()
        .enumerate()
        .map(|(i, v)| at.with(i, |at| any_value(v, at, depth + 1)))
        .collect()
}
#[allow(
    clippy::cast_precision_loss,
    reason = "explicit double tag requests floating point representation"
)]
fn tagged(
    tag: &str,
    data: InputValue,
    at: &mut Pointer,
    depth: usize,
) -> Result<AnyValue, SignalValidationError> {
    let index = |i: Result<i32, _>, at: &Pointer| {
        i.map_err(|_| at.invalid("string index exceeds int32"))
            .and_then(|i| {
                StringIndex::try_new(i).map_err(|_| at.invalid("string index must be nonnegative"))
            })
            .map(AnyValue::StringIndex)
    };
    match (tag, data) {
        ("string", InputValue::String(v)) => Ok(AnyValue::String(v)),
        ("bool", InputValue::Bool(v)) => Ok(AnyValue::Bool(v)),
        ("int", InputValue::Int(v)) => Ok(AnyValue::Int(v)),
        ("int", InputValue::UInt(v)) => Ok(AnyValue::Int(
            i64::try_from(v).map_err(|_| at.invalid("int overflow"))?,
        )),
        ("uint", InputValue::UInt(v)) => Ok(AnyValue::UInt(v)),
        ("uint", InputValue::Int(v)) => Ok(AnyValue::UInt(
            u64::try_from(v).map_err(|_| at.invalid("negative uint"))?,
        )),
        ("double", InputValue::Double(v)) => Ok(AnyValue::Double(v.into())),
        ("double", InputValue::Int(v)) => Ok(AnyValue::Double((v as f64).into())),
        ("double", InputValue::UInt(v)) => Ok(AnyValue::Double((v as f64).into())),
        ("double", InputValue::String(v)) => Ok(AnyValue::Double(
            match v.as_str() {
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => return Err(at.invalid("invalid double spelling")),
            }
            .into(),
        )),
        ("bytes", InputValue::String(v)) => {
            if !v.len().is_multiple_of(2) || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(at.invalid("bytes must be hex pairs"));
            }
            let bytes = v
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    std::str::from_utf8(pair)
                        .ok()
                        .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                        .ok_or_else(|| at.invalid("invalid byte"))
                })
                .collect::<Result<_, _>>()?;
            Ok(AnyValue::Bytes(bytes))
        }
        ("string_index", InputValue::Int(v)) => index(i32::try_from(v), at),
        ("string_index", InputValue::UInt(v)) => index(i32::try_from(v), at),
        ("array", InputValue::Array(v)) => Ok(AnyValue::Array(array(v, at, depth)?)),
        ("kv_list", v) => Ok(AnyValue::KvList(key_values(v, at, depth + 1)?)),
        _ => Err(at.invalid(format!("data does not match kind {tag}"))),
    }
}
impl TryFrom<InputValue> for KeyValues {
    type Error = SignalValidationError;
    fn try_from(input: InputValue) -> Result<Self, Self::Error> {
        key_values(input, &mut Pointer::root(), 1)
    }
}
impl TryFrom<InputValue> for AnyValue {
    type Error = SignalValidationError;
    fn try_from(input: InputValue) -> Result<Self, Self::Error> {
        any_value(input, &mut Pointer::root(), 1)
    }
}
impl<'de> Deserialize<'de> for AnyValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_from(InputValue::deserialize(d)?).map_err(de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for KeyValues {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_from(InputValue::deserialize(d)?).map_err(de::Error::custom)
    }
}

/// Validated W3C tracestate, preserving member order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct TraceState(String);
impl TraceState {
    /// Validates tracestate grammar, size and unique keys.
    /// # Errors
    /// Rejects invalid key/value grammar, duplicate keys, or excessive size.
    pub fn try_new(value: impl Into<String>) -> Result<Self, SignalValidationError> {
        let invalid = |message: &str| SignalValidationError::invalid("trace_state", message);
        let value = value.into();
        if value.len() > constants::TRACE_STATE_MAX_BYTES {
            return Err(invalid("tracestate exceeds byte limit"));
        }
        if value.is_empty() {
            return Ok(Self(value));
        }
        let mut keys = HashSet::new();
        for member in value.split(',') {
            let member = member.trim_matches([' ', '\t']);
            let Some((key, val)) = member.split_once('=') else {
                return Err(invalid("tracestate requires key=value"));
            };
            if !valid_key(key)
                || !keys.insert(key)
                || keys.len() > constants::TRACE_STATE_MAX_MEMBERS
                || val.is_empty()
                || val.len() > constants::TRACE_STATE_MEMBER_MAX_BYTES
                || val.ends_with(' ')
                || !val.bytes().all(|b| {
                    (constants::TRACE_STATE_VALUE_BYTE_MIN..=constants::TRACE_STATE_VALUE_BYTE_MAX)
                        .contains(&b)
                        && b != b','
                        && b != b'='
                })
            {
                return Err(invalid("invalid tracestate member"));
            }
        }
        Ok(Self(value))
    }
    /// Returns the original header.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn valid_key(key: &str) -> bool {
    fn chars(s: &str) -> bool {
        s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-*/".contains(&b))
    }
    if let Some((tenant, system)) = key.split_once('@') {
        !tenant.is_empty()
            && tenant.len() <= constants::TRACE_STATE_TENANT_MAX_BYTES
            && tenant.as_bytes()[0].is_ascii_alphanumeric()
            && chars(tenant)
            && !system.is_empty()
            && system.len() <= constants::TRACE_STATE_SYSTEM_MAX_BYTES
            && system.as_bytes()[0].is_ascii_lowercase()
            && chars(system)
    } else {
        !key.is_empty()
            && key.len() <= constants::TRACE_STATE_MEMBER_MAX_BYTES
            && key.as_bytes()[0].is_ascii_lowercase()
            && chars(key)
    }
}
impl TryFrom<String> for TraceState {
    type Error = SignalValidationError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::try_new(v)
    }
}

#[cfg(test)]
mod tests {
    use super::{AnyValue, KeyValues, SignalValidationError, TraceState};
    use crate::constants;

    #[test]
    fn nested_value_errors_report_json_pointer_paths() {
        let cases = [
            (r#"{"outer":{"inner":[1,null]}}"#, "#/outer/inner/1"),
            (r#"{"a/b":{"c~d":null}}"#, "#/a~1b/c~0d"),
            (r#"[["k",1],[2]]"#, "#/1"),
            (r#"[["k",1],[-1,true]]"#, "#/1/0"),
            (r#"[["k",1],["k",2]]"#, "#/1/0"),
            (r#"{"x":{"kind":"bytes","data":"zz"}}"#, "#/x/data"),
            (
                r#"{"x":{"kind":"array","data":[true,{"kind":"int","data":"1"}]}}"#,
                "#/x/data/1/data",
            ),
        ];
        for (json, pointer) in cases {
            let error = serde_json::from_str::<KeyValues>(json).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("invalid signal field {pointer}:")),
                "{json}: {error}"
            );
        }
        let error = serde_json::from_str::<AnyValue>("null").unwrap_err();
        assert!(
            error.to_string().contains("invalid signal field #:"),
            "{error}"
        );
    }

    #[test]
    fn deserialization_bounds_value_depth() {
        let ok =
            (1..constants::ANY_VALUE_MAX_DEPTH).fold(String::from("1"), |v, _| format!("[{v}]"));
        assert!(serde_json::from_str::<AnyValue>(&ok).is_ok());
        let deep = format!("[{ok}]");
        let error = serde_json::from_str::<AnyValue>(&deep).unwrap_err();
        assert!(error.to_string().contains("nesting exceeds"), "{error}");
    }

    #[test]
    fn tracestate_errors_name_the_field() {
        match TraceState::try_new("Bad=1") {
            Err(SignalValidationError::Validation { path, .. }) => assert_eq!(path, "trace_state"),
            other => panic!("expected tracestate rejection, got {other:?}"),
        }
        let control = format!(
            "k=a{}b",
            char::from(constants::TRACE_STATE_VALUE_BYTE_MIN - 1)
        );
        assert!(TraceState::try_new(control).is_err());
        let tilde = format!("k=a{}", char::from(constants::TRACE_STATE_VALUE_BYTE_MAX));
        assert!(TraceState::try_new(tilde).is_ok());
    }
}
