//! Crate-local constants for `sc-observability-otlp`.

/// Endpoint the synchronous frontends use when neither an explicit endpoint
/// nor [`OTLP_ENDPOINT_ENV`] is set.
pub(crate) const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4318";

/// Environment variable naming the synchronous frontends' default endpoint.
pub(crate) const OTLP_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Default OTLP request timeout in milliseconds.
pub const DEFAULT_OTLP_TIMEOUT_MS: u64 = 3_000;
/// Largest frontend input, in bytes, accepted by one synchronous-client call
/// (1 MiB). CLI and Python entry points reject larger input before parsing or
/// exporting it. This is a per-call safety limit, not a queue.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Largest number of records one synchronous-client frontend call accepts.
pub const MAX_BATCH_RECORDS: usize = 10_000;
/// Interval of the synchronous client's per-call `PeriodicReader`. It only
/// has to exceed one call so that the explicit flush performs the export.
#[cfg(feature = "synchronous-client")]
pub(crate) const SYNC_METRIC_READER_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(24 * 60 * 60);
/// OTLP/HTTP logs path appended to the synchronous client's base endpoint.
#[cfg(feature = "synchronous-client")]
pub(crate) const OTLP_HTTP_LOGS_PATH: &str = "v1/logs";
/// OTLP/HTTP traces path appended to the synchronous client's base endpoint.
#[cfg(feature = "synchronous-client")]
pub(crate) const OTLP_HTTP_TRACES_PATH: &str = "v1/traces";
/// OTLP/HTTP metrics path appended to the synchronous client's base endpoint.
#[cfg(feature = "synchronous-client")]
pub(crate) const OTLP_HTTP_METRICS_PATH: &str = "v1/metrics";
/// Health name reported by `OtelLogSink`.
#[cfg(feature = "log-sink")]
pub(crate) const OTEL_LOG_SINK_NAME: &str = "opentelemetry";
/// Target namespace of OpenTelemetry SDK diagnostics, dropped at the exact
/// target or `::` child boundary.
#[cfg(feature = "log-sink")]
pub(crate) const SDK_DIAGNOSTIC_TARGET: &str = "opentelemetry";
