//! Retained 1.x logger facade over the canonical D22 implementation.
//!
//! These wrappers deliberately own no runtime state. They translate only at
//! the public boundary, so root callers and canonical callers share one
//! writer, sink fan-out, and lifecycle.

#![expect(
    deprecated,
    reason = "this removable compatibility module implements released 1.x error signatures"
)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

use sc_observability_types::typed::{EventFailure, FlushFailure, InitFailure};
use sc_observability_types::v2::{
    EventError as CanonicalEventError, FlushError as CanonicalFlushError,
    InitError as CanonicalInitError,
};

use crate::builder::CanonicalLoggerBuilder;
use crate::typed::TypedLogSink;
use crate::{
    AdmissionOutcome, CanonicalLogger, ErrorContext, EventError, FlushError, InitError, LevelOwner,
    LevelState, LogEvent, LogFailure, LogQuery, Logger, LoggerBuilder, LoggerConfig,
    LoggingHealthReport, Running, SinkRegistration, Stopped, TryLogFailure, error_codes,
};
use sc_observability_types::QueryError;
use std::sync::Arc;

/// Blocking queue-admission error surface retained for 1.x callers.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
pub enum LogError {
    /// The event failed validation before queue admission.
    #[error(transparent)]
    InvalidEvent(EventError),
    /// The writer cannot accept more work reliably.
    #[error("{0}")]
    WriterDegraded(#[source] Box<ErrorContext>),
    /// The logger exceeded its shutdown timeout while draining.
    #[error("{0}")]
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}

/// Non-blocking queue-admission error surface retained for 1.x callers.
#[derive(Debug, PartialEq, Serialize, Deserialize, Error)]
pub enum TryLogError {
    /// The event failed validation before queue admission.
    #[error(transparent)]
    InvalidEvent(EventError),
    /// The bounded queue was full and did not admit the event.
    #[error("{0}")]
    QueueFull(#[source] Box<ErrorContext>),
    /// The writer cannot accept more work reliably.
    #[error("{0}")]
    WriterDegraded(#[source] Box<ErrorContext>),
    /// The logger exceeded its shutdown timeout while draining.
    #[error("{0}")]
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}

impl From<LogError> for LogFailure {
    fn from(value: LogError) -> Self {
        match value {
            LogError::InvalidEvent(error) => Self::InvalidEvent(error.into()),
            LogError::WriterDegraded(context) => Self::WriterDegraded(context),
            LogError::ShutdownTimedOut(context) => Self::ShutdownTimedOut(context),
        }
    }
}

impl From<LogFailure> for LogError {
    fn from(value: LogFailure) -> Self {
        match value {
            LogFailure::InvalidEvent(error) => Self::InvalidEvent(error.into()),
            LogFailure::WriterDegraded(context) => Self::WriterDegraded(context),
            LogFailure::ShutdownTimedOut(context) => Self::ShutdownTimedOut(context),
            _ => unreachable!("unknown LogFailure variants cannot be constructed by this version"),
        }
    }
}

impl From<TryLogError> for TryLogFailure {
    fn from(value: TryLogError) -> Self {
        match value {
            TryLogError::InvalidEvent(error) => Self::InvalidEvent(error.into()),
            TryLogError::QueueFull(context) => Self::QueueFull(context),
            TryLogError::WriterDegraded(context) => Self::WriterDegraded(context),
            TryLogError::ShutdownTimedOut(context) => Self::ShutdownTimedOut(context),
        }
    }
}

impl From<TryLogFailure> for TryLogError {
    fn from(value: TryLogFailure) -> Self {
        match value {
            TryLogFailure::InvalidEvent(error) => Self::InvalidEvent(error.into()),
            TryLogFailure::QueueFull(context) => Self::QueueFull(context),
            TryLogFailure::WriterDegraded(context) => Self::WriterDegraded(context),
            TryLogFailure::ShutdownTimedOut(context) => Self::ShutdownTimedOut(context),
            _ => {
                unreachable!("unknown TryLogFailure variants cannot be constructed by this version")
            }
        }
    }
}

impl From<CanonicalLoggerBuilder> for LoggerBuilder {
    fn from(inner: CanonicalLoggerBuilder) -> Self {
        Self { inner }
    }
}

impl From<LoggerBuilder> for CanonicalLoggerBuilder {
    fn from(value: LoggerBuilder) -> Self {
        value.inner
    }
}

impl LoggerBuilder {
    /// Creates a 1.x builder with the released initialization error wrapper.
    #[deprecated(since = "1.4.0", note = "Use v2::LoggerBuilder::new instead.")]
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        CanonicalLoggerBuilder::new(config)
            .map(Self::from)
            .map_err(legacy_init)
    }

    /// Creates a released typed builder facade.
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure> {
        CanonicalLoggerBuilder::new(config)
            .map(Self::from)
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Registers one released sink before building the shared runtime.
    pub fn register_sink(&mut self, registration: SinkRegistration) -> &mut Self {
        self.inner.register_sink(registration);
        self
    }

    /// Registers a released typed sink before building the shared runtime.
    pub fn register_typed_sink(
        &mut self,
        sink: Arc<dyn TypedLogSink>,
    ) -> Result<&mut Self, crate::SinkRegistrationError> {
        self.inner.register_typed_sink(sink)?;
        Ok(self)
    }

    /// Builds the released infallible logger facade.
    ///
    /// # Panics
    ///
    /// Panics if the writer runtime cannot start, preserving the released
    /// infallible builder contract.
    #[deprecated(since = "1.4.0", note = "Use v2::LoggerBuilder::build instead.")]
    pub fn build(self) -> Logger<Running> {
        Logger::from(
            self.inner
                .build()
                .expect("existing infallible builder expects writer thread startup"),
        )
    }

    /// Builds the released typed initialization facade.
    pub fn build_typed(self) -> Result<Logger<Running>, InitFailure> {
        self.inner
            .build()
            .map(Logger::from)
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Finalizes construction and returns a level owner with the retained
    /// 1.x initialization error wrapper.
    pub fn build_with_level_owner(self) -> Result<(Logger<Running>, LevelOwner), InitError> {
        self.inner
            .build_with_level_owner()
            .map(|(logger, owner)| (Logger::from(logger), owner))
            .map_err(legacy_init)
    }

    /// Builds the released typed initialization facade with level ownership.
    pub fn build_with_level_owner_typed(
        self,
    ) -> Result<(Logger<Running>, LevelOwner), InitFailure> {
        self.inner
            .build_with_level_owner()
            .map(|(logger, owner)| (Logger::from(logger), owner))
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }
}

impl<State> From<CanonicalLogger<State>> for Logger<State> {
    fn from(inner: CanonicalLogger<State>) -> Self {
        Self {
            inner,
            shutdown: std::marker::PhantomData,
        }
    }
}

impl<State> From<Logger<State>> for CanonicalLogger<State> {
    fn from(value: Logger<State>) -> Self {
        value.inner
    }
}

impl Logger<Running> {
    /// Queries through the released facade.
    pub fn query(&self, query: &LogQuery) -> Result<crate::LogSnapshot, QueryError> {
        self.inner.query(query)
    }

    /// Shuts down the shared canonical runtime and returns a stopped facade.
    pub fn shutdown(self) -> Logger<Stopped> {
        Logger::from(self.inner.shutdown())
    }

    /// Starts a 1.x builder with the released initialization error wrapper.
    #[deprecated(since = "1.4.0", note = "Use v2::Logger::builder instead.")]
    pub fn builder(config: LoggerConfig) -> Result<LoggerBuilder, InitError> {
        LoggerBuilder::new(config)
    }

    /// Starts a released typed builder facade.
    pub fn builder_typed(config: LoggerConfig) -> Result<LoggerBuilder, InitFailure> {
        LoggerBuilder::new_typed(config)
    }

    /// Creates a logger with the retained 1.x initialization error wrapper.
    #[deprecated(since = "1.4.0", note = "Use v2::Logger::new instead.")]
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        CanonicalLogger::new(config)
            .map(Self::from)
            .map_err(legacy_init)
    }

    /// Creates a logger and level owner with the retained 1.x error wrapper.
    #[deprecated(
        since = "1.4.0",
        note = "Use v2::Logger::new_with_level_owner instead."
    )]
    pub fn new_with_level_owner(config: LoggerConfig) -> Result<(Self, LevelOwner), InitError> {
        CanonicalLogger::new_with_level_owner(config)
            .map(|(logger, owner)| (Self::from(logger), owner))
            .map_err(legacy_init)
    }

    /// Creates a released typed logger facade.
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure> {
        CanonicalLogger::new(config)
            .map(Self::from)
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Creates a released typed logger facade with level ownership.
    pub fn new_with_level_owner_typed(
        config: LoggerConfig,
    ) -> Result<(Self, LevelOwner), InitFailure> {
        CanonicalLogger::new_with_level_owner(config)
            .map(|(logger, owner)| (Self::from(logger), owner))
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Validates, redacts, and blocks for released queue admission.
    #[deprecated(since = "1.4.0", note = "Use v2::Logger::log instead.")]
    pub fn log(&self, event: LogEvent) -> Result<(), LogError> {
        self.inner.log(event).map_err(legacy_log)
    }

    /// Validates, redacts, and blocks for queue admission using typed 1.x
    /// failures.
    pub fn log_typed(&self, event: LogEvent) -> Result<(), LogFailure> {
        self.log(event).map_err(Into::into)
    }

    /// Attempts non-blocking queue admission using 1.x errors.
    #[deprecated(since = "1.4.0", note = "Use v2::Logger::try_log instead.")]
    pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError> {
        self.try_log_with_outcome(event).map(|_| ())
    }

    /// Attempts non-blocking queue admission using typed 1.x failures.
    pub fn try_log_typed(&self, event: LogEvent) -> Result<(), TryLogFailure> {
        self.try_log(event).map_err(Into::into)
    }

    /// Attempts non-blocking admission and reports filtering using 1.x errors.
    #[deprecated(
        since = "1.4.0",
        note = "Use v2::Logger::try_log_with_outcome instead."
    )]
    pub fn try_log_with_outcome(&self, event: LogEvent) -> Result<AdmissionOutcome, TryLogError> {
        self.inner
            .try_log_with_outcome(event)
            .map_err(legacy_try_log)
    }

    /// Attempts non-blocking admission using typed 1.x failures.
    pub fn try_log_with_outcome_typed(
        &self,
        event: LogEvent,
    ) -> Result<AdmissionOutcome, TryLogFailure> {
        self.try_log_with_outcome(event).map_err(Into::into)
    }

    /// Retained event-emission compatibility path.
    #[deprecated(
        since = "1.2.0",
        note = "Use log() for blocking queue admission or try_log() for non-blocking logging."
    )]
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        self.inner
            .emit_legacy(event)
            .map_err(|error| legacy_event_from_log(legacy_log(error)))
    }

    /// Flushes the shared writer through the released typed failure.
    #[deprecated(since = "1.4.0", note = "Use v2::Logger::flush instead.")]
    #[expect(
        deprecated,
        reason = "the released 1.x method retains its deprecated error wrapper"
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.inner.flush().map_err(legacy_flush)
    }

    /// Flushes the shared writer through the released typed failure.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush().map_err(Into::into)
    }
}

impl<State> Logger<State> {
    /// Returns the configured service identity through the released facade.
    #[must_use]
    pub fn service_name(&self) -> &crate::ServiceName {
        self.inner.service_name()
    }

    /// Returns a coherent runtime level snapshot through the released facade.
    #[must_use]
    pub fn level_state(&self) -> LevelState {
        self.inner.level_state()
    }

    /// Returns runtime health through the released facade.
    #[must_use]
    pub fn health(&self) -> LoggingHealthReport {
        self.inner.health()
    }
}

impl Logger<Running> {
    /// Follows through the released facade.
    pub fn follow(&self, query: LogQuery) -> Result<crate::follow::LogFollowSession, QueryError> {
        self.inner.follow(query)
    }
}

fn legacy_init(error: CanonicalInitError) -> InitError {
    InitFailure::from_context(error.into_context()).into()
}

fn legacy_event(context: Box<ErrorContext>) -> EventError {
    EventFailure::from_context(context).into()
}

pub(crate) fn legacy_log(error: CanonicalEventError) -> LogError {
    match error {
        CanonicalEventError::Validation { context } => {
            LogError::InvalidEvent(legacy_event(context))
        }
        CanonicalEventError::Routing { context }
            if context.diagnostic().code == error_codes::LOGGER_SHUTDOWN_TIMED_OUT =>
        {
            LogError::ShutdownTimedOut(context)
        }
        CanonicalEventError::Routing { context } => LogError::WriterDegraded(context),
        _ => LogError::WriterDegraded(error.into_context()),
    }
}

fn legacy_try_log(error: CanonicalEventError) -> TryLogError {
    match error {
        CanonicalEventError::Validation { context } => {
            TryLogError::InvalidEvent(legacy_event(context))
        }
        CanonicalEventError::Routing { context }
            if context.diagnostic().code == error_codes::LOGGER_QUEUE_FULL =>
        {
            TryLogError::QueueFull(context)
        }
        CanonicalEventError::Routing { context }
            if context.diagnostic().code == error_codes::LOGGER_SHUTDOWN_TIMED_OUT =>
        {
            TryLogError::ShutdownTimedOut(context)
        }
        CanonicalEventError::Routing { context } => TryLogError::WriterDegraded(context),
        _ => TryLogError::WriterDegraded(error.into_context()),
    }
}

fn legacy_event_from_log(error: LogError) -> EventError {
    match error {
        LogError::InvalidEvent(error) => error,
        LogError::WriterDegraded(context) | LogError::ShutdownTimedOut(context) => {
            legacy_event(context)
        }
    }
}

pub(crate) fn legacy_flush(error: CanonicalFlushError) -> FlushError {
    FlushFailure::from_context(error.into_context()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::Remediation;

    fn shutdown_timeout() -> CanonicalEventError {
        CanonicalEventError::Routing {
            context: Box::new(ErrorContext::new(
                error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
                "writer thread did not stop within 10ms",
                Remediation::recoverable("wait for writer shutdown", ["retry after shutdown"]),
            )),
        }
    }

    #[test]
    fn released_log_and_try_log_preserve_shutdown_timeout_variants() {
        assert!(matches!(
            legacy_log(shutdown_timeout()),
            LogError::ShutdownTimedOut(_)
        ));
        assert!(matches!(
            legacy_try_log(shutdown_timeout()),
            TryLogError::ShutdownTimedOut(_)
        ));
    }
}
