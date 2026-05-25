use std::borrow::Cow;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

use sc_observability_types::{
    Diagnostic, DiagnosticSummary, ErrorContext, Level, LogEvent, LogSinkError, Remediation,
    SinkHealth, SinkHealthState, SinkName, Timestamp,
};
#[cfg(feature = "fault-injection")]
use std::sync::{Arc, Mutex};

use crate::{
    LogSink, RetainedLogPolicy, RetentionMaxAge, RetentionPolicy, RotationPolicy, constants,
    error_codes, rotated_log_path,
};

#[expect(
    missing_debug_implementations,
    reason = "file-sink internals include mutex-protected runtime state that is intentionally not exposed through a public Debug contract"
)]
/// Built-in JSONL file sink with rotation and retention handling.
pub struct JsonlFileSink {
    path: PathBuf,
    health: RwLock<SinkHealth>,
    legacy_policy: Option<LegacyRetentionPolicy>,
}

impl JsonlFileSink {
    /// Creates a JSONL file sink at the given active log path.
    ///
    /// This constructor is the legacy low-level sink surface. It uses
    /// `RotationPolicy` and `RetentionPolicy` directly and does not attach the
    /// logger-owned background retained-log maintenance worker. New code should
    /// prefer `LoggerConfig.retained_log_policy` and `Logger::new(...)`.
    ///
    /// # Panics
    ///
    /// Panics only if the workspace-owned `JSONL_FILE_SINK_NAME` constant ever
    /// becomes invalid for `SinkName`, which would indicate a programming bug.
    pub fn new(path: PathBuf, rotation: RotationPolicy, retention: RetentionPolicy) -> Self {
        Self::with_legacy_policy(
            path,
            Some(LegacyRetentionPolicy {
                rotation,
                retention,
            }),
        )
    }

    pub(crate) fn for_logger(path: PathBuf) -> Self {
        Self::with_legacy_policy(path, None)
    }

    fn with_legacy_policy(path: PathBuf, legacy_policy: Option<LegacyRetentionPolicy>) -> Self {
        Self {
            path,
            health: RwLock::new(SinkHealth {
                name: SinkName::new(constants::JSONL_FILE_SINK_NAME)
                    .expect("jsonl sink constant is valid"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
            legacy_policy,
        }
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
        if let Ok(metadata) = fs::metadata(&self.path)
            && metadata.len().saturating_add(incoming_len) > rotation_max_bytes
        {
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

    #[expect(
        deprecated,
        reason = "legacy RetentionPolicy remains supported for direct JsonlFileSink construction"
    )]
    fn prune_old_files(&self, retention: RetentionPolicy) {
        let Some(parent) = self.path.parent() else {
            return;
        };

        let Ok(entries) = fs::read_dir(parent) else {
            return;
        };
        let retention_cutoff = SystemTime::now()
            - Duration::from_secs(u64::from(retention.max_age_days) * constants::SECS_PER_DAY);

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

    fn prune_retained_files(
        &self,
        rotation_max_files: usize,
        retention_max_age: RetentionMaxAge,
        maintenance_max_work_per_pass: Option<usize>,
    ) -> Result<u64, LogSinkError> {
        let Some(parent) = self.path.parent() else {
            return Ok(0);
        };
        let Ok(entries) = fs::read_dir(parent) else {
            return Ok(0);
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

        if retention_max_age.is_disabled() {
            return Ok(pruned_total);
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
        let message = error.to_string();
        let diagnostic = diagnostic_for_sink_failure(message.clone());
        let mut health = self.health.write().expect("file sink health poisoned");
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
        LogSinkError(Box::new(
            ErrorContext::new(
                error_codes::LOGGER_MAINTENANCE_FAILED,
                "retained-log maintenance failed",
                Remediation::not_recoverable(
                    "retained-log maintenance failure handling is owned by the logger runtime",
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
        if let Some(policy) = self.legacy_policy {
            self.rotate_if_needed(
                policy.rotation.max_bytes.as_u64(),
                policy.rotation.max_files.as_usize(),
                line.len() as u64,
            )?;
            self.prune_old_files(policy.retention);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
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

#[derive(Debug, Clone, Copy)]
struct LegacyRetentionPolicy {
    rotation: RotationPolicy,
    retention: RetentionPolicy,
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
        let message = error.to_string();
        let diagnostic = diagnostic_for_sink_failure(message.clone());
        let mut health = self.health.write().expect("console sink health poisoned");
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

pub(crate) fn diagnostic_for_sink_failure(message: impl Into<Cow<'static, str>>) -> Diagnostic {
    Diagnostic {
        timestamp: Timestamp::now_utc(),
        code: error_codes::LOGGER_SINK_WRITE_FAILED,
        message: message.into().into_owned(),
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
    use sc_observability_types::DiagnosticInfo;
    use sc_observability_types::{
        ActionName, Level, OutcomeLabel, ProcessIdentity, SchemaVersion, ServiceName,
        TargetCategory, constants::OBSERVATION_ENVELOPE_VERSION,
    };
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sc-observability-sinks-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        path
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

    fn join_secs(value: u64) -> crate::MaintenanceJoinTimeout {
        crate::MaintenanceJoinTimeout::new(Duration::from_secs(value))
    }

    #[test]
    fn maintenance_failure_uses_maintenance_error_code() {
        let root = temp_path("maintenance-error");
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
                maintenance_join_timeout: join_secs(5),
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
    fn maintenance_max_work_per_pass_limits_pruning() {
        let root = temp_path("maintenance-budget");
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
                maintenance_join_timeout: join_secs(5),
                maintenance_max_work_per_pass: Some(2),
            })
            .expect("maintenance pass");

        assert_eq!(stats.pruned_files, 2);
        assert!(sink.rotated_path(2).exists());
        assert!(!sink.rotated_path(4).exists());
        assert!(!sink.rotated_path(3).exists());
    }

    #[test]
    fn legacy_write_failures_mark_sink_health() {
        let root = temp_path("legacy-write-error");
        let file_parent = root.join("logs");
        fs::create_dir_all(&root).expect("create root");
        fs::write(&file_parent, "not-a-directory").expect("block parent as file");
        let sink = JsonlFileSink::new(
            file_parent.join("service.log.jsonl"),
            RotationPolicy::default(),
            RetentionPolicy::default(),
        );

        let error = sink.write(&log_event()).expect_err("write failure");

        assert_eq!(
            error.diagnostic().code,
            error_codes::LOGGER_SINK_WRITE_FAILED
        );
        assert_eq!(sink.health().state, SinkHealthState::DegradedDropping);
    }
}
