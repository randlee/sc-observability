use std::borrow::Cow;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

use sc_observability_types::ErrorContext;
use sc_observability_types::v2::{EventError, InitError, LogSinkError};
use sc_observability_types::{
    Diagnostic, DiagnosticSummary, Level, LogEvent, Remediation, SinkHealth, SinkHealthState,
    SinkName, Timestamp,
};

/// Serializes an event without allowing its JSON payload to exceed the
/// configured per-event limit. The returned bytes exclude the JSONL newline.
pub(crate) fn serialize_event_bounded(event: &LogEvent) -> Result<Vec<u8>, Box<ErrorContext>> {
    let mut writer = BoundedEventWriter {
        bytes: Vec::new(),
        max_bytes: constants::MAX_LOG_EVENT_BYTES - 1,
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut writer, event) {
        if writer.exceeded {
            return Err(event_too_large_context());
        }
        return Err(Box::new(
            ErrorContext::new(
                error_codes::LOGGER_INVALID_EVENT,
                "log event could not be serialized",
                Remediation::recoverable(
                    "provide a serializable event and retry logging",
                    ["check structured event fields for unsupported values"],
                ),
            )
            .cause(error.to_string())
            .source(Box::new(error)),
        ));
    }
    Ok(writer.bytes)
}

pub(crate) fn validate_event_size(event: &LogEvent) -> Result<(), EventError> {
    serialize_event_bounded(event)
        .map(drop)
        .map_err(|context| EventError::Validation { context })
}

fn event_too_large_context() -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        error_codes::LOGGER_INVALID_EVENT,
        format!(
            "serialized log event exceeds the {} byte limit",
            constants::MAX_LOG_EVENT_BYTES
        ),
        Remediation::recoverable(
            "reduce the serialized event size before logging",
            ["shorten the message or remove structured fields"],
        ),
    ))
}

struct BoundedEventWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    exceeded: bool,
}

impl Write for BoundedEventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(new_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("serialized log event size overflowed"));
        };
        if new_len > self.max_bytes {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialized log event exceeds its byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(feature = "fault-injection")]
use std::sync::{Arc, Mutex};

use crate::sink::LogSink;
use crate::{RetainedLogPolicy, RetentionMaxAge, constants, error_codes, rotated_log_path};

#[expect(
    missing_debug_implementations,
    reason = "file-sink internals include mutex-protected runtime state that is intentionally not exposed through a public Debug contract"
)]
/// Built-in JSONL file sink with rotation and retention handling.
pub struct JsonlFileSink {
    path: PathBuf,
    // MUTEX: sink operations update health while callers snapshot it; existing expect sites intentionally panic on poison.
    health: RwLock<SinkHealth>,
    #[cfg(feature = "v1")]
    pub(crate) legacy_policy: Option<RetainedLogPolicy>,
}

impl JsonlFileSink {
    pub(crate) fn for_logger(path: PathBuf) -> Self {
        Self {
            path,
            health: RwLock::new(SinkHealth {
                name: SinkName::new(constants::JSONL_FILE_SINK_NAME)
                    .expect("jsonl sink constant is valid"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
            #[cfg(feature = "v1")]
            legacy_policy: None,
        }
    }

    /// Opens a retained JSONL file sink and applies its rotation and retention policy.
    ///
    /// This is the canonical direct-file-sink constructor. It uses the same
    /// maintenance path as logger-owned sinks, so an existing active file is
    /// rotated and retained files are pruned before the caller writes.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::Runtime`] if startup rotation or retained-file
    /// pruning fails, such as when the active file cannot be inspected or
    /// rotated, or retained files cannot be read or removed.
    pub fn open(path: PathBuf, policy: RetainedLogPolicy) -> Result<Self, InitError> {
        let sink = Self::for_logger(path);
        sink.perform_maintenance(&policy)
            .map_err(|error| InitError::Runtime {
                context: Box::new(
                    ErrorContext::new(
                        error_codes::LOGGER_INIT_FAILED,
                        "could not prepare retained JSONL file sink",
                        Remediation::recoverable(
                            "fix the log path or retention policy and retry initialization",
                            ["ensure the log directory is writable"],
                        ),
                    )
                    .cause(error.to_string())
                    .source(Box::new(error)),
                ),
            })?;
        Ok(sink)
    }

    /// Returns the active JSONL file path for the sink.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn perform_maintenance(
        &self,
        policy: &RetainedLogPolicy,
    ) -> Result<crate::maintenance::MaintenancePassStats, LogSinkError> {
        let mut stats = crate::maintenance::MaintenancePassStats::default();
        self.rotate_if_needed(
            policy.rotation_max_bytes.as_u64(),
            policy.rotation_max_files.as_usize(),
            0,
        )
        .map(|did_rotate| {
            if did_rotate {
                stats.rotated_files = 1;
            }
        })
        .map_err(|error| self.mark_maintenance_failure(error))?;
        stats.pruned_files = self
            .prune_retained_files(
                policy.rotation_max_files.as_usize(),
                policy.retention_max_age,
                policy.maintenance_max_work_per_pass,
            )
            .map_err(|error| self.mark_maintenance_failure(error))?;
        Ok(stats)
    }

    fn rotate_if_needed(
        &self,
        rotation_max_bytes: u64,
        rotation_max_files: usize,
        incoming_len: u64,
    ) -> Result<bool, LogSinkError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(self.mark_failure(error)),
        };
        if metadata.len().saturating_add(incoming_len) > rotation_max_bytes {
            for idx in (1..rotation_max_files).rev() {
                let src = self.rotated_path(idx);
                let dest = self.rotated_path(idx + 1);
                rename_if_present(&src, &dest).map_err(|err| self.mark_failure(err))?;
            }
            if rotation_max_files == 0 {
                fs::remove_file(&self.path)
                    .or_else(ignore_not_found)
                    .map_err(|err| self.mark_failure(err))?;
            } else {
                let rotated = self.rotated_path(1);
                rename_if_present(&self.path, &rotated).map_err(|err| self.mark_failure(err))?;
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|err| self.mark_failure(err))?;
            return Ok(true);
        }

        Ok(false)
    }

    pub(crate) fn rotated_path(&self, index: usize) -> PathBuf {
        rotated_log_path(&self.path, index)
    }

    fn prune_retained_files(
        &self,
        rotation_max_files: usize,
        retention_max_age: RetentionMaxAge,
        maintenance_max_work_per_pass: Option<usize>,
    ) -> Result<u64, LogSinkError> {
        let Some(parent) = self.path.parent() else {
            return Ok(0);
        };
        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(self.mark_failure(error)),
        };

        let mut retained_files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|err| self.mark_failure(err))?;
            let path = entry.path();
            let Some(index) = rotated_index_for_path(&self.path, &path) else {
                continue;
            };
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok());
            retained_files.push(RetainedFile {
                path,
                index,
                modified,
            });
        }

        let mut pruned_total = 0_u64;
        let mut remaining_budget = maintenance_max_work_per_pass.unwrap_or(usize::MAX);

        retained_files.sort_by(|left, right| right.index.cmp(&left.index));
        for retained in retained_files
            .iter()
            .filter(|retained| retained.index > rotation_max_files)
        {
            if remaining_budget == 0 {
                return Ok(pruned_total);
            }
            fs::remove_file(&retained.path)
                .or_else(ignore_not_found)
                .map_err(|err| self.mark_failure(err))?;
            pruned_total += 1;
            remaining_budget -= 1;
        }

        let retention_cutoff = SystemTime::now() - retention_max_age.as_duration();
        retained_files.sort_by_key(|retained| retained.modified.unwrap_or(SystemTime::UNIX_EPOCH));
        for retained in retained_files
            .iter()
            .filter(|retained| retained.index <= rotation_max_files)
        {
            if remaining_budget == 0 {
                break;
            }
            let Some(modified) = retained.modified else {
                continue;
            };
            if modified >= retention_cutoff {
                continue;
            }
            fs::remove_file(&retained.path)
                .or_else(ignore_not_found)
                .map_err(|err| self.mark_failure(err))?;
            pruned_total += 1;
            remaining_budget -= 1;
        }

        Ok(pruned_total)
    }

    fn mark_failure<E>(&self, error: E) -> LogSinkError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        sink_write_failure(
            &self.health,
            "file sink health poisoned",
            "jsonl file sink write failed",
            error,
        )
    }

    fn mark_maintenance_failure(&self, error: LogSinkError) -> LogSinkError {
        let message = error.to_string();
        let diagnostic = Diagnostic {
            timestamp: Timestamp::now_utc(),
            code: error_codes::LOGGER_MAINTENANCE_FAILED,
            message: message.clone(),
            cause: None,
            remediation: Remediation::not_recoverable(
                "retained-log maintenance failure handling is owned by the logger runtime",
            ),
            docs: None,
            details: serde_json::Map::new(),
        };
        let mut health = self.health.write().expect("file sink health poisoned");
        health.state = SinkHealthState::DegradedDropping;
        health.last_error = Some(DiagnosticSummary::from(&diagnostic));
        LogSinkError::Write {
            context: Box::new(
                ErrorContext::new(
                    error_codes::LOGGER_MAINTENANCE_FAILED,
                    "retained-log maintenance failed",
                    Remediation::not_recoverable(
                        "retained-log maintenance failure handling is owned by the logger runtime",
                    ),
                )
                .cause(message)
                .source(Box::new(error)),
            ),
        }
    }
}

impl LogSink for JsonlFileSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        if let Some(parent) = self.path.parent().filter(|_| !is_named_pipe(&self.path)) {
            fs::create_dir_all(parent).map_err(|err| self.mark_failure(err))?;
        }

        let mut line =
            serialize_event_bounded(event).map_err(|context| LogSinkError::Write { context })?;
        line.push(b'\n');
        #[cfg(feature = "v1")]
        if let Some(policy) = self.legacy_policy {
            self.perform_maintenance(&policy)?;
        }

        let mut options = OpenOptions::new();
        options.append(true);
        if !is_named_pipe(&self.path) {
            options.create(true);
        }
        let mut file = options
            .open(&self.path)
            .map_err(|err| self.mark_failure(err))?;
        file.write_all(&line)
            .and_then(|()| file.flush())
            .map_err(|err| self.mark_failure(err))?;

        let mut health = self.health.write().expect("file sink health poisoned");
        health.state = SinkHealthState::Healthy;
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health
            .read()
            .expect("file sink health poisoned")
            .clone()
    }
}

#[cfg(windows)]
fn is_named_pipe(path: &Path) -> bool {
    path.to_string_lossy().starts_with(r"\\.\pipe\")
}

#[cfg(not(windows))]
fn is_named_pipe(_path: &Path) -> bool {
    false
}

#[derive(Debug, Clone)]
struct RetainedFile {
    path: PathBuf,
    index: usize,
    modified: Option<SystemTime>,
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
    // MUTEX: writes update health while callers snapshot it; existing expect sites intentionally panic on poison.
    health: RwLock<SinkHealth>,
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
            health: RwLock::new(SinkHealth {
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
        sink_write_failure(
            &self.health,
            "console sink health poisoned",
            "console sink write failed",
            error,
        )
    }
}

impl LogSink for ConsoleSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        serialize_event_bounded(event).map_err(|context| LogSinkError::Write { context })?;
        let line = Self::format_line(event);
        self.writer
            .write_line(&line)
            .map_err(|err| self.mark_failure(err))?;
        let mut health = self.health.write().expect("console sink health poisoned");
        health.state = SinkHealthState::Healthy;
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health
            .read()
            .expect("console sink health poisoned")
            .clone()
    }
}

#[cfg(feature = "fault-injection")]
#[derive(Clone, Default)]
#[doc(hidden)]
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
    // MUTEX: The controller and cloned fault sink share this forced state; the mutex synchronizes force/clear with sink health checks.
    forced_state: Arc<Mutex<Option<SinkHealthState>>>,
}

#[cfg(feature = "fault-injection")]
impl RetainedSinkFaultInjector {
    /// Creates a new injector with no forced sink fault.
    #[doc(hidden)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forces the wrapped retained sink into the degraded-dropping state.
    #[doc(hidden)]
    pub fn force_degraded(&self) {
        self.set_state(SinkHealthState::DegradedDropping);
    }

    /// Forces the wrapped retained sink into the unavailable state.
    #[doc(hidden)]
    pub fn force_unavailable(&self) {
        self.set_state(SinkHealthState::Unavailable);
    }

    /// Clears any forced sink fault and returns the wrapped sink to normal
    /// health reporting.
    ///
    /// # Panics
    ///
    /// Panics if the retained-sink fault-state mutex has been poisoned.
    #[doc(hidden)]
    pub fn clear(&self) {
        *self
            .forced_state
            .lock()
            .expect("retained sink fault state poisoned") = None;
    }

    /// Builds the canonical fault sink around an already canonical inner sink.
    pub(crate) fn fault_sink(&self, inner: Arc<dyn LogSink>) -> FaultInjectingSink {
        FaultInjectingSink {
            inner,
            forced_state: self.forced_state.clone(),
        }
    }

    /// Wraps a canonical sink so validation can force its reported health.
    #[doc(hidden)]
    pub fn wrap(&self, inner: Arc<dyn LogSink>) -> Arc<dyn LogSink> {
        Arc::new(self.fault_sink(inner))
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
    // MUTEX: The wrapped sink reads this state while its controller may force or clear it; the shared mutex serializes those accesses.
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
            return Err(LogSinkError::Write {
                context: Box::new(fault_injection_error_context(state)),
            });
        }
        self.inner.write(event)
    }

    fn flush(&self) -> Result<(), LogSinkError> {
        if let Some(state) = self.current_state() {
            return Err(LogSinkError::Flush {
                context: Box::new(fault_injection_error_context(state)),
            });
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

pub(crate) fn diagnostic_for_sink_failure(message: impl Into<Cow<'static, str>>) -> Diagnostic {
    Diagnostic {
        timestamp: Timestamp::now_utc(),
        code: error_codes::LOGGER_SINK_WRITE_FAILED,
        message: message.into().into_owned(),
        cause: None,
        remediation: Remediation::not_recoverable(
            "repair or replace the failed standalone sink before retrying the write",
        ),
        docs: None,
        details: serde_json::Map::new(),
    }
}

fn sink_write_failure<E>(
    health: &RwLock<SinkHealth>,
    health_lock_context: &'static str,
    error_message: &'static str,
    error: E,
) -> LogSinkError
where
    E: std::error::Error + Send + Sync + 'static,
{
    let message = error.to_string();
    let diagnostic = diagnostic_for_sink_failure(message.clone());
    let mut health = health.write().expect(health_lock_context);
    health.state = SinkHealthState::DegradedDropping;
    health.last_error = Some(DiagnosticSummary::from(&diagnostic));
    LogSinkError::Write {
        context: Box::new(
            ErrorContext::new(
                error_codes::LOGGER_SINK_WRITE_FAILED,
                error_message,
                Remediation::not_recoverable(
                    "repair or replace the failed standalone sink before retrying the write",
                ),
            )
            .cause(message)
            .source(Box::new(error)),
        ),
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

fn ignore_not_found(error: std::io::Error) -> std::io::Result<()> {
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(())
    } else {
        Err(error)
    }
}

fn rotated_index_for_path(active_path: &Path, candidate: &Path) -> Option<usize> {
    let active_name = active_path.file_name()?.to_str()?;
    let candidate_name = candidate.file_name()?.to_str()?;
    let suffix = candidate_name.strip_prefix(&format!("{active_name}."))?;
    suffix.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileCount, RetentionMaxAge};
    use sc_observability_types::{
        ActionName, Level, OutcomeLabel, ProcessIdentity, SchemaVersion, ServiceName,
        TargetCategory, constants::OBSERVATION_ENVELOPE_VERSION,
    };
    use serde_json::json;
    use std::fs;
    use std::ops::Deref;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    struct TestRoot(tempfile::TempDir);

    struct FailingConsoleWriter;

    impl ConsoleWriter for FailingConsoleWriter {
        fn write_line(&self, _: &str) -> std::io::Result<()> {
            Err(std::io::Error::other("injected console write failure"))
        }
    }

    impl TestRoot {
        fn path_buf(&self) -> PathBuf {
            self.0.path().to_path_buf()
        }
    }

    impl Deref for TestRoot {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            self.0.path()
        }
    }

    fn temp_root(name: &str) -> TestRoot {
        TestRoot(
            tempfile::Builder::new()
                .prefix(&format!("sc-observability-sinks-{name}-"))
                .tempdir()
                .expect("create temporary test root"),
        )
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn log_event() -> LogEvent {
        LogEvent {
            version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid schema"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: TargetCategory::new("logger.core").expect("valid target"),
            action: ActionName::new("emit").expect("valid action"),
            message: Some("rotation test".to_string()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some(OutcomeLabel::new("ok").expect("valid outcome")),
            diagnostic: None,
            state_transition: None,
            fields: serde_json::Map::from_iter([("attempt".to_string(), json!(1))]),
        }
    }

    #[test]
    fn bounded_event_serialization_accepts_exact_limit_and_rejects_one_byte_over() {
        let mut event = log_event();
        event.message = Some(String::new());
        let empty_message_len = serde_json::to_vec(&event)
            .expect("serialize small fixture")
            .len();
        let exact_message_len = constants::MAX_LOG_EVENT_BYTES - 1 - empty_message_len;
        event.message = Some("x".repeat(exact_message_len));

        let serialized = serialize_event_bounded(&event).expect("event exactly at limit");
        assert_eq!(serialized.len() + 1, constants::MAX_LOG_EVENT_BYTES);

        event.message.as_mut().expect("message exists").push('x');
        let error = serialize_event_bounded(&event).expect_err("event exceeds byte limit");
        assert_eq!(error.diagnostic().code, error_codes::LOGGER_INVALID_EVENT);
        assert!(error.diagnostic().message.contains("byte limit"));
    }

    #[test]
    fn oversized_standalone_file_sink_write_does_not_create_or_degrade_sink() {
        let root = temp_root("oversized-event");
        let active_path = root.join("logs/service.log.jsonl");
        let sink = JsonlFileSink::for_logger(active_path.clone());
        let mut event = log_event();
        event.message = Some("x".repeat(constants::MAX_LOG_EVENT_BYTES));

        let error = sink.write(&event).expect_err("oversized event is rejected");

        assert_eq!(error.diagnostic().code, error_codes::LOGGER_INVALID_EVENT);
        assert!(!active_path.exists());
        assert_eq!(sink.health().state, SinkHealthState::Healthy);
    }

    fn bytes(value: u64) -> crate::ByteCount {
        crate::ByteCount::from_bytes(value)
    }

    fn file_count(value: usize) -> FileCount {
        FileCount::from_usize(value)
    }

    fn retention_secs(value: u64) -> RetentionMaxAge {
        RetentionMaxAge::from_duration(Duration::from_secs(value))
    }

    fn cadence_secs(value: u64) -> crate::MaintenanceCadence {
        crate::MaintenanceCadence::new(Duration::from_secs(value))
    }

    fn join_secs(value: u64) -> crate::WriterShutdownTimeout {
        crate::WriterShutdownTimeout::new(Duration::from_secs(value))
    }

    #[test]
    fn open_applies_rotation_and_retention_policy() {
        let root = temp_root("open-policy");
        let active_path = root.join("logs/service.log.jsonl");
        fs::create_dir_all(active_path.parent().expect("parent")).expect("create parent");
        fs::write(&active_path, "active record").expect("seed active file");
        fs::write(
            active_path.with_file_name("service.log.jsonl.1"),
            "previous record",
        )
        .expect("seed retained file");

        let sink = JsonlFileSink::open(
            active_path.clone(),
            RetainedLogPolicy {
                rotation_max_bytes: bytes(1),
                rotation_max_files: file_count(1),
                retention_max_age: retention_secs(3600),
                maintenance_cadence: cadence_secs(60),
                writer_shutdown_timeout: join_secs(5),
                maintenance_max_work_per_pass: None,
            },
        )
        .expect("open applies retained-log policy");

        assert_eq!(sink.path(), active_path);
        assert_eq!(
            fs::read_to_string(sink.rotated_path(1)).expect("rotated file"),
            "active record"
        );
        assert!(
            !sink.rotated_path(2).exists(),
            "retention cap prunes older files"
        );
    }

    #[test]
    fn maintenance_failure_uses_maintenance_error_code() {
        let root = temp_root("maintenance-error");
        let active_path = root.join("logs/service.log.jsonl");
        let sink = JsonlFileSink::for_logger(active_path.clone());
        fs::create_dir_all(active_path.parent().expect("parent")).expect("create parent");
        fs::write(&active_path, "x".repeat(512)).expect("seed active file");
        let blocking_path = active_path.with_file_name("service.log.jsonl.1");
        fs::create_dir(&blocking_path).expect("create blocking rotated directory");
        fs::write(blocking_path.join("keep"), "busy").expect("make blocking directory non-empty");

        let error = sink
            .perform_maintenance(&RetainedLogPolicy {
                rotation_max_bytes: bytes(1),
                rotation_max_files: file_count(1),
                retention_max_age: retention_secs(3600),
                maintenance_cadence: cadence_secs(60),
                writer_shutdown_timeout: join_secs(5),
                maintenance_max_work_per_pass: None,
            })
            .expect_err("maintenance failure");

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_MAINTENANCE_FAILED
        );
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
    }

    #[test]
    fn retained_prune_read_dir_invalid_input_marks_sink_failure() {
        let parent = PathBuf::from("invalid\0log-parent");
        let active_path = parent.join("service.log.jsonl");
        let sink = JsonlFileSink::for_logger(active_path);

        let error = sink
            .prune_retained_files(file_count(1).as_usize(), retention_secs(3600), None)
            .expect_err("read_dir must reject an interior-NUL parent path");

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_SINK_WRITE_FAILED
        );
        let source = std::error::Error::source(error.context())
            .and_then(|source| source.downcast_ref::<io::Error>());
        assert_eq!(
            source.map(io::Error::kind),
            Some(io::ErrorKind::InvalidInput)
        );
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
    }

    #[test]
    fn retained_prune_missing_directory_is_a_healthy_noop() {
        let root = temp_root("maintenance-missing-directory");
        let sink = JsonlFileSink::for_logger(root.join("missing-logs/service.log.jsonl"));

        assert_eq!(
            sink.prune_retained_files(file_count(1).as_usize(), retention_secs(3600), None)
                .expect("missing directory means nothing to prune"),
            0
        );
        let stats = sink
            .perform_maintenance(&RetainedLogPolicy {
                rotation_max_bytes: bytes(u64::MAX),
                rotation_max_files: file_count(1),
                retention_max_age: retention_secs(3600),
                maintenance_cadence: cadence_secs(60),
                writer_shutdown_timeout: join_secs(5),
                maintenance_max_work_per_pass: None,
            })
            .expect("maintenance with no log directory is a healthy no-op");

        assert_eq!(stats.rotated_files, 0);
        assert_eq!(stats.pruned_files, 0);
        assert_eq!(sink.health().state, SinkHealthState::Healthy);
    }

    #[test]
    fn rotation_invalid_path_marks_sink_failure() {
        let sink =
            JsonlFileSink::for_logger(PathBuf::from("invalid\0log-parent/service.log.jsonl"));

        let error = sink
            .rotate_if_needed(u64::MAX, 1, 0)
            .expect_err("metadata must reject an interior-NUL path");

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_SINK_WRITE_FAILED
        );
        let source = std::error::Error::source(error.context())
            .and_then(|source| source.downcast_ref::<io::Error>());
        assert_eq!(
            source.map(io::Error::kind),
            Some(io::ErrorKind::InvalidInput)
        );
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
    }

    #[test]
    fn maintenance_wrapper_maps_sink_failure_to_maintenance_code() {
        let root = temp_root("maintenance-wrapper-error-code");
        let sink = JsonlFileSink::for_logger(root.join("logs/service.log.jsonl"));
        let sink_error = sink.mark_failure(io::Error::new(
            io::ErrorKind::InvalidInput,
            "interior-NUL path fixture",
        ));

        let error = sink.mark_maintenance_failure(sink_error);

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_MAINTENANCE_FAILED
        );
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
    }

    #[test]
    fn rotation_metadata_not_found_is_benign() {
        let root = temp_root("rotation-metadata-not-found");
        let active_path = root.join("logs/service.log.jsonl");
        let sink = JsonlFileSink::for_logger(active_path);

        assert!(
            !sink
                .rotate_if_needed(u64::MAX, 1, 0)
                .expect("missing active file does not require rotation")
        );
        assert_eq!(sink.health().state, SinkHealthState::Healthy);
    }

    #[test]
    fn maintenance_max_work_per_pass_limits_pruning() {
        let root = temp_root("maintenance-budget");
        let active_path = root.join("logs/service.log.jsonl");
        let sink = JsonlFileSink::for_logger(active_path.clone());
        fs::create_dir_all(active_path.parent().expect("parent")).expect("create parent");
        fs::write(&active_path, "").expect("create active");
        for index in 1..=4 {
            fs::write(sink.rotated_path(index), format!("retained-{index}"))
                .expect("create retained file");
        }

        let stats = sink
            .perform_maintenance(&RetainedLogPolicy {
                rotation_max_bytes: bytes(u64::MAX),
                rotation_max_files: file_count(1),
                retention_max_age: retention_secs(3600),
                maintenance_cadence: cadence_secs(60),
                writer_shutdown_timeout: join_secs(5),
                maintenance_max_work_per_pass: Some(2),
            })
            .expect("maintenance pass");

        assert_eq!(stats.pruned_files, 2);
        assert!(sink.rotated_path(2).exists());
        assert!(!sink.rotated_path(4).exists());
        assert!(!sink.rotated_path(3).exists());
    }

    #[test]
    fn built_in_write_failures_mark_sink_health() {
        let root = temp_root("legacy-write-error");
        let file_parent = root.join("logs");
        fs::create_dir_all(root.path_buf()).expect("create root");
        fs::write(&file_parent, "not-a-directory").expect("block parent as file");
        let sink = JsonlFileSink::for_logger(file_parent.join("service.log.jsonl"));

        let error = sink.write(&log_event()).expect_err("write failure");

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_SINK_WRITE_FAILED
        );
        assert_eq!(
            error.diagnostic().remediation,
            Remediation::not_recoverable(
                "repair or replace the failed standalone sink before retrying the write"
            )
        );
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);

        let console = ConsoleSink::from_writer(Box::new(FailingConsoleWriter));
        let console_error = console.write(&log_event()).expect_err("write failure");
        assert_eq!(
            console_error.diagnostic().code,
            error_codes::LOGGER_SINK_WRITE_FAILED
        );
        assert_eq!(
            console_error.diagnostic().remediation,
            error.diagnostic().remediation
        );
        assert!(std::error::Error::source(&console_error).is_some());
        assert_eq!(console.health().state, SinkHealthState::DegradedDropping);
    }
}
