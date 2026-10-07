use serde::{Deserialize, Serialize};

/// Final span status for a completed span record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// The span completed successfully.
    Ok,
    /// The span completed with an error.
    Error,
    /// The span completed without an explicit outcome.
    Unset,
}

/// Typestate marker for a started-but-not-yet-ended span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanStarted;

/// Typestate marker for a completed span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanEnded;
