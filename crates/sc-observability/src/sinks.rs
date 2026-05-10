use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sc_observability_types::{
    Diagnostic, DiagnosticSummary, ErrorContext, Level, LogEvent, LogSinkError, Remediation,
    SinkHealth, SinkHealthState, SinkName, Timestamp,
};
#[cfg(feature = "fault-injection")]
use std::sync::Arc;

use crate::{
    LogSink, RetentionPolicy, RotationPolicy, constants, error_codes, rotated_log_path,
};

#[expect(
    missing_debug_implementations,
    reason = "file-sink internals include mutex-protected runtime state that is intentionally not exposed through a public Debug contract"
)]
/// Built-in JSONL file sink with rotation and retention handling.
pub struct JsonlFileSink {
    path: PathBuf,
    rotation: RotationPolicy,
    retention: RetentionPolicy,
    health: Mutex<SinkHealth>,
}

impl JsonlFileSink {
    /// Creates a JSONL file sink at the given active log path.
    ///
    /// # Panics
    ///
    /// Panics only if the workspace-owned `JSONL_FILE_SINK_NAME` constant ever
    /// becomes invalid for `SinkName`, which would indicate a programming bug.
    pub fn new(path: PathBuf, rotation: RotationPolicy, retention: RetentionPolicy) -> Self {
        Self {
            path,
            rotation,
            retention,
            health: Mutex::new(SinkHealth {
                name: SinkName::new(constants::JSONL_FILE_SINK_NAME)
                    .expect("jsonl sink constant is valid"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
        }
    }

    /// Returns the active JSONL file path for the sink.
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the helper preserves a Result-shaped internal API so rotation checks can grow I/O failure propagation without reshaping caller control flow"
    )]
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

    pub(crate) fn rotated_path(&self, index: u32) -> PathBuf {
        rotated_log_path(&self.path, index)
    }

    fn prune_old_files(&self) {
        let Some(parent) = self.path.parent() else {
            return;
        };

        let Ok(entries) = fs::read_dir(parent) else {
            return;
        };
        let retention_cutoff = SystemTime::now()
            - Duration::from_secs(u64::from(self.retention.max_age_days) * constants::SECS_PER_DAY);

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
            .and_then(|()| file.flush())
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

pub(crate) trait ConsoleWriter: Send + Sync {
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

struct StderrConsoleWriter;

impl ConsoleWriter for StderrConsoleWriter {
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(line.as_bytes())?;
        stderr.write_all(b"\n")?;
        stderr.flush()
    }
}

#[expect(
    missing_debug_implementations,
    reason = "console sink owns a trait-object writer and sink health state, so a derived Debug impl would not be stable or useful"
)]
/// Built-in sink that renders log events to a configured output stream
/// (stdout or stderr).
pub struct ConsoleSink {
    writer: Box<dyn ConsoleWriter>,
    health: Mutex<SinkHealth>,
}

impl ConsoleSink {
    /// Creates a console sink backed by stdout.
    pub fn stdout() -> Self {
        Self::from_writer(Box::new(StdoutConsoleWriter))
    }

    /// Creates a console sink backed by stderr.
    pub fn stderr() -> Self {
        Self::from_writer(Box::new(StderrConsoleWriter))
    }

    pub(crate) fn from_writer(writer: Box<dyn ConsoleWriter>) -> Self {
        Self {
            writer,
            health: Mutex::new(SinkHealth {
                name: SinkName::new(constants::CONSOLE_SINK_NAME)
                    .expect("console sink constant is valid"),
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

#[cfg(feature = "fault-injection")]
#[derive(Clone, Default)]
#[expect(
    missing_debug_implementations,
    reason = "fault injector state is a small validation-only mutex wrapper without a useful stable Debug contract"
)]
/// Public controller for forcing retained sink health into validation states.
///
/// This type is intended for live validation and test harness use only. It
/// wraps one existing retained sink and forces that sink to report degraded or
/// unavailable health through the ordinary `LoggingHealthReport` path without
/// sabotaging the filesystem or reaching into crate-private internals.
pub struct RetainedSinkFaultInjector {
    forced_state: Arc<Mutex<Option<SinkHealthState>>>,
}

#[cfg(feature = "fault-injection")]
impl RetainedSinkFaultInjector {
    /// Creates a new injector with no forced sink fault.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps one retained sink so its health can be forced during validation.
    ///
    /// `Arc<dyn LogSink>` is intentionally preserved in this public signature
    /// because `SinkRegistration::new()` takes `Arc<dyn LogSink>` and this
    /// helper exists solely to compose with that registration surface.
    pub fn wrap(&self, sink: Arc<dyn LogSink>) -> Arc<dyn LogSink> {
        Arc::new(FaultInjectingSink {
            inner: sink,
            forced_state: self.forced_state.clone(),
        })
    }

    /// Forces the wrapped retained sink into the degraded-dropping state.
    pub fn force_degraded(&self) {
        self.set_state(SinkHealthState::DegradedDropping);
    }

    /// Forces the wrapped retained sink into the unavailable state.
    pub fn force_unavailable(&self) {
        self.set_state(SinkHealthState::Unavailable);
    }

    /// Clears any forced sink fault and returns the wrapped sink to normal
    /// health reporting.
    ///
    /// # Panics
    ///
    /// Panics if the retained-sink fault-state mutex has been poisoned.
    pub fn clear(&self) {
        *self
            .forced_state
            .lock()
            .expect("retained sink fault state poisoned") = None;
    }

    fn set_state(&self, state: SinkHealthState) {
        *self
            .forced_state
            .lock()
            .expect("retained sink fault state poisoned") = Some(state);
    }
}

#[cfg(feature = "fault-injection")]
pub(crate) struct FaultInjectingSink {
    inner: Arc<dyn LogSink>,
    forced_state: Arc<Mutex<Option<SinkHealthState>>>,
}

#[cfg(feature = "fault-injection")]
impl FaultInjectingSink {
    fn current_state(&self) -> Option<SinkHealthState> {
        *self
            .forced_state
            .lock()
            .expect("retained sink fault state poisoned")
    }
}

#[cfg(feature = "fault-injection")]
impl LogSink for FaultInjectingSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        if let Some(state) = self.current_state() {
            return Err(LogSinkError(Box::new(fault_injection_error_context(state))));
        }
        self.inner.write(event)
    }

    fn flush(&self) -> Result<(), LogSinkError> {
        if let Some(state) = self.current_state() {
            return Err(LogSinkError(Box::new(fault_injection_error_context(state))));
        }
        self.inner.flush()
    }

    fn health(&self) -> SinkHealth {
        let mut health = self.inner.health();
        if let Some(state) = self.current_state() {
            let context = fault_injection_error_context(state);
            health.state = state;
            health.last_error = Some(DiagnosticSummary::from(context.diagnostic()));
        }
        health
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

#[cfg(feature = "fault-injection")]
fn fault_injection_error_context(state: SinkHealthState) -> ErrorContext {
    let forced_state = match state {
        SinkHealthState::Healthy => "healthy",
        SinkHealthState::DegradedDropping => "degraded_dropping",
        SinkHealthState::Unavailable => "unavailable",
    };
    ErrorContext::new(
        error_codes::LOGGER_SINK_FAULT_INJECTED,
        format!("retained sink fault injection forced {forced_state} state"),
        Remediation::recoverable(
            "clear the retained sink fault injector before resuming normal validation traffic",
            ["clear the injector state"],
        ),
    )
    .detail(
        "forced_state",
        serde_json::Value::String(forced_state.to_string()),
    )
}

fn rename_if_present(src: &Path, dest: &Path) -> std::io::Result<()> {
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}
