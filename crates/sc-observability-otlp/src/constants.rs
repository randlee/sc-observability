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
/// Default total deadline for one legacy retry sequence.
pub const DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS: u64 = 30_000;
/// Default upper bound for a legacy Retry-After value.
pub const DEFAULT_OTLP_RETRY_AFTER_CAP_MS: u64 = 5_000;
/// Default legacy retry jitter percentage.
pub const DEFAULT_OTLP_RETRY_JITTER_PERCENT: u8 = 20;
/// Default log batch size for exporter flushes.
pub const DEFAULT_LOG_BATCH_SIZE: usize = 256;
/// Default trace batch size for exporter flushes.
pub const DEFAULT_TRACE_BATCH_SIZE: usize = 256;
/// Default metric batch size for exporter flushes.
pub const DEFAULT_METRIC_BATCH_SIZE: usize = 256;
/// Default metric export interval in milliseconds.
pub const DEFAULT_METRIC_EXPORT_INTERVAL_MS: u64 = 5_000;
