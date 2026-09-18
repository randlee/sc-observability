use serde_json::Value;
use std::borrow::Cow;

use crate::constants;

pub(crate) fn redact_string_value(value: &mut Value) {
    if let Value::String(text) = value
        && let Cow::Owned(redacted) = redact_bearer_token_text(text)
    {
        *text = redacted;
    }
}

pub(crate) fn redact_bearer_token_text(input: &str) -> Cow<'_, str> {
    const PREFIX: &str = "Bearer ";
    if !input.contains(PREFIX) {
        return Cow::Borrowed(input);
    }
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
    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::{redact_bearer_token_text, redact_string_value};

    #[test]
    fn no_match_borrows_without_allocating() {
        let input = "ordinary message";
        assert!(matches!(
            redact_bearer_token_text(input),
            std::borrow::Cow::Borrowed(_)
        ));
        let mut value = serde_json::Value::String(input.into());
        redact_string_value(&mut value);
        assert_eq!(value, serde_json::Value::String(input.into()));
    }

    #[test]
    fn match_redacts_and_allocates() {
        let redacted = redact_bearer_token_text("Bearer secret");
        assert!(matches!(redacted, std::borrow::Cow::Owned(_)));
        assert_eq!(redacted, "Bearer [REDACTED]");
    }
}
