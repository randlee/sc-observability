//! Fixed command values that are not process exit codes.

pub(crate) const INTERNAL_ERROR_CODE: &str = "SC_OTEL_CLI_INTERNAL";
pub(crate) const PANIC_DETAILS_ENV: &str = "SC_OTEL_DEBUG_PANIC";
/// Instrumentation scope name recorded on every exported signal.
pub(crate) const SCOPE_NAME: &str = "sc-otel";
/// `--attributes` value that reads the JSON object from standard input.
pub(crate) const STDIN_SOURCE: &str = "-";
