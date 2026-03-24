pub mod constants;
pub mod error_codes;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability_types::{
    Diagnostic, EventError, LevelFilter, LogEvent, LogSinkError, LoggingHealthReport,
    LoggingHealthState, ProcessIdentityPolicy, Remediation, ServiceName, SinkHealth,
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

pub struct LoggerConfig {
    pub service_name: ServiceName,
    pub log_root: std::path::PathBuf,
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
    pub fn default_for(service_name: ServiceName, log_root: std::path::PathBuf) -> Self {
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

pub struct Logger {
    config: LoggerConfig,
    sinks: Vec<SinkRegistration>,
    shutdown: AtomicBool,
}

impl Logger {
    pub fn new(config: LoggerConfig) -> Result<Self, sc_observability_types::InitError> {
        Ok(Self {
            config,
            sinks: Vec::new(),
            shutdown: AtomicBool::new(false),
        })
    }

    pub fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(EventError(Box::new(
                sc_observability_types::ErrorContext::new(
                    error_codes::LOGGER_SHUTDOWN,
                    "logger is shut down",
                    Remediation::not_recoverable("create a new logger before emitting"),
                ),
            )));
        }

        for registration in &self.sinks {
            if registration
                .filter
                .as_ref()
                .is_some_and(|filter| !filter.accepts(&event))
            {
                continue;
            }
            let _ = registration.sink.write(&event);
        }

        Ok(())
    }

    pub fn flush(&self) -> Result<(), sc_observability_types::FlushError> {
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), sc_observability_types::ShutdownError> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn health(&self) -> LoggingHealthReport {
        LoggingHealthReport {
            state: LoggingHealthState::Healthy,
            dropped_events_total: 0,
            active_log_path: self
                .config
                .log_root
                .join(self.config.service_name.as_str())
                .join("logs")
                .join(format!("{}.log.jsonl", self.config.service_name.as_str())),
            sink_statuses: Vec::<SinkHealth>::new(),
            last_error: None,
        }
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
