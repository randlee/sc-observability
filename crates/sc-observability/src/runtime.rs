use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::Duration;

use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, ErrorContext, EventError, FlushError, LogQuery, LogSnapshot,
    LoggingHealthReport, LoggingHealthState, MaintenanceHealthReport, MaintenanceWorkerState,
    QueryError, QueryHealthState, Remediation, SinkHealth, SinkHealthState, WriterState,
};
use serde_json::Value;

use crate::builder::LoggerBuilder;
use crate::follow::LogFollowSession;
use crate::health::QueryHealthTracker;
use crate::jsonl_reader::JsonlLogReader;
use crate::maintenance::{TryEnqueueError, WriterHealthSnapshot, WriterRuntime};
use crate::redact::{redact_bearer_token_text, redact_string_value};
use crate::sinks::JsonlFileSink;
use crate::{
    LogError, LogEvent, Logger, RetainedLogPolicy, Running, ServiceName, Stopped, TryLogError,
    default_log_path, error_codes, shutdown_timed_out_error_context, writer_degraded_error_context,
};

pub(crate) struct LoggerRuntime {
    pub(crate) dropped_events_total: Arc<AtomicU64>,
    pub(crate) flush_errors_total: Arc<AtomicU64>,
    pub(crate) last_error: Arc<Mutex<Option<DiagnosticSummary>>>,
    pub(crate) query_health: Arc<QueryHealthTracker>,
    pub(crate) writer: Option<WriterRuntime>,
    pub(crate) writer_snapshot: Mutex<Option<WriterHealthSnapshot>>,
}

impl LoggerRuntime {
    pub(crate) fn new(
        query_available: bool,
        sinks: Vec<crate::SinkRegistration>,
        file_sink: Option<Arc<JsonlFileSink>>,
        retained_log_policy: RetainedLogPolicy,
        queue_capacity: usize,
        #[cfg(test)] test_pass_delay: Option<Duration>,
        #[cfg(test)] test_pass_signal: Option<Arc<crate::maintenance::TestPassDelaySignal>>,
    ) -> Self {
        let dropped_events_total = Arc::new(AtomicU64::new(0));
        let flush_errors_total = Arc::new(AtomicU64::new(0));
        let last_error = Arc::new(Mutex::new(None));
        let query_health = Arc::new(QueryHealthTracker::new(if query_available {
            QueryHealthState::Healthy
        } else {
            QueryHealthState::Unavailable
        }));

        Self {
            dropped_events_total: dropped_events_total.clone(),
            flush_errors_total,
            last_error: last_error.clone(),
            query_health,
            writer: Some(WriterRuntime::new(
                sinks,
                file_sink,
                retained_log_policy,
                queue_capacity,
                dropped_events_total,
                last_error,
                #[cfg(test)]
                test_pass_delay,
                #[cfg(test)]
                test_pass_signal,
            )),
            writer_snapshot: Mutex::new(None),
        }
    }
}

impl Logger<Running> {
    /// Starts a construction-time builder for sink registration.
    pub fn builder(
        config: crate::LoggerConfig,
    ) -> Result<LoggerBuilder, sc_observability_types::InitError> {
        LoggerBuilder::new(config)
    }

    /// Creates a logger with the configured built-in sinks and runtime state.
    pub fn new(config: crate::LoggerConfig) -> Result<Self, sc_observability_types::InitError> {
        Ok(LoggerBuilder::new(config)?.build())
    }

    /// Validates, redacts, and admits one structured log event into the writer queue.
    ///
    /// # Panics
    ///
    /// Panics if the running logger has lost its writer runtime unexpectedly.
    pub fn log(&self, event: LogEvent) -> Result<(), LogError> {
        let event = self.prepare_event(event).map_err(LogError::InvalidEvent)?;
        let Some(event) = event else {
            return Ok(());
        };

        let writer = self
            .runtime
            .writer
            .as_ref()
            .expect("running logger must retain its writer runtime");
        writer.enqueue_blocking(event).map_err(|error| match error {
            TryEnqueueError::Disconnected => self.log_disconnected_error(),
            TryEnqueueError::Full => unreachable!("blocking queue admission cannot report full"),
        })
    }

    /// Attempts non-blocking queue admission for one structured log event.
    ///
    /// # Panics
    ///
    /// Panics if the running logger has lost its writer runtime unexpectedly.
    pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError> {
        let event = self
            .prepare_event(event)
            .map_err(TryLogError::InvalidEvent)?;
        let Some(event) = event else {
            return Ok(());
        };

        let writer = self
            .runtime
            .writer
            .as_ref()
            .expect("running logger must retain its writer runtime");
        match writer.enqueue_nonblocking(event) {
            Ok(()) => Ok(()),
            Err(TryEnqueueError::Full) => {
                let summary = writer.record_queue_full_drop();
                self.record_last_error(summary);
                Err(TryLogError::QueueFull(Box::new(ErrorContext::new(
                    error_codes::LOGGER_QUEUE_FULL,
                    "writer queue is full",
                    Remediation::recoverable(
                        "reduce logging pressure or increase queue capacity",
                        [
                            "inspect logger.health().queue_depth",
                            "inspect logger.health().queue_high_water_mark",
                        ],
                    ),
                ))))
            }
            Err(TryEnqueueError::Disconnected) => Err(self.try_log_disconnected_error()),
        }
    }

    /// Emits one structured log event through the compatibility path.
    #[deprecated(
        since = "1.2.0",
        note = "Use log() for blocking queue admission or try_log() for non-blocking logging."
    )]
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        self.log(event).map_err(event_error_from_log_error)?;
        if !self
            .runtime
            .writer
            .as_ref()
            .is_some_and(WriterRuntime::maintenance_active)
        {
            let _ = self.flush();
        }
        Ok(())
    }

    /// Flushes all registered sinks through the writer-owned runtime.
    ///
    /// Sink flush failures are recorded in logger health and returned to the
    /// caller as `FlushError`.
    ///
    /// # Panics
    ///
    /// Panics if the running logger has lost its writer runtime unexpectedly.
    pub fn flush(&self) -> Result<(), FlushError> {
        let writer = self
            .runtime
            .writer
            .as_ref()
            .expect("running logger must retain its writer runtime");
        if let Err(error) = writer.flush() {
            self.runtime
                .flush_errors_total
                .fetch_add(1, Ordering::SeqCst);
            self.record_last_error(DiagnosticSummary::from(error.diagnostic()));
            return Err(error);
        }
        Ok(())
    }

    /// Queries the current JSONL log set synchronously using the shared query contract.
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError> {
        let reader = self.query_reader()?;
        let result = reader.query(query);
        self.runtime.query_health.record_result(&result);
        result
    }

    /// Starts a tail-style follow session from the current end of the visible log set.
    pub fn follow(&self, query: LogQuery) -> Result<LogFollowSession, QueryError> {
        let active_log_path = self.ensure_query_available()?;
        let result = LogFollowSession::with_health(
            active_log_path,
            query,
            self.runtime.query_health.clone(),
            Some(self.shutdown.clone()),
        );
        self.runtime.query_health.record_result(&result);
        result
    }

    /// Drains queued events, stops the writer thread, and returns a stopped logger typestate.
    ///
    /// # Panics
    ///
    /// Panics if the internal writer-snapshot mutex has been poisoned while
    /// recording final runtime state.
    pub fn shutdown(mut self) -> Logger<Stopped> {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(writer) = self.runtime.writer.take() {
            let snapshot = writer.shutdown();
            if let Some(summary) = snapshot.last_writer_error.clone() {
                self.record_last_error(summary);
            }
            *self
                .runtime
                .writer_snapshot
                .lock()
                .expect("writer snapshot poisoned") = Some(snapshot);
        }
        self.runtime.query_health.mark_unavailable(None);
        Logger {
            config: self.config,
            sinks: self.sinks,
            shutdown: self.shutdown,
            runtime: self.runtime,
            state: std::marker::PhantomData,
        }
    }

    fn prepare_event(&self, event: LogEvent) -> Result<Option<LogEvent>, EventError> {
        validate_event(&event, &self.config.service_name)?;
        if !level_enabled(self.config.level, event.level) {
            return Ok(None);
        }
        Ok(Some(self.redact_event(event)))
    }

    fn redact_event(&self, mut event: LogEvent) -> LogEvent {
        if self.config.redaction.redact_bearer_tokens
            && let Some(message) = event.message.as_mut()
        {
            *message = redact_bearer_token_text(message);
        }

        for (key, value) in &mut event.fields {
            if self
                .config
                .redaction
                .denylist_keys
                .iter()
                .any(|deny| deny == key)
            {
                *value = Value::String(crate::constants::REDACTED_VALUE.to_string());
            }
            if self.config.redaction.redact_bearer_tokens {
                redact_string_value(value);
            }
            for redactor in &self.config.redaction.custom_redactors {
                redactor.redact(key, value);
            }
        }

        event
    }

    fn query_reader(&self) -> Result<JsonlLogReader, QueryError> {
        self.ensure_query_available().map(JsonlLogReader::new)
    }

    fn ensure_query_available(&self) -> Result<PathBuf, QueryError> {
        if !self.config.enable_file_sink {
            let error = crate::query::unavailable_error(
                "logger query/follow requires the built-in JSONL file sink to be enabled",
            );
            self.runtime.query_health.record_error(&error);
            return Err(error);
        }

        Ok(default_log_path(
            &self.config.log_root,
            &self.config.service_name,
        ))
    }

    fn log_disconnected_error(&self) -> LogError {
        match self.runtime_snapshot().last_writer_error {
            Some(summary)
                if summary.code.as_ref() == Some(&error_codes::LOGGER_SHUTDOWN_TIMED_OUT) =>
            {
                LogError::ShutdownTimedOut(Box::new(shutdown_timed_out_error_context(
                    &summary.message,
                )))
            }
            Some(summary) => {
                LogError::WriterDegraded(Box::new(writer_degraded_error_context(&format!(
                    "writer thread disconnected while admitting log work: {}",
                    summary.message
                ))))
            }
            None => LogError::WriterDegraded(Box::new(writer_degraded_error_context(
                "writer thread disconnected while admitting log work",
            ))),
        }
    }

    fn try_log_disconnected_error(&self) -> TryLogError {
        match self.runtime_snapshot().last_writer_error {
            Some(summary)
                if summary.code.as_ref() == Some(&error_codes::LOGGER_SHUTDOWN_TIMED_OUT) =>
            {
                TryLogError::ShutdownTimedOut(Box::new(shutdown_timed_out_error_context(
                    &summary.message,
                )))
            }
            Some(summary) => {
                TryLogError::WriterDegraded(Box::new(writer_degraded_error_context(&format!(
                    "writer thread disconnected while admitting non-blocking log work: {}",
                    summary.message
                ))))
            }
            None => TryLogError::WriterDegraded(Box::new(writer_degraded_error_context(
                "writer thread disconnected while admitting non-blocking log work",
            ))),
        }
    }
}

impl<State> Logger<State> {
    /// Returns aggregate logging and query/follow health for the runtime.
    ///
    /// # Panics
    ///
    /// Panics if the internal writer-snapshot or last-error mutex has been
    /// poisoned while aggregating runtime health.
    pub fn health(&self) -> LoggingHealthReport {
        let sink_statuses: Vec<SinkHealth> =
            self.sinks.iter().map(|entry| entry.sink.health()).collect();
        let writer_snapshot = self.runtime_snapshot();
        let state = if self.shutdown.load(Ordering::SeqCst) {
            LoggingHealthState::Unavailable
        } else {
            aggregate_logging_health_state(
                &sink_statuses,
                writer_snapshot.writer_state,
                writer_snapshot.maintenance.as_ref(),
                self.runtime.dropped_events_total.load(Ordering::SeqCst),
                self.runtime.flush_errors_total.load(Ordering::SeqCst),
                writer_snapshot.queue_full_drops_total,
            )
        };

        LoggingHealthReport {
            state,
            dropped_events_total: self.runtime.dropped_events_total.load(Ordering::SeqCst),
            flush_errors_total: self.runtime.flush_errors_total.load(Ordering::SeqCst),
            active_log_path: default_log_path(&self.config.log_root, &self.config.service_name),
            sink_statuses,
            queue_depth: writer_snapshot.queue_depth,
            queue_capacity: writer_snapshot.queue_capacity,
            queue_high_water_mark: writer_snapshot.queue_high_water_mark,
            queue_full_drops_total: writer_snapshot.queue_full_drops_total,
            writer_state: writer_snapshot.writer_state,
            last_writer_error: writer_snapshot.last_writer_error,
            query: Some(self.runtime.query_health.snapshot()),
            maintenance: writer_snapshot.maintenance,
            last_error: self
                .runtime
                .last_error
                .lock()
                .expect("logger last_error poisoned")
                .clone(),
        }
    }

    fn record_last_error(&self, summary: DiagnosticSummary) {
        *self
            .runtime
            .last_error
            .lock()
            .expect("logger last_error poisoned") = Some(summary);
    }

    fn runtime_snapshot(&self) -> WriterHealthSnapshot {
        if let Some(writer) = self.runtime.writer.as_ref() {
            writer.snapshot()
        } else {
            self.runtime
                .writer_snapshot
                .lock()
                .expect("writer snapshot poisoned")
                .clone()
                .unwrap_or(WriterHealthSnapshot {
                    queue_depth: 0,
                    queue_capacity: self.config.queue_capacity as u64,
                    queue_high_water_mark: 0,
                    queue_full_drops_total: 0,
                    writer_state: WriterState::Stopped,
                    last_writer_error: None,
                    maintenance: None,
                })
        }
    }
}

fn aggregate_logging_health_state(
    sink_statuses: &[SinkHealth],
    writer_state: WriterState,
    maintenance: Option<&MaintenanceHealthReport>,
    dropped_events_total: u64,
    flush_errors_total: u64,
    queue_full_drops_total: u64,
) -> LoggingHealthState {
    if writer_state == WriterState::Stopped
        || sink_statuses
            .iter()
            .any(|sink| sink.state == SinkHealthState::Unavailable)
    {
        LoggingHealthState::Unavailable
    } else if writer_state == WriterState::Degraded
        || maintenance.is_some_and(|report| report.state == MaintenanceWorkerState::Degraded)
        || sink_statuses
            .iter()
            .any(|sink| sink.state != SinkHealthState::Healthy)
        || dropped_events_total != 0
        || flush_errors_total != 0
        || queue_full_drops_total != 0
    {
        LoggingHealthState::DegradedDropping
    } else {
        LoggingHealthState::Healthy
    }
}

fn event_error_from_log_error(error: LogError) -> EventError {
    match error {
        LogError::InvalidEvent(error) => error,
        LogError::WriterDegraded(error) => EventError(Box::new(writer_degraded_error_context(
            &error.diagnostic().message,
        ))),
        LogError::ShutdownTimedOut(error) => EventError(Box::new(
            shutdown_timed_out_error_context(&error.diagnostic().message),
        )),
    }
}

fn level_enabled(
    filter: sc_observability_types::LevelFilter,
    level: sc_observability_types::Level,
) -> bool {
    use sc_observability_types::{Level, LevelFilter};

    match filter {
        LevelFilter::Trace => true,
        LevelFilter::Debug => matches!(
            level,
            Level::Debug | Level::Info | Level::Warn | Level::Error
        ),
        LevelFilter::Info => matches!(level, Level::Info | Level::Warn | Level::Error),
        LevelFilter::Warn => matches!(level, Level::Warn | Level::Error),
        LevelFilter::Error => matches!(level, Level::Error),
        LevelFilter::Off => false,
    }
}

fn validate_event(event: &LogEvent, expected_service: &ServiceName) -> Result<(), EventError> {
    if event.version.as_str() != sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION {
        return Err(EventError(Box::new(ErrorContext::new(
            error_codes::LOGGER_INVALID_EVENT,
            "log event version is invalid",
            Remediation::recoverable(
                "emit an observation v1 log event",
                ["recreate the event with the current contract"],
            ),
        ))));
    }

    if &event.service != expected_service {
        return Err(EventError(Box::new(ErrorContext::new(
            error_codes::LOGGER_INVALID_EVENT,
            "log event service does not match logger service",
            Remediation::recoverable(
                "emit the event with the logger service name",
                ["rebuild the event before emitting"],
            ),
        ))));
    }

    Ok(())
}
