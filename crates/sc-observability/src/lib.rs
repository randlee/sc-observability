pub mod constants;
pub mod error_codes;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use sc_observability_types::{
    Diagnostic, DiagnosticSummary, ErrorContext, FlushError, InitError, Level, LevelFilter,
    LogEvent, LogSinkError, ProcessIdentityPolicy, Remediation, ServiceName, ShutdownError,
};
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

pub struct LoggerConfig {
    pub service_name: ServiceName,
    pub log_root: PathBuf,
    pub level: LevelFilter,
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
    pub retention: RetentionPolicy,
    pub redaction: RedactionPolicy,
    pub process_identity: ProcessIdentityPolicy,
    pub enable_file_sink: bool,
    pub enable_console_sink: bool,
}

impl LoggerConfig {
    pub fn default_for(service_name: ServiceName, log_root: PathBuf) -> Self {
        Self {
            service_name,
            log_root,
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

#[derive(Default)]
struct LoggerRuntime {
    dropped_events_total: AtomicU64,
    last_error: Mutex<Option<DiagnosticSummary>>,
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
        let mut sinks = Vec::new();

        if config.enable_file_sink {
            let sink = JsonlFileSink::new(active_log_path, config.rotation, config.retention);
            sinks.push(SinkRegistration::new(Arc::new(sink)));
        }

        if config.enable_console_sink {
            sinks.push(SinkRegistration::new(Arc::new(ConsoleSink::stdout())));
        }

        Ok(Self {
            config,
            sinks,
            shutdown: AtomicBool::new(false),
            runtime: LoggerRuntime::default(),
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
                self.runtime
                    .dropped_events_total
                    .fetch_add(1, Ordering::SeqCst);
                *self
                    .runtime
                    .last_error
                    .lock()
                    .expect("logger last_error poisoned") = Some(DiagnosticSummary::from(
                    &diagnostic_for_sink_failure(err.to_string()),
                ));
            }
        }

        Ok(())
    }

    pub fn flush(&self) -> Result<(), FlushError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }

        for registration in &self.sinks {
            if let Err(err) = registration.sink.flush() {
                *self
                    .runtime
                    .last_error
                    .lock()
                    .expect("logger last_error poisoned") = Some(DiagnosticSummary::from(
                    &diagnostic_for_sink_failure(err.to_string()),
                ));
                return Err(FlushError(Box::new(
                    ErrorContext::new(
                        error_codes::LOGGER_FLUSH_FAILED,
                        "logger flush failed",
                        Remediation::not_recoverable(
                            "sink flush failure handling is owned by the logger runtime",
                        ),
                    )
                    .cause(err.to_string()),
                )));
            }
        }

        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        self.flush().map_err(|err| ShutdownError(err.0))?;
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
            active_log_path: default_log_path(&self.config.log_root, &self.config.service_name),
            sink_statuses,
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
}

pub struct JsonlFileSink {
    path: PathBuf,
    rotation: RotationPolicy,
    retention: RetentionPolicy,
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
                let src = self.path.with_extension(format!("log.jsonl.{idx}"));
                let dest = self.path.with_extension(format!("log.jsonl.{}", idx + 1));
                if src.exists() {
                    let _ = fs::rename(&src, &dest);
                }
            }
            if self.path.exists() {
                let rotated = self.path.with_extension("log.jsonl.1");
                let _ = fs::rename(&self.path, rotated);
            }
        }

        self.prune_old_files();
        Ok(())
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

    fn mark_failure(&self, message: impl Into<String>) -> LogSinkError {
        let message = message.into();
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
            .cause(message),
        ))
    }
}

impl LogSink for JsonlFileSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| self.mark_failure(err.to_string()))?;
        }

        let mut line =
            serde_json::to_vec(event).map_err(|err| self.mark_failure(err.to_string()))?;
        line.push(b'\n');
        self.rotate_if_needed(line.len() as u64)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| self.mark_failure(err.to_string()))?;
        file.write_all(&line)
            .and_then(|_| file.flush())
            .map_err(|err| self.mark_failure(err.to_string()))?;

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

trait ConsoleWriter: Send {
    fn write_line(&mut self, line: &str) -> std::io::Result<()>;
}

struct StdoutConsoleWriter;

impl ConsoleWriter for StdoutConsoleWriter {
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(line.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()
    }
}

pub struct ConsoleSink {
    writer: Mutex<Box<dyn ConsoleWriter>>,
    health: Mutex<SinkHealth>,
}

impl ConsoleSink {
    pub fn stdout() -> Self {
        Self::from_writer(Box::new(StdoutConsoleWriter))
    }

    pub(crate) fn from_writer(writer: Box<dyn ConsoleWriter>) -> Self {
        Self {
            writer: Mutex::new(writer),
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

    fn mark_failure(&self, message: impl Into<String>) -> LogSinkError {
        let message = message.into();
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
            .cause(message),
        ))
    }
}

impl LogSink for ConsoleSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        let line = Self::format_line(event);
        self.writer
            .lock()
            .expect("console writer poisoned")
            .write_line(&line)
            .map_err(|err| self.mark_failure(err.to_string()))?;
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
    #[allow(dead_code)]
    pub trait Sealed {}
}

#[allow(dead_code)]
pub(crate) trait LogEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError>;
}

impl sealed_emitters::Sealed for Logger {}

impl LogEmitter for Logger {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError> {
        self.emit(event)
    }
}

pub fn diagnostic_for_sink_failure(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
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

fn default_log_path(log_root: &Path, service_name: &ServiceName) -> PathBuf {
    log_root
        .join(service_name.as_str())
        .join("logs")
        .join(format!("{}.log.jsonl", service_name.as_str()))
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
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, ProcessIdentity, TargetCategory,
        Timestamp,
    };
    use serde_json::json;
    use std::sync::Arc;

    struct SharedBuffer {
        lines: Arc<Mutex<Vec<String>>>,
    }

    impl ConsoleWriter for SharedBuffer {
        fn write_line(&mut self, line: &str) -> std::io::Result<()> {
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
    fn shutdown_blocks_future_emits() {
        let root = temp_path("shutdown");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");
        logger.shutdown().expect("shutdown");
        assert!(logger.emit(log_event(service_name())).is_err());
        assert!(logger.flush().is_ok());
        assert!(logger.shutdown().is_ok());
    }
}
