//! Fixed command values that are not process exit codes.

/// Endpoint used when neither `--endpoint` nor `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
pub(crate) const DEFAULT_ENDPOINT: &str = "http://localhost:4318";
pub(crate) const ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// Instrumentation scope name recorded on every exported signal.
pub(crate) const SCOPE_NAME: &str = "sc-otel";
/// `--attributes` value that reads the JSON object from standard input.
pub(crate) const STDIN_SOURCE: &str = "-";
