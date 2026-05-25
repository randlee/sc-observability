use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, ErrorContext, EventError, FlushError, LogQuery, LogSnapshot,
    LoggingHealthReport, LoggingHealthState, MaintenanceHealthReport, MaintenanceWorkerState,
    QueryError, QueryHealthState, Remediation, ShutdownError, SinkHealth, SinkHealthState,
};
use serde_json::Value;

use crate::builder::LoggerBuilder;
use crate::follow::LogFollowSession;
use crate::health::QueryHealthTracker;
use crate::jsonl_reader::JsonlLogReader;
use crate::maintenance::MaintenanceRuntime;
use crate::redact::{redact_bearer_token_text, redact_string_value};
use crate::sinks::JsonlFileSink;
use crate::{
    LogEvent, LogSinkError, Logger, RetainedLogPolicy, ServiceName, default_log_path, error_codes,
};

pub(crate) struct LoggerRuntime {
    pub(crate) dropped_events_total: AtomicU64,
    pub(crate) flush_errors_total: AtomicU64,
    pub(crate) last_error: Mutex<Option<DiagnosticSummary>>,
    pub(crate) query_health: Arc<QueryHealthTracker>,
    pub(crate) maintenance: Option<MaintenanceRuntime>,
}

impl LoggerRuntime {
    pub(crate) fn new(
        query_available: bool,
        file_sink: Option<Arc<JsonlFileSink>>,
        retained_log_policy: RetainedLogPolicy,
    ) -> Self {
        Self {
            dropped_events_total: AtomicU64::new(0),
            flush_errors_total: AtomicU64::new(0),
            last_error: Mutex::new(None),
            query_health: Arc::new(QueryHealthTracker::new(if query_available {
                QueryHealthState::Healthy
            } else {
                QueryHealthState::Unavailable
            })),
            maintenance: file_sink.map(|sink| MaintenanceRuntime::new(sink, retained_log_policy)),
        }
    }
}

impl Logger {
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

    /// Emits one structured log event through the configured sinks.
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(EventError(Box::new(ErrorContext::new(
                error_codes::LOGGER_SHUTDOWN,
                "logger is shut down",
                Remediation::not_recoverable("create a new logger before emitting"),
            ))));
        }

        validate_event(&event, &self.config.service_name)?;
        let redacted = self.redact_event(event);

        for registration in &self.sinks {
            if registration
                .filter
                .as_ref()
                .is_some_and(|filter| !filter.accepts(&redacted))
            {
                continue;
            }

            if let Err(err) = registration.sink.write(&redacted) {
                self.record_sink_failure(&err);
            }
        }

        Ok(())
    }

    /// Flushes all registered sinks.
    ///
    /// Sink flush failures are absorbed into logger health and counters so the
    /// caller can continue shutdown or health inspection without a secondary
    /// runtime failure.
    ///
    /// # Panics
    ///
    /// Panics if an internal sink-health mutex has been poisoned while one of
    /// the built-in sink implementations is updating its flush state.
    pub fn flush(&self) -> Result<(), FlushError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.flush_registered_sinks();
        Ok(())
    }

    /// Queries the current JSONL log set synchronously using the shared query contract.
    ///
    /// # Panics
    ///
    /// Panics if the internal query-health mutex has been poisoned while the
    /// runtime records the result of this query.
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError> {
        let reader = self.query_reader()?;
        let result = reader.query(query);
        self.runtime.query_health.record_result(&result);
        result
    }

    /// Starts a tail-style follow session from the current end of the visible log set.
    ///
    /// # Panics
    ///
    /// Panics if the internal query-health mutex has been poisoned while the
    /// runtime records the result of this follow-start operation.
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

    /// Shuts the logger down and makes logger-owned query/follow unavailable.
    ///
    /// # Panics
    ///
    /// Panics if an internal sink-health mutex or the internal query-health
    /// mutex has been poisoned while shutdown is flushing sinks and marking
    /// query/follow unavailable.
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        self.flush_registered_sinks();
        if let Some(maintenance) = &self.runtime.maintenance
            && let Some(summary) = maintenance.shutdown()
        {
            *self
                .runtime
                .last_error
                .lock()
                .expect("logger last_error poisoned") = Some(summary);
        }
        self.runtime.query_health.mark_unavailable(None);
        Ok(())
    }

    /// Returns aggregate logging and query/follow health for the runtime.
    ///
    /// # Panics
    ///
    /// Panics if the internal last-error mutex has been poisoned.
    pub fn health(&self) -> LoggingHealthReport {
        let sink_statuses: Vec<SinkHealth> =
            self.sinks.iter().map(|entry| entry.sink.health()).collect();
        let maintenance = self
            .runtime
            .maintenance
            .as_ref()
            .map(MaintenanceRuntime::snapshot);
        LoggingHealthReport {
            state: aggregate_logging_health_state(&sink_statuses, maintenance.as_ref()),
            dropped_events_total: self.runtime.dropped_events_total.load(Ordering::SeqCst),
            flush_errors_total: self.runtime.flush_errors_total.load(Ordering::SeqCst),
            active_log_path: default_log_path(&self.config.log_root, &self.config.service_name),
            sink_statuses,
            query: Some(self.runtime.query_health.snapshot()),
            maintenance,
            last_error: self
                .runtime
                .last_error
                .lock()
                .expect("logger last_error poisoned")
                .clone(),
        }
    }

    fn flush_registered_sinks(&self) {
        for registration in &self.sinks {
            if let Err(err) = registration.sink.flush() {
                self.record_flush_failure(&err);
            }
        }
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

    fn record_sink_failure(&self, error: &LogSinkError) {
        self.runtime
            .dropped_events_total
            .fetch_add(1, Ordering::SeqCst);
        *self
            .runtime
            .last_error
            .lock()
            .expect("logger last_error poisoned") =
            Some(DiagnosticSummary::from(error.diagnostic()));
    }

    fn record_flush_failure(&self, error: &LogSinkError) {
        self.runtime
            .flush_errors_total
            .fetch_add(1, Ordering::SeqCst);
        *self
            .runtime
            .last_error
            .lock()
            .expect("logger last_error poisoned") =
            Some(DiagnosticSummary::from(error.diagnostic()));
    }

    fn query_reader(&self) -> Result<JsonlLogReader, QueryError> {
        self.ensure_query_available().map(JsonlLogReader::new)
    }

    fn ensure_query_available(&self) -> Result<PathBuf, QueryError> {
        if self.shutdown.load(Ordering::SeqCst) {
            let error = crate::query::shutdown_error();
            self.runtime.query_health.record_error(&error);
            return Err(error);
        }

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
}

fn aggregate_logging_health_state(
    sink_statuses: &[SinkHealth],
    maintenance: Option<&MaintenanceHealthReport>,
) -> LoggingHealthState {
    if sink_statuses
        .iter()
        .any(|sink| sink.state == SinkHealthState::Unavailable)
    {
        LoggingHealthState::Unavailable
    } else if maintenance.is_some_and(|report| report.state == MaintenanceWorkerState::Degraded)
        || sink_statuses
            .iter()
            .any(|sink| sink.state != SinkHealthState::Healthy)
    {
        LoggingHealthState::DegradedDropping
    } else {
        LoggingHealthState::Healthy
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
