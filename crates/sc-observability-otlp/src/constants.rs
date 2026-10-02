//! Crate-local constants for `sc-observability-otlp`.

/// Default OTLP request timeout in milliseconds.
pub const DEFAULT_OTLP_TIMEOUT_MS: u64 = 3_000;
/// Default time allowed for a backend flush barrier.
pub const DEFAULT_OTLP_LIFECYCLE_FLUSH_TIMEOUT_MS: u64 = 30_000;
/// Default time allowed for an orderly backend shutdown.
pub const DEFAULT_OTLP_LIFECYCLE_SHUTDOWN_TIMEOUT_MS: u64 = 30_000;
/// Default number of records admitted across queued and in-flight batches.
pub const DEFAULT_OTLP_QUEUE_CAPACITY: usize = 1_024;
/// Hard upper bound for admitted records.
pub const MAX_OTLP_QUEUE_CAPACITY: usize = 65_536;
/// Default aggregate payload-byte admission budget (16 MiB).
pub const DEFAULT_OTLP_QUEUE_BYTE_CAPACITY: usize = 16 * 1024 * 1024;
/// Hard upper bound for the aggregate payload-byte admission budget (64 MiB).
pub const MAX_OTLP_QUEUE_BYTE_CAPACITY: usize = 64 * 1024 * 1024;
/// Largest single record accepted by the contract (1 MiB).
pub const MAX_OTLP_RECORD_BYTES: usize = 1024 * 1024;
/// Maximum live V1 span lifecycles retained before deterministic eviction.
pub const MAX_OTLP_LIVE_SPANS: usize = 1_024;
/// Maximum events retained for one live V1 span lifecycle.
pub const MAX_OTLP_EVENTS_PER_SPAN: usize = 256;
/// Maximum records in one backend batch.
pub const MAX_OTLP_BATCH_RECORDS: usize = 512;
/// Maximum serialized bytes in one backend batch (1 MiB).
pub const MAX_OTLP_BATCH_BYTES: usize = 1024 * 1024;
/// Maximum JSON bytes submitted in one synchronous OTLP/HTTP request (1 MiB).
///
/// This bound is applied after OTLP/JSON encoding rather than to the source
/// envelope so that transport requests remain bounded even when JSON expands
/// IDs, base64 values, or escaped strings.
#[cfg(feature = "sync-http")]
pub(crate) const MAX_OTLP_ENCODED_REQUEST_BYTES: usize = 1024 * 1024;
/// Default maximum number of OTLP export retries.
pub const DEFAULT_OTLP_MAX_RETRIES: u32 = 3;
/// Default initial OTLP retry backoff in milliseconds.
pub const DEFAULT_OTLP_INITIAL_BACKOFF_MS: u64 = 250;
/// Default maximum OTLP retry backoff in milliseconds.
pub const DEFAULT_OTLP_MAX_BACKOFF_MS: u64 = 5_000;
/// Default total deadline for one synchronous HTTP retry sequence.
pub const DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS: u64 = 30_000;
/// Minimum lifecycle and retry budget retained by released OTLP compatibility.
pub(crate) const RELEASED_OTLP_BUDGET_FLOOR_MS: u64 = 30_000;
/// Default upper bound for a synchronous HTTP Retry-After value.
pub const DEFAULT_OTLP_RETRY_AFTER_CAP_MS: u64 = 5_000;
/// Default synchronous HTTP retry jitter percentage.
pub const DEFAULT_OTLP_RETRY_JITTER_PERCENT: u8 = 20;
/// Maximum synchronous HTTP retry jitter percentage.
pub const MAX_OTLP_RETRY_JITTER_PERCENT: u8 = 100;
/// Default log batch size for exporter flushes.
pub const DEFAULT_LOG_BATCH_SIZE: usize = 256;
/// Default trace batch size for exporter flushes.
pub const DEFAULT_TRACE_BATCH_SIZE: usize = 256;
/// Default metric batch size for exporter flushes.
pub const DEFAULT_METRIC_BATCH_SIZE: usize = 256;
/// Default metric export interval in milliseconds.
pub const DEFAULT_METRIC_EXPORT_INTERVAL_MS: u64 = 5_000;

/// Maximum envelopes claimed by one durable drain batch.
#[cfg(feature = "durable-store")]
pub(crate) const DRAIN_BATCH_SIZE: usize = 64;
/// Renew a drain lease after one third of its duration.
#[cfg(feature = "durable-store")]
pub(crate) const LEASE_RENEWAL_DIVISOR: u32 = 3;
/// `SQLite` busy timeout in milliseconds.
#[cfg(feature = "durable-store")]
pub(crate) const STORE_BUSY_TIMEOUT_MS: u64 = 5_000;
/// Pinned profiles export route.
#[cfg(feature = "sync-http")]
pub(crate) const PROFILES_EXPORT_PATH: &str = "/v1development/profiles";

/// Idle durable workers wake on admission; the timer covers other processes.
#[cfg(feature = "durable-store")]
pub(crate) const DRAIN_POLL_INTERVAL_MS: u64 = 250;
/// Maximum delay after consecutive storage/worker errors.
#[cfg(feature = "durable-store")]
pub(crate) const DRAIN_ERROR_BACKOFF_MAX_MS: u64 = 5_000;
/// Frozen durable store DDL version, checked against PRAGMA `user_version`.
#[cfg(feature = "durable-store")]
pub(crate) const STORE_SCHEMA_VERSION: u32 = 1;
/// Bound configuration reads before YAML parsing (1 MiB).
#[cfg(feature = "durable-store")]
pub(crate) const TELEMETRY_CONFIG_MAX_BYTES: u64 = 1024 * 1024;
/// `SQLite` integer form of the durable batch record limit.
#[cfg(feature = "durable-store")]
#[allow(clippy::cast_possible_wrap, reason = "the fixed limit 64 fits in i64")]
pub(crate) const DRAIN_BATCH_SIZE_SQL: i64 = DRAIN_BATCH_SIZE as i64;

/// Pinned SDK classifier cap for a server `RetryInfo` delay.
#[cfg(feature = "otlp-sdk")]
pub(crate) const SDK_RETRY_INFO_CAP: std::time::Duration = std::time::Duration::from_secs(600);
/// Pinned SDK executor cap for effective throttling.
#[cfg(feature = "otlp-sdk")]
pub(crate) const SDK_THROTTLE_CAP: std::time::Duration = std::time::Duration::from_secs(30);
/// Pinned SDK recommended maximum additive jitter, in milliseconds.
#[cfg(feature = "otlp-sdk")]
pub(crate) const SDK_RETRY_JITTER_MS: u64 = 100;
/// Protobuf Duration's maximum valid seconds magnitude.
#[cfg(feature = "otlp-sdk")]
pub(crate) const PROTO_DURATION_MAX_SECONDS: i64 = 315_576_000_000;

/// Fixed dispatch margin beyond a complete synchronous submission retry sequence.
#[cfg(feature = "sync-http")]
pub(crate) const SUBMISSION_DISPATCH_MARGIN: std::time::Duration =
    std::time::Duration::from_secs(1);

/// Maximum OTLP/JSON export response size (64 KiB, including collector warnings).
#[cfg(feature = "sync-http")]
pub(crate) const MAX_OTLP_RESPONSE_BYTES: u64 = 64 * 1024;
