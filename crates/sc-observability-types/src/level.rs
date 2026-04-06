use serde::{Deserialize, Serialize};

/// Canonical event/log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// Verbose trace-level event.
    Trace,
    /// Debug-level event intended for development or diagnostics.
    Debug,
    /// Informational event for normal operation.
    Info,
    /// Warning event signaling degraded or unexpected behavior.
    Warn,
    /// Error event signaling a failure.
    Error,
}

/// Level threshold used by filtering surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelFilter {
    /// Allow trace, debug, info, warn, and error events.
    Trace,
    /// Allow debug, info, warn, and error events.
    Debug,
    /// Allow info, warn, and error events.
    Info,
    /// Allow warn and error events.
    Warn,
    /// Allow only error events.
    Error,
    /// Disable all events.
    Off,
}
