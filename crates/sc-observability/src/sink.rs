//! Canonical logger sink contract.
//!
//! [`LogSink`] is the trait the logger runtime and [`crate::SinkRegistration`]
//! store directly. It reports the canonical [`LogSinkError`], and the built-in
//! sinks implement it without traversing the retained 1.x compatibility
//! facade. The released root [`crate::LogSink`] and
//! [`crate::typed::TypedLogSink`] traits adapt to it exactly once, at their
//! registration boundaries.

use sc_observability_types::v2::LogSinkError;
use sc_observability_types::{LogEvent, SinkHealth};

/// One concrete event sink stored by the logger runtime.
///
/// This trait is intentionally open for downstream implementations. Adding
/// required methods or tightening object-safety guarantees is therefore a
/// semver-significant public API change.
pub trait LogSink: Send + Sync {
    /// Writes one event to the sink.
    ///
    /// # Errors
    ///
    /// Returns a canonical write failure that preserves the original
    /// diagnostic, remediation, and source chain.
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError>;

    /// Flushes any buffered sink state.
    ///
    /// The default implementation has nothing to flush and succeeds.
    ///
    /// # Errors
    ///
    /// Returns a canonical flush failure that preserves the original
    /// diagnostic, remediation, and source chain.
    fn flush(&self) -> Result<(), LogSinkError> {
        Ok(())
    }

    /// Returns the current sink health snapshot.
    fn health(&self) -> SinkHealth;
}
