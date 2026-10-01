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
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by durable-store")
)]
pub(crate) const DRAIN_BATCH_SIZE: usize = 64;
/// Renew a drain lease after one third of its duration.
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by durable-store")
)]
pub(crate) const LEASE_RENEWAL_DIVISOR: u32 = 3;
/// `SQLite` busy timeout in milliseconds.
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by durable-store")
)]
pub(crate) const STORE_BUSY_TIMEOUT_MS: u64 = 5_000;
/// Pinned profiles export route.
#[expect(
    dead_code,
    reason = "staged by d-29; wired by d-33/d-34 under durable-store"
)]
pub(crate) const PROFILES_EXPORT_PATH: &str = "/v1development/profiles";
