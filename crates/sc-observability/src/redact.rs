use serde_json::Value;

use crate::constants;

pub(crate) fn redact_string_value(value: &mut Value) {
    if let Value::String(text) = value {
        *text = redact_bearer_token_text(text);
    }
}

pub(crate) fn redact_bearer_token_text(input: &str) -> String {
    const PREFIX: &str = "Bearer ";
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(PREFIX) {
        result.push_str(&remaining[..index + PREFIX.len()]);
        let token_start = index + PREFIX.len();
        let token_end = remaining[token_start..]
            .find(char::is_whitespace)
            .map_or(remaining.len(), |value| token_start + value);
        result.push_str(constants::REDACTED_VALUE);
        remaining = &remaining[token_end..];
    }

    result.push_str(remaining);
    result
}
