//! Opt-in typed logger sink interoperability.
//!
//! The retained [`crate::LogSink`] trait remains the registration boundary.
//! These adapters let new sink implementations use neutral typed failures
//! without changing legacy consumers or introducing root trait ambiguity.

use std::sync::Arc;

use sc_observability_types::typed::LogSinkFailure;
use sc_observability_types::v2::LogSinkError as CanonicalLogSinkError;

use crate::{LogEvent, LogSink, SinkHealth};

/// A logger sink that reports neutral typed failures.
pub trait TypedLogSink: Send + Sync {
    /// Writes one event to the sink.
    fn write(&self, event: &LogEvent) -> Result<(), CanonicalLogSinkError>;

    /// Flushes buffered state.
    fn flush(&self) -> Result<(), CanonicalLogSinkError> {
        Ok(())
    }

    /// Returns the current sink health snapshot.
    fn health(&self) -> SinkHealth;
}

/// Adapts a typed sink to the retained registration trait.
#[must_use]
pub fn legacy_sink(value: Arc<dyn TypedLogSink>) -> Arc<dyn LogSink> {
    Arc::new(LegacySinkAdapter { value })
}

/// Adapts a retained sink to the typed sink trait.
#[must_use]
pub fn typed_sink(value: Arc<dyn LogSink>) -> Arc<dyn TypedLogSink> {
    Arc::new(TypedSinkAdapter { value })
}

struct LegacySinkAdapter {
    value: Arc<dyn TypedLogSink>,
}

#[expect(
    deprecated,
    reason = "retained 1.x boundary intentionally exposes the released error wrapper"
)]
impl LogSink for LegacySinkAdapter {
    fn write(&self, event: &LogEvent) -> Result<(), sc_observability_types::LogSinkError> {
        self.value
            .write(event)
            .map_err(|error| LogSinkFailure::from_context(error.into_context()).into())
    }

    fn flush(&self) -> Result<(), sc_observability_types::LogSinkError> {
        self.value
            .flush()
            .map_err(|error| LogSinkFailure::from_context(error.into_context()).into())
    }

    fn health(&self) -> SinkHealth {
        self.value.health()
    }
}

struct TypedSinkAdapter {
    value: Arc<dyn LogSink>,
}

#[expect(
    deprecated,
    reason = "retained 1.x boundary intentionally exposes the released error wrapper"
)]
impl TypedLogSink for TypedSinkAdapter {
    fn write(&self, event: &LogEvent) -> Result<(), CanonicalLogSinkError> {
        self.value
            .write(event)
            .map_err(|error| CanonicalLogSinkError::Write { context: error.0 })
    }

    fn flush(&self) -> Result<(), CanonicalLogSinkError> {
        self.value
            .flush()
            .map_err(|error| CanonicalLogSinkError::Flush { context: error.0 })
    }

    fn health(&self) -> SinkHealth {
        self.value.health()
    }
}
