//! Profiles v1development payloads and checked dictionary references.
use super::{AnyValue, AttributeKey, KeyValues, ResourceRecord, SignalValidationError};
use crate::Timestamp;
use serde::{Deserialize, Serialize};

/// Neutral profiles `ProfilesDictionary` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesDictionary {
    /// Protocol `mapping_table` field.
    pub mapping_table: Vec<Mapping>,
    /// Protocol `location_table` field.
    pub location_table: Vec<Location>,
    /// Protocol `function_table` field.
    pub function_table: Vec<Function>,
    /// Protocol `link_table` field.
    pub link_table: Vec<ProfileLink>,
    /// Protocol `string_table` field.
    pub string_table: Vec<String>,
    /// Protocol `attribute_table` field.
    pub attribute_table: Vec<KeyValueAndUnit>,
    /// Protocol `stack_table` field.
    pub stack_table: Vec<Stack>,
}
impl ProfilesDictionary {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(
        mapping_table: Vec<Mapping>,
        location_table: Vec<Location>,
        function_table: Vec<Function>,
        link_table: Vec<ProfileLink>,
        string_table: Vec<String>,
        attribute_table: Vec<KeyValueAndUnit>,
        stack_table: Vec<Stack>,
    ) -> Self {
        Self {
            mapping_table,
            location_table,
            function_table,
            link_table,
            string_table,
            attribute_table,
            stack_table,
        }
    }
}

/// Neutral profiles `Profile` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Protocol `sample_type` field.
    pub sample_type: Option<ValueType>,
    /// Protocol `samples` field.
    pub samples: Vec<Sample>,
    /// Protocol `time` field.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub time: Timestamp,
    /// Protocol `duration_nanos` field.
    pub duration_nanos: u64,
    /// Protocol `period_type` field.
    pub period_type: Option<ValueType>,
    /// Protocol `period` field.
    pub period: i64,
    /// Protocol `profile_id` field.
    #[serde(with = "hex_id")]
    pub profile_id: [u8; 16],
    /// Protocol `dropped_attributes_count` field.
    pub dropped_attributes_count: u32,
    /// Protocol `original_payload_format` field.
    pub original_payload_format: Option<String>,
    /// Protocol `original_payload` field.
    pub original_payload: Vec<u8>,
    /// Protocol `attribute_indices` field.
    pub attribute_indices: Vec<i32>,
}
impl Profile {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors the complete neutral profile record"
    )]
    #[must_use]
    pub fn new(
        sample_type: Option<ValueType>,
        samples: Vec<Sample>,
        time: Timestamp,
        duration_nanos: u64,
        period_type: Option<ValueType>,
        period: i64,
        profile_id: [u8; 16],
        dropped_attributes_count: u32,
        original_payload_format: Option<String>,
        original_payload: Vec<u8>,
        attribute_indices: Vec<i32>,
    ) -> Self {
        Self {
            sample_type,
            samples,
            time,
            duration_nanos,
            period_type,
            period,
            profile_id,
            dropped_attributes_count,
            original_payload_format,
            original_payload,
            attribute_indices,
        }
    }
}

/// Neutral profiles `ProfileLink` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProfileLink {
    /// Protocol `trace_id` field.
    #[serde(with = "hex_id")]
    pub trace_id: [u8; 16],
    /// Protocol `span_id` field.
    #[serde(with = "hex_id")]
    pub span_id: [u8; 8],
}
impl ProfileLink {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(trace_id: [u8; 16], span_id: [u8; 8]) -> Self {
        Self { trace_id, span_id }
    }
}

/// Neutral profiles `ValueType` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ValueType {
    /// Protocol `type_strindex` field.
    pub type_strindex: i32,
    /// Protocol `unit_strindex` field.
    pub unit_strindex: i32,
}
impl ValueType {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(type_strindex: i32, unit_strindex: i32) -> Self {
        Self {
            type_strindex,
            unit_strindex,
        }
    }
}

/// Neutral profiles `Sample` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Protocol `stack_index` field.
    pub stack_index: i32,
    /// Protocol `attribute_indices` field.
    pub attribute_indices: Vec<i32>,
    /// Protocol `link_index` field.
    pub link_index: i32,
    /// Protocol `values` field.
    pub values: Vec<i64>,
    /// Protocol `timestamps_unix_nano` field.
    pub timestamps_unix_nano: Vec<u64>,
}
impl Sample {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(
        stack_index: i32,
        attribute_indices: Vec<i32>,
        link_index: i32,
        values: Vec<i64>,
        timestamps_unix_nano: Vec<u64>,
    ) -> Self {
        Self {
            stack_index,
            attribute_indices,
            link_index,
            values,
            timestamps_unix_nano,
        }
    }
}

/// Neutral profiles `Mapping` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    /// Protocol `memory_start` field.
    pub memory_start: u64,
    /// Protocol `memory_limit` field.
    pub memory_limit: u64,
    /// Protocol `file_offset` field.
    pub file_offset: u64,
    /// Protocol `filename_strindex` field.
    pub filename_strindex: i32,
    /// Protocol `attribute_indices` field.
    pub attribute_indices: Vec<i32>,
}
impl Mapping {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(
        memory_start: u64,
        memory_limit: u64,
        file_offset: u64,
        filename_strindex: i32,
        attribute_indices: Vec<i32>,
    ) -> Self {
        Self {
            memory_start,
            memory_limit,
            file_offset,
            filename_strindex,
            attribute_indices,
        }
    }
}

/// Neutral profiles `Stack` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// Protocol `location_indices` field.
    pub location_indices: Vec<i32>,
}
impl Stack {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(location_indices: Vec<i32>) -> Self {
        Self { location_indices }
    }
}

/// Neutral profiles `Location` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// Protocol `mapping_index` field.
    pub mapping_index: i32,
    /// Protocol `address` field.
    pub address: u64,
    /// Protocol `lines` field.
    pub lines: Vec<Line>,
    /// Protocol `attribute_indices` field.
    pub attribute_indices: Vec<i32>,
}
impl Location {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(
        mapping_index: i32,
        address: u64,
        lines: Vec<Line>,
        attribute_indices: Vec<i32>,
    ) -> Self {
        Self {
            mapping_index,
            address,
            lines,
            attribute_indices,
        }
    }
}

/// Neutral profiles `Line` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Protocol `function_index` field.
    pub function_index: i32,
    /// Protocol `line` field.
    pub line: i64,
    /// Protocol `column` field.
    pub column: i64,
}
impl Line {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(function_index: i32, line: i64, column: i64) -> Self {
        Self {
            function_index,
            line,
            column,
        }
    }
}

/// Neutral profiles `Function` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Function {
    /// Protocol `name_strindex` field.
    pub name_strindex: i32,
    /// Protocol `system_name_strindex` field.
    pub system_name_strindex: i32,
    /// Protocol `filename_strindex` field.
    pub filename_strindex: i32,
    /// Protocol `start_line` field.
    pub start_line: i64,
}
impl Function {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(
        name_strindex: i32,
        system_name_strindex: i32,
        filename_strindex: i32,
        start_line: i64,
    ) -> Self {
        Self {
            name_strindex,
            system_name_strindex,
            filename_strindex,
            start_line,
        }
    }
}

/// Neutral profiles `KeyValueAndUnit` record from protocol v1.10.0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct KeyValueAndUnit {
    /// Protocol `key_strindex` field.
    pub key_strindex: i32,
    /// Protocol `value` field.
    pub value: Option<AnyValue>,
    /// Protocol `unit_strindex` field.
    pub unit_strindex: i32,
}
impl KeyValueAndUnit {
    /// Constructs the record; dictionary references are checked with the complete submission.
    #[must_use]
    pub fn new(key_strindex: i32, value: Option<AnyValue>, unit_strindex: i32) -> Self {
        Self {
            key_strindex,
            value,
            unit_strindex,
        }
    }
}

impl Default for ProfilesDictionary {
    fn default() -> Self {
        Self {
            mapping_table: vec![Mapping::default()],
            location_table: vec![Location::default()],
            function_table: vec![Function::default()],
            link_table: vec![ProfileLink::default()],
            string_table: vec![String::new()],
            attribute_table: vec![KeyValueAndUnit::default()],
            stack_table: vec![Stack::default()],
        }
    }
}
impl ProfilesDictionary {
    /// Checks sentinel entries and every dictionary reference, including nested values.
    /// # Errors
    /// Rejects missing zero entries, negative indices, or references beyond their table.
    pub fn validate_references(
        &self,
        profiles: &[ResourceRecord<Profile>],
    ) -> Result<(), SignalValidationError> {
        fn zero<T: Default + PartialEq>(
            table: &[T],
            name: &str,
        ) -> Result<(), SignalValidationError> {
            if table.first() == Some(&T::default()) {
                Ok(())
            } else {
                Err(SignalValidationError::invalid(
                    name,
                    "index zero must contain the zero-value entry",
                ))
            }
        }
        zero(&self.mapping_table, "mapping_table")?;
        zero(&self.location_table, "location_table")?;
        zero(&self.function_table, "function_table")?;
        zero(&self.link_table, "link_table")?;
        zero(&self.string_table, "string_table")?;
        zero(&self.attribute_table, "attribute_table")?;
        zero(&self.stack_table, "stack_table")?;
        for mapping in &self.mapping_table {
            self.string(mapping.filename_strindex)?;
            self.attributes(&mapping.attribute_indices)?;
        }
        for location in &self.location_table {
            index(
                location.mapping_index,
                self.mapping_table.len(),
                "mapping_index",
            )?;
            self.attributes(&location.attribute_indices)?;
            for line in &location.lines {
                index(
                    line.function_index,
                    self.function_table.len(),
                    "function_index",
                )?;
            }
        }
        for function in &self.function_table {
            self.string(function.name_strindex)?;
            self.string(function.system_name_strindex)?;
            self.string(function.filename_strindex)?;
        }
        for a in &self.attribute_table {
            self.string(a.key_strindex)?;
            self.string(a.unit_strindex)?;
            if let Some(v) = &a.value {
                self.value(v)?;
            }
        }
        for stack in &self.stack_table {
            for i in &stack.location_indices {
                index(*i, self.location_table.len(), "location_index")?;
            }
        }
        for record in profiles {
            self.key_values(&record.resource.attributes)?;
            self.key_values(&record.scope.attributes)?;
            let profile = &record.record;
            self.attributes(&profile.attribute_indices)?;
            for t in [profile.sample_type.as_ref(), profile.period_type.as_ref()]
                .into_iter()
                .flatten()
            {
                self.string(t.type_strindex)?;
                self.string(t.unit_strindex)?;
            }
            for sample in &profile.samples {
                index(sample.stack_index, self.stack_table.len(), "stack_index")?;
                index(sample.link_index, self.link_table.len(), "link_index")?;
                self.attributes(&sample.attribute_indices)?;
            }
        }
        Ok(())
    }
    fn string(&self, i: i32) -> Result<(), SignalValidationError> {
        index(i, self.string_table.len(), "string_index")
    }
    fn attributes(&self, indices: &[i32]) -> Result<(), SignalValidationError> {
        for i in indices {
            index(*i, self.attribute_table.len(), "attribute_index")?;
        }
        Ok(())
    }
    fn key_values(&self, values: &KeyValues) -> Result<(), SignalValidationError> {
        for (key, value) in values.entries() {
            if let AttributeKey::Index(i) = key {
                self.string(i.get())?;
            }
            self.value(value)?;
        }
        Ok(())
    }
    fn value(&self, value: &AnyValue) -> Result<(), SignalValidationError> {
        match value {
            AnyValue::StringIndex(i) => self.string(i.get())?,
            AnyValue::Array(values) => {
                for v in values {
                    self.value(v)?;
                }
            }
            AnyValue::KvList(values) => self.key_values(values)?,
            _ => {}
        }
        Ok(())
    }
}
fn index(i: i32, len: usize, path: &str) -> Result<(), SignalValidationError> {
    if usize::try_from(i).is_ok_and(|i| i < len) {
        Ok(())
    } else {
        Err(SignalValidationError::invalid(
            path,
            "dictionary index out of range",
        ))
    }
}

mod hex_id {
    use serde::{Deserialize, Deserializer, Serializer, de, ser};
    pub fn serialize<S: Serializer, const N: usize>(
        value: &[u8; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        use std::fmt::Write;
        let mut text = String::with_capacity(N * 2);
        for byte in value {
            write!(text, "{byte:02x}").map_err(ser::Error::custom)?;
        }
        s.serialize_str(&text)
    }
    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[u8; N], D::Error> {
        let text = String::deserialize(d)?;
        if text.len() != N * 2 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(de::Error::custom("invalid hexadecimal identifier"));
        }
        let mut bytes = [0; N];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(de::Error::custom)?;
        }
        Ok(bytes)
    }
}
