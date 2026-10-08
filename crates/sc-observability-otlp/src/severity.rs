//! Private mapping from core log levels to OpenTelemetry severity text.

use sc_observability_types::Level;

/// Returns the OTLP severity number and canonical upper-case text for a level.
#[must_use]
pub(crate) const fn fields(level: Level) -> (u8, &'static str) {
    match level {
        Level::Trace => (1, "TRACE"),
        Level::Debug => (5, "DEBUG"),
        Level::Info => (9, "INFO"),
        Level::Warn => (13, "WARN"),
        Level::Error => (17, "ERROR"),
    }
}
