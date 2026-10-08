//! Process exit codes and fixed command values.

pub(crate) const EXIT_OK: u8 = 0;
pub(crate) const EXIT_INTERNAL: u8 = 1;
pub(crate) const EXIT_USAGE: u8 = 2;
/// Input or configuration was rejected before any export was attempted.
pub(crate) const EXIT_VALIDATION: u8 = 3;
/// The official exporter reported a failed export.
/// Use 7 to distinguish delivery failures from success and local CLI errors (0–3).
pub(crate) const EXIT_EXPORT: u8 = 7;

/// Endpoint used when neither `--endpoint` nor `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
pub(crate) const DEFAULT_ENDPOINT: &str = "http://localhost:4318";
pub(crate) const ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// Instrumentation scope name recorded on every exported signal.
pub(crate) const SCOPE_NAME: &str = "sc-otel";
/// `--attributes` value that reads the JSON object from standard input.
pub(crate) const STDIN_SOURCE: &str = "-";
