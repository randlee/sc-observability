//! Shared cross-crate constants owned by `sc-observability-types`.

use std::sync::LazyLock;

use crate::SchemaVersion;

/// Current version string for the observation envelope contract.
pub const OBSERVATION_ENVELOPE_VERSION: &str = "v1";
/// Validated schema version used by newly-created observation envelopes.
///
/// This is a static because `SchemaVersion` owns a `String` and validates its
/// input, so it cannot be constructed in a `const` initializer.
pub static OBSERVATION_SCHEMA_VERSION: LazyLock<SchemaVersion> = LazyLock::new(|| {
    SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)
        .expect("shared schema version constant is valid")
});
/// Required character length for W3C trace identifiers.
pub const TRACE_ID_LEN: usize = 32;
/// Required character length for W3C span identifiers.
pub const SPAN_ID_LEN: usize = 16;
/// Separator used when deriving environment prefixes.
pub const DEFAULT_ENV_PREFIX_SEPARATOR: char = '_';

/// W3C sampled bit in the trace flags byte.
pub(crate) const TRACE_FLAG_SAMPLED: u8 = 0x01;

/// Maximum W3C tracestate byte length.
pub const TRACE_STATE_MAX_BYTES: usize = 512;
/// Maximum W3C tracestate list members.
pub const TRACE_STATE_MAX_MEMBERS: usize = 32;
/// Maximum tracestate key or value byte length.
pub const TRACE_STATE_MEMBER_MAX_BYTES: usize = 256;
/// Maximum multi-tenant tracestate tenant length.
pub const TRACE_STATE_TENANT_MAX_BYTES: usize = 241;
/// Maximum multi-tenant tracestate system length.
pub const TRACE_STATE_SYSTEM_MAX_BYTES: usize = 14;

/// Maximum OTLP log severity number.
pub const OTLP_SEVERITY_MAX: u8 = 24;
/// Minimum exponential histogram scale.
pub const OTLP_EXPONENTIAL_SCALE_MIN: i32 = -10;
/// Maximum exponential histogram scale.
pub const OTLP_EXPONENTIAL_SCALE_MAX: i32 = 20;

/// Maximum caller record-key length in UTF-8 bytes.
pub const TELEMETRY_RECORD_KEY_MAX_BYTES: usize = 256;
/// Default producer service when no source supplies one.
pub const TELEMETRY_DEFAULT_SERVICE: &str = "unknown_service";
/// Default OTLP/HTTP base endpoint.
pub const TELEMETRY_DEFAULT_ENDPOINT: &str = "http://localhost:4318";
/// Default per-request export deadline.
pub const TELEMETRY_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Default durable-store byte bound (256 MiB).
pub const TELEMETRY_MAX_STORE_BYTES: u64 = 256 * 1024 * 1024;
/// Seconds per hour for checked file-config conversion.
pub const TELEMETRY_SECONDS_PER_HOUR: u64 = 3600;
/// Default delivered-row retention.
pub const TELEMETRY_DELIVERED_RETENTION: std::time::Duration =
    std::time::Duration::from_secs(24 * 3600);
/// Default local deduplication-key retention.
pub const TELEMETRY_RECORD_KEY_RETENTION: std::time::Duration =
    std::time::Duration::from_secs(30 * 24 * 3600);
/// Default emit-and-flush deadline.
pub const TELEMETRY_EMIT_FLUSH_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
/// Default explicit flush deadline.
pub const TELEMETRY_FLUSH_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
/// Default drain lease duration.
pub const TELEMETRY_LEASE_DURATION: std::time::Duration = std::time::Duration::from_secs(30);

/// OTLP severity number for `Level::Trace` (`SEVERITY_NUMBER_TRACE`).
pub(crate) const OTLP_SEVERITY_TRACE: u8 = 1;
/// OTLP severity number for `Level::Debug` (`SEVERITY_NUMBER_DEBUG`).
pub(crate) const OTLP_SEVERITY_DEBUG: u8 = 5;
/// OTLP severity number for `Level::Info` (`SEVERITY_NUMBER_INFO`).
pub(crate) const OTLP_SEVERITY_INFO: u8 = 9;
/// OTLP severity number for `Level::Warn` (`SEVERITY_NUMBER_WARN`).
pub(crate) const OTLP_SEVERITY_WARN: u8 = 13;
/// OTLP severity number for `Level::Error` (`SEVERITY_NUMBER_ERROR`).
pub(crate) const OTLP_SEVERITY_ERROR: u8 = 17;

/// Lowest byte allowed in a W3C tracestate value (`chr(0x20)`).
pub(crate) const TRACE_STATE_VALUE_BYTE_MIN: u8 = 0x20;
/// Highest byte allowed in a W3C tracestate value (`chr(0x7E)`).
pub(crate) const TRACE_STATE_VALUE_BYTE_MAX: u8 = 0x7e;

/// Text length of a hyphenated UUID (`8-4-4-4-12`).
pub(crate) const UUID_TEXT_LEN: usize = 36;
/// Byte offsets of the four hyphens in a hyphenated UUID.
pub(crate) const UUID_HYPHEN_OFFSETS: [usize; 4] = [8, 13, 18, 23];
/// Byte offset of the version nibble in a hyphenated UUID.
pub(crate) const UUID_VERSION_OFFSET: usize = 14;
/// Version nibble of a `UUIDv7`.
pub(crate) const UUID_VERSION_7: u8 = b'7';
/// Byte offset of the variant nibble in a hyphenated UUID.
pub(crate) const UUID_VARIANT_OFFSET: usize = 19;
/// RFC 9562 variant nibbles (`10xx`), in either letter case.
pub(crate) const UUID_RFC9562_VARIANTS: &[u8] = b"89abAB";

/// Maximum caller JSON accepted by `SubmissionEnvelope::from_json` (16 MiB).
pub(crate) const SUBMISSION_MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum records in one signal family of one submission.
pub(crate) const SUBMISSION_MAX_RECORDS_PER_SIGNAL: usize = 10_000;
/// Maximum entries in one attribute collection, value array, event list or link list.
pub(crate) const SUBMISSION_MAX_COLLECTION_ENTRIES: usize = 1_024;
/// Maximum nesting depth of one `AnyValue` (arrays and key/value lists).
pub(crate) const ANY_VALUE_MAX_DEPTH: usize = 32;
/// Maximum UTF-8 bytes in one string, key or bytes value (1 MiB).
pub(crate) const SUBMISSION_MAX_STRING_BYTES: usize = 1024 * 1024;

/// Smallest accepted client duration; the file format is millisecond-grained.
pub(crate) const TELEMETRY_DURATION_MIN: std::time::Duration = std::time::Duration::from_millis(1);
/// Largest accepted per-request export timeout.
pub(crate) const TELEMETRY_REQUEST_TIMEOUT_MAX: std::time::Duration =
    std::time::Duration::from_secs(10 * 60);
/// Largest accepted flush deadline, emit-flush deadline or lease duration.
pub(crate) const TELEMETRY_DEADLINE_MAX: std::time::Duration =
    std::time::Duration::from_secs(24 * 3600);
/// Largest accepted delivered-row or record-key retention.
pub(crate) const TELEMETRY_RETENTION_MAX: std::time::Duration =
    std::time::Duration::from_secs(366 * 24 * 3600);
/// Largest accepted durable-store byte bound (1 TiB).
pub(crate) const TELEMETRY_MAX_STORE_BYTES_LIMIT: u64 = 1024 * 1024 * 1024 * 1024;
/// Largest accepted retry delay or retry budget, in milliseconds (24 hours).
pub(crate) const TELEMETRY_RETRY_DURATION_MAX_MS: u64 = 24 * 3600 * 1000;
/// Maximum synchronous HTTP retry jitter percentage.
pub(crate) const TELEMETRY_RETRY_JITTER_PERCENT_MAX: u8 = 100;

#[cfg(test)]
mod tests {
    use super::{OBSERVATION_ENVELOPE_VERSION, OBSERVATION_SCHEMA_VERSION};
    use crate::SchemaVersion;

    #[test]
    fn observation_schema_version_matches_the_envelope_version() {
        assert_eq!(
            *OBSERVATION_SCHEMA_VERSION,
            SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid schema version")
        );
    }
}
