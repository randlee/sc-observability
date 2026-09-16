use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
#[cfg(test)]
use std::time::Duration;

use sc_observability_types::{
    AdmissionOutcome, ChangeDiagnostic, DiagnosticInfo, DiagnosticSummary, ErrorCode, ErrorContext,
    EventError, FlushError, InitError, LevelChange, LevelChangeError, LevelChangeSource,
    LevelFilter, LevelState, LogQuery, LogSnapshot, LoggingHealthReport, LoggingHealthState,
    MaintenanceHealthReport, MaintenanceWorkerState, OperationDiagnostic, QueryError,
    QueryHealthState, Remediation, SinkHealth, SinkHealthState, Timestamp, WriterState,
};
use serde_json::Value;

use crate::builder::LoggerBuilder;
use crate::follow::LogFollowSession;
use crate::health::QueryHealthTracker;
use crate::jsonl_reader::JsonlLogReader;
use crate::maintenance::{
    DiagnosticAdmitter, TryEnqueueError, WriterHealthSnapshot, WriterRuntime,
};
use crate::redact::{redact_bearer_token_text, redact_string_value};
use crate::sinks::JsonlFileSink;
use crate::{
    LevelOwner, LogError, LogEvent, Logger, RetainedLogPolicy, Running, ServiceName, Stopped,
    TryLogError, default_log_path, error_codes, shutdown_timed_out_error_context,
    writer_degraded_error_context,
};

pub(crate) struct LoggerRuntime {
    pub(crate) dropped_events_total: Arc<AtomicU64>,
    pub(crate) flush_errors_total: Arc<AtomicU64>,
    pub(crate) last_error: Arc<Mutex<Option<DiagnosticSummary>>>,
    pub(crate) query_health: Arc<QueryHealthTracker>,
    pub(crate) writer: Option<WriterRuntime>,
    pub(crate) writer_snapshot: Mutex<Option<WriterHealthSnapshot>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelLifecycle {
    Running,
    Stopping,
    Stopped,
}

pub(crate) struct LevelControl {
    pub(crate) state: LevelState,
    pub(crate) lifecycle: LevelLifecycle,
    diagnostic_admitter: Weak<dyn Fn(LogEvent) -> Result<(), TryEnqueueError> + Send + Sync>,
    service: ServiceName,
}

impl std::fmt::Debug for LevelControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LevelControl")
            .field("state", &self.state)
            .field("lifecycle", &self.lifecycle)
            .finish_non_exhaustive()
    }
}

impl LevelControl {
    pub(crate) fn new(
        configured_level: LevelFilter,
        service: ServiceName,
        diagnostic_admitter: &DiagnosticAdmitter,
    ) -> Self {
        Self {
            state: LevelState {
                configured_level,
                effective_level: configured_level,
                revision: 0,
            },
            lifecycle: LevelLifecycle::Running,
            diagnostic_admitter: Arc::downgrade(diagnostic_admitter),
            service,
        }
    }

    fn admit_change_diagnostic(
        &self,
        previous: LevelState,
        current: LevelState,
        source: LevelChangeSource,
    ) -> ChangeDiagnostic {
        let event = LogEvent {
            version: sc_observability_types::SchemaVersion::new(
                sc_observability_types::OBSERVATION_ENVELOPE_VERSION,
            )
            .expect("workspace schema constant is valid"),
            timestamp: Timestamp::now_utc(),
            level: sc_observability_types::Level::Info,
            service: self.service.clone(),
            target: sc_observability_types::TargetCategory::new("sc_observability")
                .expect("static diagnostic target is valid"),
            action: sc_observability_types::ActionName::new("logging.level_changed")
                .expect("static diagnostic action is valid"),
            message: Some("logger level changed".to_string()),
            identity: sc_observability_types::ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields: serde_json::Map::from_iter([
                (
                    "previous_level".to_string(),
                    serde_json::json!(previous.effective_level),
                ),
                (
                    "effective_level".to_string(),
                    serde_json::json!(current.effective_level),
                ),
                (
                    "configured_level".to_string(),
                    serde_json::json!(current.configured_level),
                ),
                (
                    "level_revision".to_string(),
                    serde_json::json!(current.revision),
                ),
                ("source".to_string(), serde_json::json!(source)),
            ]),
        };
        let Some(admitter) = self.diagnostic_admitter.upgrade() else {
            return ChangeDiagnostic::NotAccepted {
                diagnostic: unavailable_level_diagnostic(
                    "logger stopped while recording the change diagnostic",
                ),
            };
        };
        match admitter(event) {
            Ok(()) => ChangeDiagnostic::Accepted,
            Err(TryEnqueueError::Full) => ChangeDiagnostic::NotAccepted {
                diagnostic: OperationDiagnostic {
                    code: error_codes::LOGGER_QUEUE_FULL,
                    message: "writer queue is full; level change diagnostic was not admitted"
                        .to_string(),
                    remediation: Remediation::recoverable(
                        "reduce logging pressure or increase queue capacity",
                        ["inspect logger.health().queue_depth"],
                    ),
                    at: Timestamp::now_utc(),
                },
            },
            Err(TryEnqueueError::Disconnected) => ChangeDiagnostic::NotAccepted {
                diagnostic: OperationDiagnostic {
                    code: error_codes::LOGGER_WRITER_DEGRADED,
                    message: "writer is unavailable; level change diagnostic was not admitted"
                        .to_string(),
                    remediation: Remediation::recoverable(
                        "inspect logger writer-thread health",
                        ["recreate the logger instance"],
                    ),
                    at: Timestamp::now_utc(),
                },
            },
        }
    }
}

fn level_rank(level: LevelFilter) -> u8 {
    match level {
        LevelFilter::Trace => 0,
        LevelFilter::Debug => 1,
        LevelFilter::Info => 2,
        LevelFilter::Warn => 3,
        LevelFilter::Error => 4,
        LevelFilter::Off => 5,
    }
}

fn unavailable_level_error(message: &str) -> LevelChangeError {
    LevelChangeError::Unavailable {
        diagnostic: unavailable_level_diagnostic(message),
    }
}

fn unavailable_level_diagnostic(message: &str) -> OperationDiagnostic {
    OperationDiagnostic {
        code: ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE"),
        message: message.to_string(),
        remediation: Remediation::not_recoverable("inspect state and create a new logger"),
        at: Timestamp::now_utc(),
    }
}

fn unavailable_event_error(message: &str) -> EventError {
    EventError(Box::new(ErrorContext::new(
        ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE"),
        message,
        Remediation::not_recoverable("inspect state and create a new logger"),
    )))
}

impl LoggerRuntime {
    pub(crate) fn diagnostic_admitter(&self) -> DiagnosticAdmitter {
        self.writer
            .as_ref()
            .expect("running logger must retain its writer runtime")
            .diagnostic_admitter()
    }

    pub(crate) fn new(
        query_available: bool,
        sinks: Vec<crate::SinkRegistration>,
        file_sink: Option<Arc<JsonlFileSink>>,
        retained_log_policy: RetainedLogPolicy,
        queue_capacity: usize,
        #[cfg(test)] test_pass_delay: Option<Duration>,
        #[cfg(test)] test_pass_signal: Option<Arc<crate::maintenance::TestPassDelaySignal>>,
    ) -> Self {
        Self::try_new(
            query_available,
            sinks,
            file_sink,
            retained_log_policy,
            queue_capacity,
            #[cfg(test)]
            test_pass_delay,
            #[cfg(test)]
            test_pass_signal,
        )
        .expect("existing infallible logger construction expects writer thread startup")
    }

    pub(crate) fn try_new(
        query_available: bool,
        sinks: Vec<crate::SinkRegistration>,
        file_sink: Option<Arc<JsonlFileSink>>,
        retained_log_policy: RetainedLogPolicy,
        queue_capacity: usize,
        #[cfg(test)] test_pass_delay: Option<Duration>,
        #[cfg(test)] test_pass_signal: Option<Arc<crate::maintenance::TestPassDelaySignal>>,
    ) -> Result<Self, InitError> {
        let dropped_events_total = Arc::new(AtomicU64::new(0));
        let flush_errors_total = Arc::new(AtomicU64::new(0));
        let last_error = Arc::new(Mutex::new(None));
        let query_health = Arc::new(QueryHealthTracker::new(if query_available {
            QueryHealthState::Healthy
        } else {
            QueryHealthState::Unavailable
        }));

        let writer = WriterRuntime::try_new(
            sinks,
            file_sink,
            retained_log_policy,
            queue_capacity,
            dropped_events_total.clone(),
            last_error.clone(),
            #[cfg(test)]
            test_pass_delay,
            #[cfg(test)]
            test_pass_signal,
        )
        .map_err(|error| {
            InitError(Box::new(
                ErrorContext::new(
                    error_codes::LOGGER_INIT_FAILED,
                    "failed to start logger writer thread",
                    Remediation::recoverable(
                        "inspect the operating system thread-resource limits",
                        ["retry logger construction after resources are available"],
                    ),
                )
                .source(Box::new(error)),
            ))
        })?;

        Ok(Self {
            dropped_events_total: dropped_events_total.clone(),
            flush_errors_total,
            last_error: last_error.clone(),
            query_health,
            writer: Some(writer),
            writer_snapshot: Mutex::new(None),
        })
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

    /// Creates a logger together with weak authority for runtime level changes.
    pub fn new_with_level_owner(
        config: crate::LoggerConfig,
    ) -> Result<(Self, LevelOwner), sc_observability_types::InitError> {
        LoggerBuilder::new(config)?.build_with_level_owner()
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
        self.try_log_with_outcome(event).map(|_| ())
    }

    /// Attempts non-blocking admission and reports whether level policy filtered the event.
    ///
    /// # Panics
    ///
    /// Panics if the running logger has lost its writer runtime unexpectedly.
    pub fn try_log_with_outcome(&self, event: LogEvent) -> Result<AdmissionOutcome, TryLogError> {
        let event = self
            .prepare_event(event)
            .map_err(TryLogError::InvalidEvent)?;
        let Some(event) = event else {
            return Ok(AdmissionOutcome::Filtered);
        };

        let writer = self
            .runtime
            .writer
            .as_ref()
            .expect("running logger must retain its writer runtime");
        match writer.enqueue_nonblocking(event) {
            Ok(()) => Ok(AdmissionOutcome::Accepted),
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

    /// Drains queued events, waits for the writer thread to finish, and returns
    /// a stopped logger typestate.
    ///
    /// If shutdown exceeds the configured timeout threshold, the runtime records
    /// degraded health before continuing to wait for definitive writer-thread
    /// completion.
    ///
    /// # Panics
    ///
    /// Panics if the internal writer-snapshot mutex has been poisoned while
    /// recording final runtime state.
    pub fn shutdown(mut self) -> Logger<Stopped> {
        self.shutdown.store(true, Ordering::SeqCst);
        self.mark_level_stopping();
        // The owner only has a weak reference to this admission path. Drop the
        // logger's strong reference before joining so it cannot retain sender.
        self.diagnostic_admitter.take();
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
        self.mark_level_stopped();
        Logger {
            config: self.config,
            sinks: self.sinks,
            shutdown: self.shutdown,
            runtime: self.runtime,
            diagnostic_admitter: None,
            level_control: self.level_control,
            state: std::marker::PhantomData,
        }
    }

    fn prepare_event(&self, event: LogEvent) -> Result<Option<LogEvent>, EventError> {
        validate_event(&event, &self.config.service_name)?;
        // Filtering and mutation share this short critical section. Redaction,
        // queue waits, and writer work are intentionally outside it.
        let control = self
            .level_control
            .lock()
            .map_err(|_| unavailable_event_error("logger level state is unavailable"))?;
        if !level_enabled(control.state.effective_level, event.level) {
            return Ok(None);
        }
        drop(control);
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

    fn mark_level_stopping(&self) {
        if let Ok(mut control) = self.level_control.lock()
            && control.lifecycle == LevelLifecycle::Running
        {
            control.lifecycle = LevelLifecycle::Stopping;
        }
    }

    fn mark_level_stopped(&self) {
        if let Ok(mut control) = self.level_control.lock() {
            control.lifecycle = LevelLifecycle::Stopped;
        }
    }
}

impl<State> Logger<State> {
    /// Returns a coherent snapshot of the logger's runtime level state.
    #[must_use]
    pub fn level_state(&self) -> LevelState {
        self.level_control
            .lock()
            .map(|control| control.state)
            .unwrap_or(LevelState {
                configured_level: self.config.level,
                effective_level: self.config.level,
                revision: 0,
            })
    }
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

impl LevelOwner {
    /// Changes the logger's effective level without changing its configured baseline.
    pub fn elevate_level(
        &mut self,
        level: LevelFilter,
        source: LevelChangeSource,
    ) -> Result<LevelChange, LevelChangeError> {
        self.change_level(level, source)
    }

    /// Removes the runtime override and restores the configured baseline.
    pub fn reset_level(
        &mut self,
        source: LevelChangeSource,
    ) -> Result<LevelChange, LevelChangeError> {
        let control = self.control.upgrade().ok_or(LevelChangeError::Stopped)?;
        let level = control
            .lock()
            .map_err(|_| unavailable_level_error("logger level state is unavailable"))?
            .state
            .configured_level;
        self.change_level(level, source)
    }

    fn change_level(
        &mut self,
        level: LevelFilter,
        source: LevelChangeSource,
    ) -> Result<LevelChange, LevelChangeError> {
        let control = self.control.upgrade().ok_or(LevelChangeError::Stopped)?;
        let mut control = control
            .lock()
            .map_err(|_| unavailable_level_error("logger level state is unavailable"))?;
        match control.lifecycle {
            LevelLifecycle::Stopping => return Err(LevelChangeError::Stopping),
            LevelLifecycle::Stopped => return Err(LevelChangeError::Stopped),
            LevelLifecycle::Running => {}
        }
        let previous = control.state;
        if level_rank(level) > level_rank(previous.configured_level) {
            return Err(LevelChangeError::BelowBaseline {
                requested: level,
                configured: previous.configured_level,
            });
        }
        if previous.effective_level == level {
            return Ok(LevelChange::Unchanged { state: previous });
        }
        let revision = previous
            .revision
            .checked_add(1)
            .ok_or_else(|| unavailable_level_error("logger level revision is exhausted"))?;
        control.state = LevelState {
            effective_level: level,
            revision,
            ..previous
        };
        let current = control.state;
        // This is a non-blocking bounded admission attempt. It bypasses only
        // the level threshold and never waits for writer or sink work.
        let diagnostic = control.admit_change_diagnostic(previous, current, source);
        drop(control);
        Ok(LevelChange::Changed {
            previous,
            current,
            source,
            diagnostic,
        })
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
