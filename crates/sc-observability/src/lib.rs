//! Lightweight structured logging for the `sc-observability` workspace.
//!
//! This crate owns logging-only concerns: logger configuration, built-in file
//! and console sinks, sink fan-out, filtering, redaction, and logging health.
//! It intentionally avoids typed observation routing and OTLP transport logic.

pub mod constants;
pub mod error_codes;

mod follow;
mod health;
mod jsonl_reader;
mod query;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::health::QueryHealthTracker;
#[doc(inline)]
pub use follow::LogFollowSession;
#[doc(inline)]
pub use jsonl_reader::JsonlLogReader;
use sc_observability_types::{
    Diagnostic, DiagnosticInfo, DiagnosticSummary, ErrorContext, FlushError, InitError, Level,
    LevelFilter, LogEvent, LogQuery, LogSinkError, LogSnapshot, ProcessIdentityPolicy, QueryError,
    QueryHealthState, Remediation, ServiceName, ShutdownError, Timestamp,
};
#[doc(inline)]
pub use sc_observability_types::{
    EventError, LoggingHealthReport, LoggingHealthState, SinkHealth, SinkHealthState,
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPolicy {
    pub max_bytes: u64,
    pub max_files: u32,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_bytes: constants::DEFAULT_ROTATION_MAX_BYTES,
            max_files: constants::DEFAULT_ROTATION_MAX_FILES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub max_age_days: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: constants::DEFAULT_RETENTION_MAX_AGE_DAYS,
        }
    }
}

pub trait Redactor: Send + Sync {
    fn redact(&self, key: &str, value: &mut Value);
}

#[derive(Default)]
pub struct RedactionPolicy {
    pub denylist_keys: Vec<String>,
    pub redact_bearer_tokens: bool,
    pub custom_redactors: Vec<Arc<dyn Redactor>>,
}

impl std::fmt::Debug for RedactionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedactionPolicy")
            .field("denylist_keys", &self.denylist_keys)
            .field("redact_bearer_tokens", &self.redact_bearer_tokens)
            .field("custom_redactors", &self.custom_redactors.len())
            .finish()
    }
}

pub trait LogFilter: Send + Sync {
    fn accepts(&self, event: &LogEvent) -> bool;
}

pub trait LogSink: Send + Sync {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError>;

    fn flush(&self) -> Result<(), LogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth;
}

#[derive(Clone)]
pub struct SinkRegistration {
    pub sink: Arc<dyn LogSink>,
    pub filter: Option<Arc<dyn LogFilter>>,
}

impl SinkRegistration {
    pub fn new(sink: Arc<dyn LogSink>) -> Self {
        Self { sink, filter: None }
    }

    pub fn with_filter(mut self, filter: Arc<dyn LogFilter>) -> Self {
        self.filter = Some(filter);
        self
    }
}

#[derive(Debug)]
pub struct LoggerConfig {
    pub service_name: ServiceName,
    pub log_root: PathBuf,
    pub level: LevelFilter,
    /// Reserved for future async/backpressure implementation. Phase 1 execution is synchronous; this value is stored but not yet applied.
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
    pub retention: RetentionPolicy,
    pub redaction: RedactionPolicy,
    pub process_identity: ProcessIdentityPolicy,
    pub enable_file_sink: bool,
    pub enable_console_sink: bool,
}

impl LoggerConfig {
    /// Builds the documented v1 defaults for a service-scoped logger configuration.
    ///
    /// If `SC_LOG_ROOT` is set, it is used only when `log_root` is empty. A
    /// non-empty `log_root` parameter is treated as explicit configuration and
    /// takes precedence over the environment helper per LOG-009.
    pub fn default_for(service_name: ServiceName, log_root: PathBuf) -> Self {
        let resolved_log_root = if log_root.as_os_str().is_empty() {
            std::env::var("SC_LOG_ROOT")
                .ok()
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(log_root)
        } else {
            log_root
        };
        Self {
            service_name,
            log_root: resolved_log_root,
            level: LevelFilter::Info,
            queue_capacity: constants::DEFAULT_LOG_QUEUE_CAPACITY,
            rotation: RotationPolicy::default(),
            retention: RetentionPolicy::default(),
            redaction: RedactionPolicy {
                redact_bearer_tokens: true,
                ..RedactionPolicy::default()
            },
            process_identity: ProcessIdentityPolicy::Auto,
            enable_file_sink: constants::DEFAULT_ENABLE_FILE_SINK,
            enable_console_sink: constants::DEFAULT_ENABLE_CONSOLE_SINK,
        }
    }
}

struct LoggerRuntime {
    dropped_events_total: AtomicU64,
    flush_errors_total: AtomicU64,
    // MUTEX: sink failures update the shared last_error summary from multiple sinks and call paths;
    // Mutex is sufficient because reads are rare and the value is replaced atomically as one unit.
    last_error: Mutex<Option<DiagnosticSummary>>,
    query_health: Arc<QueryHealthTracker>,
}

impl LoggerRuntime {
    fn new(query_available: bool) -> Self {
        Self {
            dropped_events_total: AtomicU64::new(0),
            flush_errors_total: AtomicU64::new(0),
            last_error: Mutex::new(None),
            query_health: Arc::new(QueryHealthTracker::new(if query_available {
                QueryHealthState::Healthy
            } else {
                QueryHealthState::Unavailable
            })),
        }
    }
}

pub struct Logger {
    config: LoggerConfig,
    sinks: Vec<SinkRegistration>,
    shutdown: AtomicBool,
    runtime: LoggerRuntime,
}

impl Logger {
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let query_available = active_log_path.exists() || config.enable_file_sink;
        let mut sinks = Vec::new();

        if config.enable_file_sink {
            let sink =
                JsonlFileSink::new(active_log_path.clone(), config.rotation, config.retention);
            sinks.push(SinkRegistration::new(Arc::new(sink)));
        }

        if config.enable_console_sink {
            sinks.push(SinkRegistration::new(Arc::new(ConsoleSink::stdout())));
        }

        Ok(Self {
            config,
            sinks,
            shutdown: AtomicBool::new(false),
            runtime: LoggerRuntime::new(query_available),
        })
    }

    pub fn register_sink(&mut self, registration: SinkRegistration) {
        self.sinks.push(registration);
    }

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

    pub fn flush(&self) -> Result<(), FlushError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.flush_registered_sinks();
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
        );
        self.runtime.query_health.record_result(&result);
        result
    }

    fn flush_registered_sinks(&self) {
        for registration in &self.sinks {
            if let Err(err) = registration.sink.flush() {
                self.record_flush_failure(&err);
            }
        }
    }

    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        self.flush_registered_sinks();
        self.runtime.query_health.mark_unavailable(None);
        Ok(())
    }

    pub fn health(&self) -> LoggingHealthReport {
        let sink_statuses: Vec<SinkHealth> =
            self.sinks.iter().map(|entry| entry.sink.health()).collect();
        let degraded = sink_statuses
            .iter()
            .any(|sink| sink.state != SinkHealthState::Healthy);
        LoggingHealthReport {
            state: if degraded {
                LoggingHealthState::DegradedDropping
            } else {
                LoggingHealthState::Healthy
            },
            dropped_events_total: self.runtime.dropped_events_total.load(Ordering::SeqCst),
            flush_errors_total: self.runtime.flush_errors_total.load(Ordering::SeqCst),
            active_log_path: default_log_path(&self.config.log_root, &self.config.service_name),
            sink_statuses,
            query: Some(self.runtime.query_health.snapshot()),
            last_error: self
                .runtime
                .last_error
                .lock()
                .expect("logger last_error poisoned")
                .clone(),
        }
    }

    fn redact_event(&self, mut event: LogEvent) -> LogEvent {
        if self.config.redaction.redact_bearer_tokens {
            if let Some(message) = event.message.as_mut() {
                *message = redact_bearer_token_text(message);
            }
        }

        for (key, value) in &mut event.fields {
            if self
                .config
                .redaction
                .denylist_keys
                .iter()
                .any(|deny| deny == key)
            {
                *value = Value::String(constants::REDACTED_VALUE.to_string());
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
            let error = query::shutdown_error("logger query/follow runtime is shut down");
            self.runtime.query_health.record_error(&error);
            return Err(error);
        }

        if !self.config.enable_file_sink {
            let error = query::unavailable_error(
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

pub struct JsonlFileSink {
    path: PathBuf,
    rotation: RotationPolicy,
    retention: RetentionPolicy,
    // MUTEX: sink health mutates as one small shared struct during writes and health reads;
    // Mutex keeps updates simple and coherent, while RwLock would not reduce contention materially.
    health: Mutex<SinkHealth>,
}

impl JsonlFileSink {
    pub fn new(path: PathBuf, rotation: RotationPolicy, retention: RetentionPolicy) -> Self {
        Self {
            path,
            rotation,
            retention,
            health: Mutex::new(SinkHealth {
                name: "jsonl-file".to_string(),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn rotate_if_needed(&self, incoming_len: u64) -> Result<(), LogSinkError> {
        if let Ok(metadata) = fs::metadata(&self.path)
            && metadata.len().saturating_add(incoming_len) > self.rotation.max_bytes
        {
            for idx in (1..self.rotation.max_files).rev() {
                let src = self.rotated_path(idx);
                let dest = self.rotated_path(idx + 1);
                let _ = rename_if_present(&src, &dest);
            }
            let rotated = self.rotated_path(1);
            let _ = rename_if_present(&self.path, &rotated);
        }

        self.prune_old_files();
        Ok(())
    }

    fn rotated_path(&self, index: u32) -> PathBuf {
        rotated_log_path(&self.path, index)
    }

    fn prune_old_files(&self) {
        let parent = match self.path.parent() {
            Some(parent) => parent,
            None => return,
        };

        let Ok(entries) = fs::read_dir(parent) else {
            return;
        };
        let retention_cutoff =
            SystemTime::now() - Duration::from_secs((self.retention.max_age_days as u64) * 86_400);

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            let active_name = self
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();

            if !file_name.starts_with(active_name) || file_name == active_name {
                continue;
            }

            if let Ok(metadata) = entry.metadata()
                && let Ok(modified) = metadata.modified()
                && modified < retention_cutoff
            {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn mark_failure<E>(&self, error: E) -> LogSinkError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let message = error.to_string();
        let diagnostic = diagnostic_for_sink_failure(message.clone());
        let mut health = self.health.lock().expect("file sink health poisoned");
        health.state = SinkHealthState::DegradedDropping;
        health.last_error = Some(DiagnosticSummary::from(&diagnostic));
        LogSinkError(Box::new(
            ErrorContext::new(
                error_codes::LOGGER_SINK_WRITE_FAILED,
                "jsonl file sink write failed",
                Remediation::not_recoverable(
                    "file sink write failure handling is owned by the logger runtime",
                ),
            )
            .cause(message)
            .source(Box::new(error)),
        ))
    }
}

impl LogSink for JsonlFileSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| self.mark_failure(err))?;
        }

        let mut line = serde_json::to_vec(event).map_err(|err| self.mark_failure(err))?;
        line.push(b'\n');
        self.rotate_if_needed(line.len() as u64)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| self.mark_failure(err))?;
        file.write_all(&line)
            .and_then(|_| file.flush())
            .map_err(|err| self.mark_failure(err))?;

        let mut health = self.health.lock().expect("file sink health poisoned");
        health.state = SinkHealthState::Healthy;
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health
            .lock()
            .expect("file sink health poisoned")
            .clone()
    }
}

trait ConsoleWriter: Send + Sync {
    fn write_line(&self, line: &str) -> std::io::Result<()>;
}

struct StdoutConsoleWriter;

impl ConsoleWriter for StdoutConsoleWriter {
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(line.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()
    }
}

pub struct ConsoleSink {
    writer: Box<dyn ConsoleWriter>,
    // MUTEX: console sink health mutates as one shared status struct on write failures and reads;
    // Mutex is simpler than RwLock because writes dominate and the payload is tiny.
    health: Mutex<SinkHealth>,
}

impl ConsoleSink {
    pub fn stdout() -> Self {
        Self::from_writer(Box::new(StdoutConsoleWriter))
    }

    pub(crate) fn from_writer(writer: Box<dyn ConsoleWriter>) -> Self {
        Self {
            writer,
            health: Mutex::new(SinkHealth {
                name: "console".to_string(),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
        }
    }

    fn format_line(event: &LogEvent) -> String {
        let level = match event.level {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        };
        let message = event.message.as_deref().unwrap_or("");
        format!(
            "{} {} {} {} {}",
            event.timestamp,
            level,
            event.target.as_str(),
            event.action.as_str(),
            message
        )
    }

    fn mark_failure<E>(&self, error: E) -> LogSinkError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let message = error.to_string();
        let diagnostic = diagnostic_for_sink_failure(message.clone());
        let mut health = self.health.lock().expect("console sink health poisoned");
        health.state = SinkHealthState::DegradedDropping;
        health.last_error = Some(DiagnosticSummary::from(&diagnostic));
        LogSinkError(Box::new(
            ErrorContext::new(
                error_codes::LOGGER_SINK_WRITE_FAILED,
                "console sink write failed",
                Remediation::not_recoverable(
                    "console sink write failure handling is owned by the logger runtime",
                ),
            )
            .cause(message)
            .source(Box::new(error)),
        ))
    }
}

impl LogSink for ConsoleSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        let line = Self::format_line(event);
        self.writer
            .write_line(&line)
            .map_err(|err| self.mark_failure(err))?;
        let mut health = self.health.lock().expect("console sink health poisoned");
        health.state = SinkHealthState::Healthy;
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health
            .lock()
            .expect("console sink health poisoned")
            .clone()
    }
}

mod sealed_emitters {
    pub trait Sealed {}
}

#[expect(
    dead_code,
    reason = "crate-local emitter trait is intentionally available for logging-only injection"
)]
pub(crate) trait LogEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError>;
}

impl sealed_emitters::Sealed for Logger {}

impl LogEmitter for Logger {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError> {
        self.emit(event)
    }
}

pub(crate) fn diagnostic_for_sink_failure(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        timestamp: Timestamp::now_utc(),
        code: error_codes::LOGGER_SINK_WRITE_FAILED,
        message: message.into(),
        cause: None,
        remediation: Remediation::not_recoverable(
            "sink failure handling is owned by the logger runtime",
        ),
        docs: None,
        details: serde_json::Map::new(),
    }
}

fn validate_event(event: &LogEvent, expected_service: &ServiceName) -> Result<(), EventError> {
    if event.version != sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION {
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

pub(crate) fn default_log_path(log_root: &Path, service_name: &ServiceName) -> PathBuf {
    log_root
        .join(service_name.as_str())
        .join("logs")
        .join(format!("{}.log.jsonl", service_name.as_str()))
}

pub(crate) fn rotated_log_path(active_path: &Path, index: u32) -> PathBuf {
    let parent = active_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = active_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("active.log.jsonl");
    parent.join(format!("{file_name}.{index}"))
}

fn rename_if_present(src: &Path, dest: &Path) -> std::io::Result<()> {
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn redact_string_value(value: &mut Value) {
    if let Value::String(text) = value {
        *text = redact_bearer_token_text(text);
    }
}

fn redact_bearer_token_text(input: &str) -> String {
    const PREFIX: &str = "Bearer ";
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(PREFIX) {
        result.push_str(&remaining[..index + PREFIX.len()]);
        let token_start = index + PREFIX.len();
        let token_end = remaining[token_start..]
            .find(char::is_whitespace)
            .map(|value| token_start + value)
            .unwrap_or(remaining.len());
        result.push_str(constants::REDACTED_VALUE);
        remaining = &remaining[token_end..];
    }

    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, LogOrder, LogQuery, LogSnapshot,
        ProcessIdentity, QueryError, QueryHealthState, TargetCategory, Timestamp,
    };
    use serde_json::json;
    use std::sync::Arc;
    use temp_env::{with_var, with_var_unset};

    struct SharedBuffer {
        lines: Arc<Mutex<Vec<String>>>,
    }

    impl ConsoleWriter for SharedBuffer {
        fn write_line(&self, line: &str) -> std::io::Result<()> {
            self.lines
                .lock()
                .expect("buffer poisoned")
                .push(line.to_string());
            Ok(())
        }
    }

    struct PrefixRedactor;

    impl Redactor for PrefixRedactor {
        fn redact(&self, key: &str, value: &mut Value) {
            if key == "secret" {
                *value = Value::String("custom-redacted".to_string());
            }
        }
    }

    struct FailSink;

    impl LogSink for FailSink {
        fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
            Err(LogSinkError(Box::new(ErrorContext::new(
                error_codes::LOGGER_SINK_WRITE_FAILED,
                "fail sink write failed",
                Remediation::not_recoverable("test sink intentionally fails"),
            ))))
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: "fail".to_string(),
                state: SinkHealthState::DegradedDropping,
                last_error: None,
            }
        }
    }

    #[derive(Default)]
    struct RecordingFlushSink {
        flush_calls: AtomicU64,
    }

    impl LogSink for RecordingFlushSink {
        fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
            Ok(())
        }

        fn flush(&self) -> Result<(), LogSinkError> {
            self.flush_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: "recording-flush".to_string(),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sc-observability-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn with_sc_log_root<T>(value: Option<&Path>, f: impl FnOnce() -> T) -> T {
        match value {
            Some(path) => with_var("SC_LOG_ROOT", Some(path), f),
            None => with_var_unset("SC_LOG_ROOT", f),
        }
    }

    fn log_event(service_name: ServiceName) -> LogEvent {
        LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name,
            target: TargetCategory::new("logger.core").expect("valid target"),
            action: ActionName::new("emit").expect("valid action"),
            message: Some("Authorization: Bearer abc123".to_string()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some("ok".to_string()),
            diagnostic: Some(Diagnostic {
                timestamp: Timestamp::UNIX_EPOCH,
                code: ErrorCode::new_static("SC_TEST"),
                message: "diagnostic".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect logs"]),
                docs: None,
                details: Default::default(),
            }),
            state_transition: None,
            fields: serde_json::Map::from_iter([
                ("token".to_string(), json!("Bearer secret")),
                ("secret".to_string(), json!("raw")),
            ]),
        }
    }

    fn log_event_with_request(
        service_name: ServiceName,
        request_id: &str,
        message_padding: usize,
    ) -> LogEvent {
        let mut event = log_event(service_name);
        event.message = Some(format!("{request_id} {}", "x".repeat(message_padding)));
        event.request_id = Some(request_id.to_string());
        event
            .fields
            .insert("sequence".to_string(), json!(request_id.to_string()));
        event
    }

    fn query_all(order: LogOrder) -> LogQuery {
        LogQuery {
            order,
            ..LogQuery::default()
        }
    }

    fn request_ids(snapshot: &LogSnapshot) -> Vec<String> {
        snapshot
            .events
            .iter()
            .map(|event| event.request_id.clone().expect("request_id"))
            .collect()
    }

    fn drain_follow_until_request_id(
        follow: &mut LogFollowSession,
        expected_request_id: &str,
    ) -> Vec<String> {
        let mut drained = Vec::new();
        for _ in 0..3 {
            let snapshot = follow.poll().expect("follow poll");
            drained.extend(request_ids(&snapshot));
            if drained
                .iter()
                .any(|request_id| request_id == expected_request_id)
            {
                return drained;
            }
        }

        panic!("follow session never yielded {expected_request_id}");
    }

    #[test]
    fn logger_config_default_for_sets_documented_defaults() {
        let root = temp_path("defaults");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        assert_eq!(config.level, LevelFilter::Info);
        assert_eq!(config.queue_capacity, constants::DEFAULT_LOG_QUEUE_CAPACITY);
        assert_eq!(
            config.rotation.max_bytes,
            constants::DEFAULT_ROTATION_MAX_BYTES
        );
        assert_eq!(
            config.rotation.max_files,
            constants::DEFAULT_ROTATION_MAX_FILES
        );
        assert_eq!(
            config.retention.max_age_days,
            constants::DEFAULT_RETENTION_MAX_AGE_DAYS
        );
        assert!(config.enable_file_sink);
        assert!(!config.enable_console_sink);
        assert_eq!(
            default_log_path(&root, &config.service_name),
            root.join("sc-observability")
                .join("logs")
                .join("sc-observability.log.jsonl")
        );
    }

    #[test]
    fn logger_config_default_for_uses_sc_log_root_when_log_root_is_empty() {
        let env_root = temp_path("env-root");

        with_sc_log_root(Some(&env_root), || {
            let config = LoggerConfig::default_for(service_name(), PathBuf::new());
            assert_eq!(config.log_root, env_root);
        });
    }

    #[test]
    fn logger_config_default_for_prefers_explicit_log_root_over_env() {
        let env_root = temp_path("env-root-override");
        let explicit_root = temp_path("explicit-root");

        with_sc_log_root(Some(&env_root), || {
            let config = LoggerConfig::default_for(service_name(), explicit_root.clone());
            assert_eq!(config.log_root, explicit_root);
        });
    }

    #[test]
    fn logger_config_debug_renders_redaction_summary() {
        let root = temp_path("debug");
        let config = LoggerConfig::default_for(service_name(), root);

        let rendered = format!("{config:?}");

        assert!(rendered.contains("LoggerConfig"));
        assert!(rendered.contains("RedactionPolicy"));
        assert!(rendered.contains("custom_redactors: 0"));
    }

    #[test]
    fn file_only_logging_writes_jsonl_to_default_path() {
        let root = temp_path("file-only");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        let logger = Logger::new(config).expect("logger");
        logger.emit(log_event(service_name())).expect("emit");

        let path = root
            .join("sc-observability")
            .join("logs")
            .join("sc-observability.log.jsonl");
        let contents = fs::read_to_string(&path).expect("read log file");
        assert!(contents.contains("\"level\":\"Info\""));
        assert!(contents.contains("[REDACTED]"));
    }

    #[test]
    fn file_and_console_fan_out_both_receive_event() {
        let root = temp_path("fanout");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.enable_console_sink = false;
        let mut logger = Logger::new(config).expect("logger");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        logger.register_sink(SinkRegistration::new(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));

        logger.emit(log_event(service_name())).expect("emit");

        let path = root
            .join("sc-observability")
            .join("logs")
            .join("sc-observability.log.jsonl");
        assert!(path.exists());
        let lines = lines.lock().expect("lines poisoned");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("logger.core"));
    }

    #[test]
    fn redaction_runs_before_sink_fan_out() {
        let root = temp_path("redaction");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.enable_console_sink = false;
        config.redaction.denylist_keys.push("token".to_string());
        config
            .redaction
            .custom_redactors
            .push(Arc::new(PrefixRedactor));
        let mut logger = Logger::new(config).expect("logger");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        logger.register_sink(SinkRegistration::new(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));

        logger.emit(log_event(service_name())).expect("emit");

        let file_path = root
            .join("sc-observability")
            .join("logs")
            .join("sc-observability.log.jsonl");
        let file_contents = fs::read_to_string(file_path).expect("read file");
        let console_line = lines.lock().expect("lines poisoned")[0].clone();
        assert!(file_contents.contains("[REDACTED]"));
        assert!(file_contents.contains("custom-redacted"));
        assert!(console_line.contains("[REDACTED]"));
    }

    #[test]
    fn invalid_event_returns_event_error() {
        let root = temp_path("invalid");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");
        let mut event = log_event(service_name());
        event.version = "v0".to_string();
        assert!(logger.emit(event).is_err());
    }

    #[test]
    fn sink_failures_are_fail_open_and_counted_in_health() {
        let root = temp_path("fail-open");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut logger = Logger::new(config).expect("logger");
        logger.register_sink(SinkRegistration::new(Arc::new(FailSink)));

        logger
            .emit(log_event(service_name()))
            .expect("emit still succeeds");

        let health = logger.health();
        assert_eq!(health.state, LoggingHealthState::DegradedDropping);
        assert_eq!(health.dropped_events_total, 1);
        assert!(health.last_error.is_some());
    }

    #[test]
    fn flush_failures_are_fail_open_and_counted_in_health() {
        struct FlushFailSink;

        impl LogSink for FlushFailSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }

            fn flush(&self) -> Result<(), LogSinkError> {
                Err(LogSinkError(Box::new(ErrorContext::new(
                    error_codes::LOGGER_FLUSH_FAILED,
                    "flush failed",
                    Remediation::not_recoverable("test sink intentionally fails flush"),
                ))))
            }

            fn health(&self) -> SinkHealth {
                SinkHealth {
                    name: "flush-fail".to_string(),
                    state: SinkHealthState::DegradedDropping,
                    last_error: None,
                }
            }
        }

        let root = temp_path("flush-fail");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut logger = Logger::new(config).expect("logger");
        logger.register_sink(SinkRegistration::new(Arc::new(FlushFailSink)));

        logger.flush().expect("flush remains fail-open");

        let health = logger.health();
        assert_eq!(health.dropped_events_total, 0);
        assert_eq!(health.flush_errors_total, 1);
        assert!(health.last_error.is_some());
    }

    #[test]
    fn default_log_path_uses_service_scoped_layout() {
        let service = ServiceName::new("custom-service").expect("valid service");
        let log_root = PathBuf::from("observability-root");

        let path = default_log_path(&log_root, &service);

        assert_eq!(
            path,
            PathBuf::from("observability-root/custom-service/logs/custom-service.log.jsonl")
        );
    }

    #[test]
    fn rotated_log_paths_keep_the_active_filename_prefix() {
        let sink = JsonlFileSink::new(
            PathBuf::from("observability-root/custom-service/logs/custom-service.log.jsonl"),
            RotationPolicy::default(),
            RetentionPolicy::default(),
        );

        assert_eq!(
            sink.rotated_path(1),
            PathBuf::from("observability-root/custom-service/logs/custom-service.log.jsonl.1")
        );
        assert_eq!(
            sink.rotated_path(2),
            PathBuf::from("observability-root/custom-service/logs/custom-service.log.jsonl.2")
        );
    }

    #[test]
    fn sink_filter_blocks_event_delivery() {
        struct DenyAll;

        impl LogFilter for DenyAll {
            fn accepts(&self, _event: &LogEvent) -> bool {
                false
            }
        }

        let root = temp_path("filter");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut logger = Logger::new(config).expect("logger");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        logger.register_sink(
            SinkRegistration::new(Arc::new(ConsoleSink::from_writer(Box::new(SharedBuffer {
                lines: lines.clone(),
            }))))
            .with_filter(Arc::new(DenyAll)),
        );

        logger.emit(log_event(service_name())).expect("emit");

        assert!(lines.lock().expect("lines poisoned").is_empty());
    }

    #[test]
    fn shutdown_blocks_future_emits() {
        let root = temp_path("shutdown");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");
        logger.shutdown().expect("shutdown");
        assert!(logger.emit(log_event(service_name())).is_err());
        assert!(logger.flush().is_ok());
        assert!(logger.shutdown().is_ok());
    }

    #[test]
    fn shutdown_flushes_registered_sinks_before_marking_shutdown() {
        let root = temp_path("shutdown-flush");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut logger = Logger::new(config).expect("logger");
        let sink = Arc::new(RecordingFlushSink::default());
        logger.register_sink(SinkRegistration::new(sink.clone()));

        logger.shutdown().expect("shutdown");

        assert_eq!(sink.flush_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn historical_query_reads_active_and_rotated_files() {
        let root = temp_path("query-rotated");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.rotation.max_bytes = 350;
        config.rotation.max_files = 4;
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 240))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        let resolved_paths = crate::query::query_active_and_rotated_paths(&active_path, 4);
        assert!(
            resolved_paths
                .iter()
                .any(|path| path.ends_with("sc-observability.log.jsonl.1"))
        );
        assert!(
            resolved_paths
                .iter()
                .any(|path| path.ends_with("sc-observability.log.jsonl"))
        );

        let asc = logger
            .query(&query_all(LogOrder::OldestFirst))
            .expect("asc query");
        assert_eq!(request_ids(&asc), ["req-1", "req-2", "req-3"]);

        let desc = logger
            .query(&LogQuery {
                order: LogOrder::NewestFirst,
                limit: Some(2),
                ..LogQuery::default()
            })
            .expect("desc query");
        assert_eq!(request_ids(&desc), ["req-3", "req-2"]);
        assert!(desc.truncated);
    }

    #[test]
    fn logger_and_jsonl_reader_query_have_parity() {
        let root = temp_path("query-parity");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.rotation.max_bytes = 350;
        config.rotation.max_files = 4;
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-a", "req-b", "req-c"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let query = LogQuery {
            order: LogOrder::NewestFirst,
            limit: Some(2),
            ..LogQuery::default()
        };
        let logger_snapshot = logger.query(&query).expect("logger query");
        let reader = JsonlLogReader::new(default_log_path(&root, &service_name()));
        let reader_snapshot = reader.query(&query).expect("reader query");

        assert_eq!(reader_snapshot, logger_snapshot);
    }

    #[test]
    fn follow_starts_at_tail_and_survives_multiple_rotations() {
        let root = temp_path("follow-rotation");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.rotation.max_bytes = 350;
        config.rotation.max_files = 6;
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "backlog", 220))
            .expect("emit backlog");

        let mut follow = logger
            .follow(query_all(LogOrder::OldestFirst))
            .expect("follow");
        assert!(follow.poll().expect("initial poll").events.is_empty());

        for request_id in ["fresh-1", "fresh-2", "fresh-3"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit fresh");
        }

        let snapshot = follow.poll().expect("follow poll");
        assert_eq!(request_ids(&snapshot), ["fresh-1", "fresh-2", "fresh-3"]);
        assert_eq!(follow.health().state, QueryHealthState::Healthy);
    }

    #[test]
    fn logger_and_jsonl_reader_follow_have_parity() {
        let root = temp_path("follow-parity");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.rotation.max_bytes = 350;
        config.rotation.max_files = 6;
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "backlog", 220))
            .expect("emit backlog");

        let query = query_all(LogOrder::OldestFirst);
        let mut logger_follow = logger.follow(query.clone()).expect("logger follow");
        let reader = JsonlLogReader::new(default_log_path(&root, &service_name()));
        let mut reader_follow = reader.follow(query).expect("reader follow");

        for request_id in ["reader-1", "reader-2"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit fresh");
        }

        assert_eq!(
            logger_follow.poll().expect("logger follow poll"),
            reader_follow.poll().expect("reader follow poll")
        );
    }

    #[test]
    fn query_health_tracks_decode_and_shutdown_failures() {
        use std::io::Write as _;

        let root = temp_path("query-health");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "healthy", 20))
            .expect("emit");

        let active_path = default_log_path(&root, &service_name());
        let mut file = OpenOptions::new()
            .append(true)
            .open(&active_path)
            .expect("open active log");
        writeln!(file, "{{not-json").expect("append malformed json");

        let decode_error = logger
            .query(&query_all(LogOrder::OldestFirst))
            .expect_err("decode error");
        assert!(matches!(decode_error, QueryError::Decode(_)));
        let degraded_health = logger.health().query.expect("query health");
        assert_eq!(degraded_health.state, QueryHealthState::Degraded);
        assert!(degraded_health.last_error.is_some());

        logger.shutdown().expect("shutdown");
        let shutdown_error = logger
            .query(&query_all(LogOrder::OldestFirst))
            .expect_err("shutdown error");
        assert!(matches!(shutdown_error, QueryError::Shutdown));
        assert_eq!(
            logger.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
        assert!(matches!(
            logger.follow(query_all(LogOrder::OldestFirst)),
            Err(QueryError::Shutdown)
        ));
    }

    #[test]
    fn logger_query_and_follow_reject_invalid_queries() {
        let root = temp_path("invalid-query");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");

        let invalid_limit = LogQuery {
            limit: Some(0),
            ..LogQuery::default()
        };
        let invalid_range = LogQuery {
            since: Some(Timestamp::now_utc()),
            until: Some(Timestamp::UNIX_EPOCH),
            ..LogQuery::default()
        };

        assert!(matches!(
            logger.query(&invalid_limit),
            Err(QueryError::InvalidQuery(_))
        ));
        assert!(matches!(
            logger.follow(invalid_limit),
            Err(QueryError::InvalidQuery(_))
        ));
        assert!(matches!(
            logger.query(&invalid_range),
            Err(QueryError::InvalidQuery(_))
        ));
        assert!(matches!(
            logger.follow(invalid_range),
            Err(QueryError::InvalidQuery(_))
        ));
    }

    #[test]
    fn query_and_follow_are_unavailable_without_file_sink() {
        let root = temp_path("query-unavailable");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let logger = Logger::new(config).expect("logger");

        assert!(matches!(
            logger.query(&query_all(LogOrder::OldestFirst)),
            Err(QueryError::Unavailable(_))
        ));
        assert!(matches!(
            logger.follow(query_all(LogOrder::OldestFirst)),
            Err(QueryError::Unavailable(_))
        ));
        assert_eq!(
            logger.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
    }

    #[test]
    fn follow_recovers_after_active_file_truncate_and_recreate() {
        let root = temp_path("follow-truncate-recreate");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "backlog", 20))
            .expect("emit backlog");

        let mut follow = logger
            .follow(query_all(LogOrder::OldestFirst))
            .expect("follow");
        assert!(follow.poll().expect("initial poll").events.is_empty());

        logger
            .emit(log_event_with_request(
                service_name(),
                "before-truncate",
                20,
            ))
            .expect("emit before truncate");
        assert_eq!(
            request_ids(&follow.poll().expect("poll before truncate")),
            ["before-truncate"]
        );

        let active_path = default_log_path(&root, &service_name());
        fs::File::create(&active_path).expect("truncate active log");

        logger
            .emit(log_event_with_request(service_name(), "after-truncate", 20))
            .expect("emit after truncate");
        // Windows can replay previously read records once a truncate resets the file position,
        // while Unix platforms often yield only the new post-truncate record.
        let after_truncate = drain_follow_until_request_id(&mut follow, "after-truncate");
        assert!(
            after_truncate == vec!["after-truncate"]
                || after_truncate == vec!["backlog", "before-truncate", "after-truncate"]
        );

        fs::remove_file(&active_path).expect("remove active log");
        logger
            .emit(log_event_with_request(service_name(), "after-recreate", 20))
            .expect("emit after recreate");
        let after_recreate = drain_follow_until_request_id(&mut follow, "after-recreate");
        assert_eq!(after_recreate, vec!["after-recreate"]);
    }
}
