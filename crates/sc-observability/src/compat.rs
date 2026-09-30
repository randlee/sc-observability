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

use crate::{
    AdmissionOutcome, ErrorContext, EventError, FlushError, InitError, LevelOwner, LogEvent,
    LogFailure, Logger, LoggerBuilder, LoggerConfig, Running, TryLogFailure, error_codes,
};

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

impl LoggerBuilder {
    /// Creates a 1.x builder with the released initialization error wrapper.
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        Self::new_canonical(config).map_err(legacy_init)
    }

    /// Finalizes construction and returns a level owner with the retained
    /// 1.x initialization error wrapper.
    pub fn build_with_level_owner(self) -> Result<(Logger<Running>, LevelOwner), InitError> {
        self.build_with_level_owner_canonical().map_err(legacy_init)
    }
}

impl Logger<Running> {
    /// Starts a 1.x builder with the released initialization error wrapper.
    pub fn builder(config: LoggerConfig) -> Result<LoggerBuilder, InitError> {
        LoggerBuilder::new(config)
    }

    /// Creates a logger with the retained 1.x initialization error wrapper.
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        Self::new_canonical(config).map_err(legacy_init)
    }

    /// Creates a logger and level owner with the retained 1.x error wrapper.
    pub fn new_with_level_owner(config: LoggerConfig) -> Result<(Self, LevelOwner), InitError> {
        Self::new_with_level_owner_canonical(config).map_err(legacy_init)
    }

    /// Validates, redacts, and blocks for queue admission using 1.x errors.
    pub fn log(&self, event: LogEvent) -> Result<(), LogError> {
        self.log_canonical(event).map_err(legacy_log)
    }

    /// Validates, redacts, and blocks for queue admission using typed 1.x
    /// failures.
    pub fn log_typed(&self, event: LogEvent) -> Result<(), LogFailure> {
        self.log(event).map_err(Into::into)
    }

    /// Attempts non-blocking queue admission using 1.x errors.
    pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError> {
        self.try_log_with_outcome(event).map(|_| ())
    }

    /// Attempts non-blocking queue admission using typed 1.x failures.
    pub fn try_log_typed(&self, event: LogEvent) -> Result<(), TryLogFailure> {
        self.try_log(event).map_err(Into::into)
    }

    /// Attempts non-blocking admission and reports filtering using 1.x errors.
    pub fn try_log_with_outcome(&self, event: LogEvent) -> Result<AdmissionOutcome, TryLogError> {
        self.try_log_with_outcome_canonical(event)
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
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        self.emit_canonical(event)
            .map_err(|error| legacy_event_from_log(legacy_log(error)))
    }

    /// Flushes the shared writer through the retained 1.x error wrapper.
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_canonical().map_err(legacy_flush)
    }

    /// Flushes the shared writer through the released typed failure.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.flush().map_err(Into::into)
    }
}

fn legacy_init(error: CanonicalInitError) -> InitError {
    InitFailure::from_context(error.into_context()).into()
}

fn legacy_event(context: Box<ErrorContext>) -> EventError {
    EventFailure::from_context(context).into()
}

fn legacy_log(error: CanonicalEventError) -> LogError {
    match error {
        CanonicalEventError::Validation { context } => {
            LogError::InvalidEvent(legacy_event(context))
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

fn legacy_flush(error: CanonicalFlushError) -> FlushError {
    FlushFailure::from_context(error.into_context()).into()
}
