use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ErrorCode, constants, error_codes};

/// Validation error returned when a public value type rejects an input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("{message}")]
pub struct ValueValidationError {
    code: ErrorCode,
    message: String,
}

impl ValueValidationError {
    /// Creates a validation error using the default shared validation code.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: error_codes::VALUE_VALIDATION_FAILED,
            message: message.into(),
        }
    }

    /// Creates a validation error with an explicit stable error code.
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Returns the stable error code associated with the validation failure.
    pub fn code(&self) -> &ErrorCode {
        &self.code
    }
}

macro_rules! validated_name_type {
    ($name:ident, $doc:literal, $validator:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Creates a validated value from caller-provided string data.
            pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError> {
                let value = value.into();
                $validator(&value)?;
                Ok(Self(value))
            }

            /// Returns the underlying validated string value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

pub(crate) fn validate_identifier(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("identifier must not be empty"));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "identifier must match [A-Za-z0-9._-]+",
        ))
    }
}

pub(crate) fn validate_env_prefix(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("env prefix must not be empty"));
    }
    if value.ends_with(constants::DEFAULT_ENV_PREFIX_SEPARATOR) {
        return Err(ValueValidationError::new(
            "env prefix must not end with underscore",
        ));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "env prefix must match [A-Z0-9_]+",
        ))
    }
}

pub(crate) fn validate_metric_name(value: &str) -> Result<(), ValueValidationError> {
    if value.is_empty() {
        return Err(ValueValidationError::new("metric name must not be empty"));
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
    {
        Ok(())
    } else {
        Err(ValueValidationError::new(
            "metric name must match [A-Za-z0-9._\\-/]+",
        ))
    }
}

validated_name_type!(
    ToolName,
    "Validated tool identity used for top-level configuration.",
    validate_identifier
);
validated_name_type!(
    EnvPrefix,
    "Validated environment prefix used for config loading namespaces.",
    validate_env_prefix
);
validated_name_type!(
    ServiceName,
    "Validated service name carried in logs and telemetry.",
    validate_identifier
);
validated_name_type!(
    TargetCategory,
    "Validated stable target category for log events.",
    validate_identifier
);
validated_name_type!(
    ActionName,
    "Validated stable action name for log and span events.",
    validate_identifier
);
validated_name_type!(
    MetricName,
    "Validated metric identity using [A-Za-z0-9._\\-/]+.",
    validate_metric_name
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validated_name_newtypes_accept_expected_values() {
        assert_eq!(
            ToolName::new("codex-cli")
                .expect("valid tool name")
                .as_str(),
            "codex-cli"
        );
        assert_eq!(
            ToolName::new("codex-cli")
                .expect("valid tool name")
                .to_string(),
            "codex-cli"
        );
        assert_eq!(
            EnvPrefix::new("SC_OBSERVABILITY")
                .expect("valid env prefix")
                .as_str(),
            "SC_OBSERVABILITY"
        );
        assert_eq!(
            ServiceName::new("service.core")
                .expect("valid service name")
                .as_str(),
            "service.core"
        );
        assert_eq!(
            TargetCategory::new("pipeline-ingest")
                .expect("valid target category")
                .as_str(),
            "pipeline-ingest"
        );
        assert_eq!(
            ActionName::new("observation.received")
                .expect("valid action name")
                .as_str(),
            "observation.received"
        );
        assert_eq!(
            MetricName::new("obs/events_total")
                .expect("valid metric name")
                .as_str(),
            "obs/events_total"
        );
    }

    #[test]
    fn validated_name_newtypes_reject_invalid_values() {
        assert!(ToolName::new("").is_err());
        assert!(EnvPrefix::new("sc_observability").is_err());
        assert!(EnvPrefix::new("SC_OBSERVABILITY_").is_err());
        assert!(ServiceName::new("service core").is_err());
        assert!(TargetCategory::new("category/invalid").is_err());
        assert!(ActionName::new("action invalid").is_err());
        assert!(MetricName::new("metric name").is_err());
    }
}
