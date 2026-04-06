use std::borrow::Cow;
use std::fmt;
use std::ops::{Add, Sub};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

/// Canonical millisecond duration type used across the workspace.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DurationMs(u64);

impl DurationMs {
    /// Returns the raw millisecond count.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for DurationMs {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DurationMs> for u64 {
    fn from(value: DurationMs) -> Self {
        value.0
    }
}

impl fmt::Display for DurationMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// Canonical UTC timestamp type used across the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    /// Canonical Unix epoch timestamp in UTC.
    pub const UNIX_EPOCH: Self = Self(OffsetDateTime::UNIX_EPOCH);

    /// Returns the current UTC timestamp.
    pub fn now_utc() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    /// Normalizes an arbitrary offset date-time into the canonical UTC timestamp.
    pub fn from_offset_date_time(value: OffsetDateTime) -> Self {
        Self(value.to_offset(UtcOffset::UTC))
    }

    /// Returns the normalized inner UTC date-time value.
    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self::from_offset_date_time(value)
    }
}

impl From<Timestamp> for OffsetDateTime {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self::from_offset_date_time(self.0 + rhs)
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self::from_offset_date_time(self.0 - rhs)
    }
}

impl Sub for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered = self
            .0
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| fmt::Error)?;
        f.write_str(&rendered)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let rendered = self
            .0
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&rendered)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let parsed = OffsetDateTime::parse(&value, &Rfc3339).map_err(serde::de::Error::custom)?;
        Ok(Self::from_offset_date_time(parsed))
    }
}

/// Stable machine-readable error code used across diagnostics and error types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorCode(Cow<'static, str>);

impl ErrorCode {
    /// Creates an error code from a `'static` string without allocating.
    pub const fn new_static(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// Creates an error code from owned or borrowed string data by taking ownership.
    pub fn new_owned(code: impl Into<String>) -> Self {
        Self(Cow::Owned(code.into()))
    }

    /// Returns the string representation of the error code.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{OffsetDateTime, UtcOffset};

    #[test]
    fn timestamp_serde_round_trips_as_utc_rfc3339() {
        let timestamp = Timestamp::from(
            OffsetDateTime::UNIX_EPOCH.to_offset(UtcOffset::from_hms(2, 0, 0).expect("offset")),
        );
        let encoded = serde_json::to_string(&timestamp).expect("serialize timestamp");
        let decoded: Timestamp = serde_json::from_str(&encoded).expect("deserialize timestamp");

        assert_eq!(encoded, "\"1970-01-01T00:00:00Z\"");
        assert_eq!(decoded, timestamp);
    }

    #[test]
    fn duration_ms_displays_in_milliseconds() {
        assert_eq!(DurationMs::from(250).to_string(), "250ms");
    }

    #[test]
    fn duration_ms_exposes_raw_millisecond_count() {
        assert_eq!(DurationMs::from(250).as_u64(), 250);
    }

    #[test]
    fn timestamp_arithmetic_preserves_utc_normalization() {
        let start = Timestamp::UNIX_EPOCH;
        let shifted = start + Duration::seconds(90);
        let rewound = shifted - Duration::seconds(30);

        assert_eq!(u64::from(DurationMs::from(250)), 250);
        assert_eq!(shifted - start, Duration::seconds(90));
        assert_eq!(rewound, start + Duration::seconds(60));
    }

    #[test]
    fn timestamp_into_inner_and_from_timestamp_preserve_utc_value() {
        let timestamp = Timestamp::from(
            OffsetDateTime::UNIX_EPOCH.to_offset(UtcOffset::from_hms(-7, 0, 0).expect("offset")),
        );

        let inner = timestamp.into_inner();
        let round_trip = OffsetDateTime::from(timestamp);

        assert_eq!(inner.offset(), UtcOffset::UTC);
        assert_eq!(round_trip, inner);
    }

    #[test]
    fn error_code_displays_as_plain_code() {
        assert_eq!(
            ErrorCode::new_static("SC_TEST_ERROR_CODE").to_string(),
            "SC_TEST_ERROR_CODE"
        );
    }

    #[test]
    fn error_code_new_owned_preserves_owned_value() {
        let code = ErrorCode::new_owned("SC_OWNED_CODE");
        assert_eq!(code.as_str(), "SC_OWNED_CODE");
    }
}
