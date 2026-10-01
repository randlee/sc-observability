//! Lossless values shared by the four signal families.
use std::{collections::HashSet, fmt};

use crate::{ErrorCode, ErrorContext, Remediation, constants};
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
fn invalid(message: &str) -> SignalValidationError {
    SignalValidationError::invalid("value", message)
}
impl TryFrom<InputValue> for KeyValues {
    type Error = SignalValidationError;
    fn try_from(input: InputValue) -> Result<Self, Self::Error> {
        let pairs = match input {
            InputValue::Object(mut entries) => {
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
                    .into_iter()
                    .map(|(k, v)| Ok((AttributeKey::Name(k), AnyValue::try_from(v)?)))
                    .collect::<Result<Vec<_>, Self::Error>>()?
            }
            InputValue::Array(entries) => entries
                .into_iter()
                .map(|entry| {
                    let InputValue::Array(mut pair) = entry else {
                        return Err(invalid("expected attribute pair"));
                    };
                    if pair.len() != 2 {
                        return Err(invalid("attribute pair must have two elements"));
                    }
                    let value =
                        AnyValue::try_from(pair.pop().ok_or_else(|| invalid("missing value"))?)?;
                    let key = match pair.pop() {
                        Some(InputValue::String(k)) => AttributeKey::Name(k),
                        Some(InputValue::Int(i)) => AttributeKey::Index(StringIndex::try_new(
                            i32::try_from(i).map_err(|_| invalid("index overflow"))?,
                        )?),
                        Some(InputValue::UInt(i)) => AttributeKey::Index(StringIndex::try_new(
                            i32::try_from(i).map_err(|_| invalid("index overflow"))?,
                        )?),
                        _ => return Err(invalid("invalid attribute key")),
                    };
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
            _ => return Err(invalid("expected attributes object or pair list")),
        };
        Self::try_from_iter(pairs)
    }
}
impl TryFrom<InputValue> for AnyValue {
    type Error = SignalValidationError;
    fn try_from(input: InputValue) -> Result<Self, Self::Error> {
        Ok(match input {
            InputValue::Null => return Err(invalid("null is not a telemetry value")),
            InputValue::Bool(v) => Self::Bool(v),
            InputValue::Int(v) => Self::Int(v),
            InputValue::UInt(v) => i64::try_from(v).map_or(Self::UInt(v), Self::Int),
            InputValue::Double(v) => Self::Double(v.into()),
            InputValue::String(v) => Self::String(v),
            InputValue::Array(v) => Self::Array(
                v.into_iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            ),
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
                        .ok_or_else(|| invalid("missing tagged data"))?;
                    Self::tagged(&tag, entries.swap_remove(position).1)?
                } else {
                    Self::KvList(KeyValues::try_from(InputValue::Object(entries))?)
                }
            }
        })
    }
}
impl AnyValue {
    #[allow(
        clippy::cast_precision_loss,
        reason = "explicit double tag requests floating point representation"
    )]
    fn tagged(tag: &str, data: InputValue) -> Result<Self, SignalValidationError> {
        match (tag, data) {
            ("string", InputValue::String(v)) => Ok(Self::String(v)),
            ("bool", InputValue::Bool(v)) => Ok(Self::Bool(v)),
            ("int", InputValue::Int(v)) => Ok(Self::Int(v)),
            ("int", InputValue::UInt(v)) => Ok(Self::Int(
                i64::try_from(v).map_err(|_| invalid("int overflow"))?,
            )),
            ("uint", InputValue::UInt(v)) => Ok(Self::UInt(v)),
            ("uint", InputValue::Int(v)) => Ok(Self::UInt(
                u64::try_from(v).map_err(|_| invalid("negative uint"))?,
            )),
            ("double", InputValue::Double(v)) => Ok(Self::Double(v.into())),
            ("double", InputValue::Int(v)) => Ok(Self::Double((v as f64).into())),
            ("double", InputValue::UInt(v)) => Ok(Self::Double((v as f64).into())),
            ("double", InputValue::String(v)) => Ok(Self::Double(
                match v.as_str() {
                    "NaN" => f64::NAN,
                    "Infinity" => f64::INFINITY,
                    "-Infinity" => f64::NEG_INFINITY,
                    _ => return Err(invalid("invalid double spelling")),
                }
                .into(),
            )),
            ("bytes", InputValue::String(v)) => {
                if !v.len().is_multiple_of(2) || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(invalid("bytes must be hex pairs"));
                }
                let bytes = (0..v.len())
                    .step_by(2)
                    .map(|i| {
                        u8::from_str_radix(&v[i..i + 2], 16).map_err(|_| invalid("invalid byte"))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Self::Bytes(bytes))
            }
            ("string_index", InputValue::Int(v)) => Ok(Self::StringIndex(StringIndex::try_new(
                i32::try_from(v).map_err(|_| invalid("index overflow"))?,
            )?)),
            ("string_index", InputValue::UInt(v)) => Ok(Self::StringIndex(StringIndex::try_new(
                i32::try_from(v).map_err(|_| invalid("index overflow"))?,
            )?)),
            ("array", InputValue::Array(v)) => Ok(Self::Array(
                v.into_iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            )),
            ("kv_list", v) => Ok(Self::KvList(KeyValues::try_from(v)?)),
            _ => Err(invalid("tag does not match value")),
        }
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
                || !val
                    .bytes()
                    .all(|b| (0x20..=0x7e).contains(&b) && b != b',' && b != b'=')
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
