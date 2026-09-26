//! Explicit wire shapes; input entrypoints apply checked semantic validation.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Stable validation failures returned by [`DecimalDto`] constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalDtoError {
    /// The value is not in canonical decimal spelling.
    InvalidCanonical,
    /// A negative value is outside the signed 64-bit domain.
    SignedOverflow,
    /// A non-negative value is outside the unsigned 64-bit domain.
    UnsignedOverflow,
    /// The canonical value is negative and cannot represent a counter.
    NotUnsigned,
}

impl DecimalDtoError {
    /// Stable machine-readable validation code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidCanonical => "SC_OBSERVABILITY_DTO_DECIMAL_INVALID_CANONICAL",
            Self::SignedOverflow => "SC_OBSERVABILITY_DTO_DECIMAL_SIGNED_OVERFLOW",
            Self::UnsignedOverflow => "SC_OBSERVABILITY_DTO_DECIMAL_UNSIGNED_OVERFLOW",
            Self::NotUnsigned => "SC_OBSERVABILITY_DTO_DECIMAL_NOT_UNSIGNED",
        }
    }
}

impl fmt::Display for DecimalDtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCanonical => "expected canonical decimal integer",
            Self::SignedOverflow => "signed integer overflow",
            Self::UnsignedOverflow => "unsigned integer overflow",
            Self::NotUnsigned => "expected unsigned decimal",
        })
    }
}

impl std::error::Error for DecimalDtoError {}

/// Canonical integer string: signed i64 or unsigned u64; counters additionally reject negatives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct DecimalDto(
    #[cfg_attr(
        feature = "schema-gen",
        schemars(regex(pattern = r"^(0|[1-9][0-9]*|-[1-9][0-9]*)(?![\s\S])"))
    )]
    String,
);
impl DecimalDto {
    /// Checks canonical spelling and the shared JSON integer domain.
    pub fn new(value: impl Into<String>) -> std::result::Result<Self, DecimalDtoError> {
        let value = value.into();
        let digits = value.strip_prefix('-').unwrap_or(&value);
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
            || value == "-0"
        {
            return Err(DecimalDtoError::InvalidCanonical);
        }
        if value.starts_with('-') {
            value
                .parse::<i64>()
                .map_err(|_| DecimalDtoError::SignedOverflow)?;
        } else {
            value
                .parse::<u64>()
                .map_err(|_| DecimalDtoError::UnsignedOverflow)?;
        }
        Ok(Self(value))
    }
    /// Returns the canonical representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Checks the unsigned counter domain.
    pub fn as_u64(&self) -> std::result::Result<u64, DecimalDtoError> {
        self.0.parse().map_err(|_| DecimalDtoError::NotUnsigned)
    }
}
impl From<u64> for DecimalDto {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}
impl From<i64> for DecimalDto {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}
impl<'de> Deserialize<'de> for DecimalDto {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Validates a decimal wire value belongs to the unsigned counter domain.
pub(crate) fn unsigned_decimal<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<DecimalDto, D::Error> {
    let value = DecimalDto::deserialize(d)?;
    value.as_u64().map_err(serde::de::Error::custom)?;
    Ok(value)
}

/// Version-one LevelDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelDto {
    /// Wire trace.
    Trace,
    /// Wire debug.
    Debug,
    /// Wire info.
    Info,
    /// Wire warn.
    Warn,
    /// Wire error.
    Error,
}

/// Version-one LevelFilterDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelFilterDto {
    /// Wire off.
    Off,
    /// Wire error.
    Error,
    /// Wire warn.
    Warn,
    /// Wire info.
    Info,
    /// Wire debug.
    Debug,
    /// Wire trace.
    Trace,
}

/// Version-one LevelChangeSourceDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelChangeSourceDto {
    /// Wire application.
    Application,
    /// Wire user request.
    UserRequest,
    /// Wire diagnostic session.
    DiagnosticSession,
}

/// Version-one AvailabilityDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityDto {
    /// Wire healthy.
    Healthy,
    /// Wire degraded dropping.
    DegradedDropping,
    /// Wire unavailable.
    Unavailable,
}

/// Version-one WorkerStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkerStateDto {
    /// Wire running.
    Running,
    /// Wire degraded.
    Degraded,
    /// Wire stopped.
    Stopped,
}

/// Version-one QueryStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum QueryStateDto {
    /// Wire healthy.
    Healthy,
    /// Wire degraded.
    Degraded,
    /// Wire unavailable.
    Unavailable,
}

/// Version-one LifecycleDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDto {
    /// Wire running.
    Running,
    /// Wire stopping.
    Stopping,
    /// Wire stopped.
    Stopped,
    /// Wire failed.
    Failed,
}

/// Version-one LogOrderDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LogOrderDto {
    #[default]
    /// Wire oldest first.
    OldestFirst,
    /// Wire newest first.
    NewestFirst,
}

pub(crate) fn default_limit() -> usize {
    crate::constants::DEFAULT_QUERY_LIMIT
}
/// Version-one PathDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PathDto {
    /// Wire utf8.
    Utf8 {
        /// Active variant value.
        value: String,
    },
    /// Wire unrepresentable.
    Unrepresentable,
    /// Wire absent.
    Absent,
}

/// Version-one ValueDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-input-strict" = true)))]
pub enum ValueDto {
    /// Wire null.
    Null {},
    /// Wire boolean.
    Boolean {
        /// Active variant value.
        value: bool,
    },
    /// Wire string.
    String {
        /// Active variant value.
        value: String,
    },
    /// Wire integer.
    Integer {
        /// Active variant value.
        value: DecimalDto,
    },
    /// Wire float.
    Float {
        /// Active variant value.
        value: f64,
    },
    /// Wire array.
    Array {
        /// Active variant value.
        value: Vec<ValueDto>,
    },
    /// Wire object.
    Object {
        /// Active variant value.
        value: BTreeMap<String, ValueDto>,
    },
}

/// Version-one RemediationDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationDto {
    /// Wire recoverable.
    Recoverable {
        /// Active variant steps.
        steps: Vec<String>,
    },
    /// Wire not recoverable.
    NotRecoverable {
        /// Active variant justification.
        justification: String,
    },
}

/// Version-one TraceContextDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-input-strict" = true)))]
pub struct TraceContextDto {
    /// trace id.
    pub trace_id: String,
    /// span id.
    pub span_id: String,
    /// parent span id.
    pub parent_span_id: Option<String>,
}

/// Version-one ProcessIdentityDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct ProcessIdentityDto {
    /// hostname.
    pub hostname: Option<String>,
    /// pid.
    pub pid: Option<u32>,
}

/// Version-one StateTransitionDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StateTransitionDto {
    /// entity kind.
    pub entity_kind: String,
    /// entity id.
    pub entity_id: Option<String>,
    /// from state.
    pub from_state: String,
    /// to state.
    pub to_state: String,
    /// reason.
    pub reason: Option<String>,
    /// trigger.
    pub trigger: Option<String>,
}

/// Version-one StoredDiagnosticDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StoredDiagnosticDto {
    /// timestamp.
    pub timestamp: String,
    /// code.
    pub code: String,
    /// message.
    pub message: String,
    /// cause.
    pub cause: Option<String>,
    /// remediation.
    pub remediation: RemediationDto,
    /// docs.
    pub docs: Option<String>,
    /// details.
    pub details: BTreeMap<String, ValueDto>,
}
