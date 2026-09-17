//! Opt-in typed logger sink interoperability.
//!
//! The retained [`crate::LogSink`] trait remains the registration boundary.
//! These adapters let new sink implementations use neutral typed failures
//! without changing legacy consumers or introducing root trait ambiguity.

use std::sync::Arc;

use sc_observability_types::typed::LogSinkFailure;

use crate::{LogEvent, LogSink, LogSinkError, SinkHealth};

/// A logger sink that reports neutral typed failures.
pub trait TypedLogSink: Send + Sync {
    /// Writes one event to the sink.
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkFailure>;

    /// Flushes buffered state.
    fn flush(&self) -> Result<(), LogSinkFailure> {
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

impl LogSink for LegacySinkAdapter {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        self.value.write(event).map_err(Into::into)
    }

    fn flush(&self) -> Result<(), LogSinkError> {
        self.value.flush().map_err(Into::into)
    }

    fn health(&self) -> SinkHealth {
        self.value.health()
    }
}

struct TypedSinkAdapter {
    value: Arc<dyn LogSink>,
}

impl TypedLogSink for TypedSinkAdapter {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkFailure> {
        self.value.write(event).map_err(Into::into)
    }

    fn flush(&self) -> Result<(), LogSinkFailure> {
        self.value.flush().map_err(Into::into)
    }

    fn health(&self) -> SinkHealth {
        self.value.health()
    }
}
