//! Fixed command values that are not process exit codes.

/// Instrumentation scope name recorded on every exported signal.
pub(crate) const SCOPE_NAME: &str = "sc-otel";
/// `--attributes` value that reads the JSON object from standard input.
pub(crate) const STDIN_SOURCE: &str = "-";
