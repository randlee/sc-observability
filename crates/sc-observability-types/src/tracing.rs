use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ValueValidationError, error_codes};

/// Validated 32-character lowercase hexadecimal trace identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    /// Creates a validated lowercase hexadecimal trace identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            crate::constants::TRACE_ID_LEN,
            &error_codes::TRACE_ID_INVALID,
        )?;
        Ok(Self(value))
    }

    /// Returns the underlying lowercase hexadecimal trace identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated 16-character lowercase hexadecimal span identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(String);

impl SpanId {
    /// Creates a validated lowercase hexadecimal span identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
        let value = value.into();
        validate_lower_hex(
            &value,
            crate::constants::SPAN_ID_LEN,
            &error_codes::SPAN_ID_INVALID,
        )?;
        Ok(Self(value))
    }

    /// Returns the underlying lowercase hexadecimal span identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn validate_lower_hex(
    value: &str,
    expected_len: usize,
    code: &ErrorCode,
) -> Result<(), ValueValidationError> {
    if value.len() != expected_len {
        return Err(ValueValidationError::with_code(
            code.clone(),
            format!("value must be {expected_len} lowercase hex characters"),
        ));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ValueValidationError::with_code(
            code.clone(),
            "value must contain lowercase hex characters only",
        ))
    }
}

/// Generic trace correlation context shared by logs, spans, and observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    /// W3C-compatible trace identifier.
    pub trace_id: TraceId,
    /// Current span identifier.
    pub span_id: SpanId,
    /// Optional parent span identifier.
    pub parent_span_id: Option<SpanId>,
}

/// Typed description of an entity moving from one state to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransition {
    /// Stable category describing what changed, such as `task` or `subagent`.
    pub entity_kind: String,
    /// Optional caller-owned identifier for the entity that changed.
    pub entity_id: Option<String>,
    /// Previous stable state label.
    pub from_state: String,
    /// New stable state label.
    pub to_state: String,
    /// Optional human-readable explanation for why the transition occurred.
    pub reason: Option<String>,
    /// Optional action or event name that triggered the transition.
    pub trigger: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_and_span_ids_validate_w3c_shapes() {
        assert!(TraceId::new("0123456789abcdef0123456789abcdef").is_ok());
        let short_trace = TraceId::new("0123456789abcdef0123456789abcde")
            .expect_err("short trace id should fail");
        assert_eq!(short_trace.code(), &error_codes::TRACE_ID_INVALID);
        let uppercase_trace = TraceId::new("0123456789ABCDEF0123456789abcdef")
            .expect_err("uppercase trace id should fail");
        assert_eq!(uppercase_trace.code(), &error_codes::TRACE_ID_INVALID);

        assert!(SpanId::new("0123456789abcdef").is_ok());
        let short_span = SpanId::new("0123456789abcde").expect_err("short span id should fail");
        assert_eq!(short_span.code(), &error_codes::SPAN_ID_INVALID);
        let uppercase_span =
            SpanId::new("0123456789ABCDEf").expect_err("uppercase span id should fail");
        assert_eq!(uppercase_span.code(), &error_codes::SPAN_ID_INVALID);
    }
}
