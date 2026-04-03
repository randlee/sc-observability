//! Crate-local constants for `sc-observability-otlp`.

/// Default OTLP request timeout in milliseconds.
pub const DEFAULT_OTLP_TIMEOUT_MS: u64 = 3_000;
/// Default maximum number of OTLP export retries.
pub const DEFAULT_OTLP_MAX_RETRIES: u32 = 3;
/// Default initial OTLP retry backoff in milliseconds.
pub const DEFAULT_OTLP_INITIAL_BACKOFF_MS: u64 = 250;
/// Default maximum OTLP retry backoff in milliseconds.
pub const DEFAULT_OTLP_MAX_BACKOFF_MS: u64 = 5_000;
/// Default log batch size for exporter flushes.
pub const DEFAULT_LOG_BATCH_SIZE: usize = 256;
/// Default trace batch size for exporter flushes.
pub const DEFAULT_TRACE_BATCH_SIZE: usize = 256;
/// Default metric batch size for exporter flushes.
pub const DEFAULT_METRIC_BATCH_SIZE: usize = 256;
/// Default metric export interval in milliseconds.
pub const DEFAULT_METRIC_EXPORT_INTERVAL_MS: u64 = 5_000;
