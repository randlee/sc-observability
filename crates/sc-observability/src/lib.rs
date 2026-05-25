//! Lightweight structured logging for the `sc-observability` workspace.
//!
//! This crate owns logging-only concerns: logger configuration, built-in file
//! and console sinks, sink fan-out, filtering, redaction, and logging health.
//! It intentionally avoids typed observation routing and OTLP transport logic.
#![expect(
    clippy::missing_errors_doc,
    reason = "public facade methods already have behavior documented centrally in the workspace docs, and repeating per-method Errors sections here would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "the facade intentionally avoids pervasive must_use boilerplate on constructors and lightweight accessors where the return types are already obvious from call sites"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "builder-style chaining methods in this facade predate pedantic lint adoption and remain intentionally lightweight"
)]

pub mod constants;
pub mod error_codes;

mod builder;
mod follow;
mod health;
mod jsonl_reader;
mod maintenance;
mod query;
mod redact;
mod runtime;
mod sinks;

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[doc(inline)]
pub use builder::LoggerBuilder;
#[doc(inline)]
pub use follow::LogFollowSession;
#[doc(inline)]
pub use jsonl_reader::JsonlLogReader;
#[doc(inline)]
pub use sc_observability_types::{
    ActionName, ErrorCode, EventError, Level, LogEvent, LogQuery, LogSnapshot, LoggingHealthReport,
    LoggingHealthState, MaintenanceHealthReport, MaintenanceWorkerState,
    OBSERVATION_ENVELOPE_VERSION, OutcomeLabel, ProcessIdentity, SchemaVersion, ServiceName,
    SinkHealth, SinkHealthState, TargetCategory, Timestamp,
};
use sc_observability_types::{LevelFilter, LogSinkError, ProcessIdentityPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "fault-injection")]
#[doc(inline)]
pub use sinks::RetainedSinkFaultInjector;
#[doc(inline)]
pub use sinks::{ConsoleSink, JsonlFileSink};

pub(crate) use runtime::LoggerRuntime;

/// Rotation limits for the built-in JSONL file sink.
///
/// This legacy low-level policy is used only by direct `JsonlFileSink::new()`
/// construction. It does not configure the logger-owned background retained-log
/// maintenance worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPolicy {
    /// Maximum size of the active JSONL file before rotation.
    pub max_bytes: ByteCount,
    /// Maximum number of rotated files to retain.
    pub max_files: usize,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_bytes: ByteCount::from_bytes(constants::DEFAULT_ROTATION_MAX_BYTES),
            max_files: constants::DEFAULT_ROTATION_MAX_FILES_USIZE,
        }
    }
}

/// Retention limits for rotated JSONL files owned by the built-in file sink.
///
/// This legacy low-level policy is used only by direct `JsonlFileSink::new()`
/// construction. It does not configure the logger-owned background retained-log
/// maintenance worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Maximum age in days for rotated JSONL files.
    #[deprecated(
        since = "1.1.0",
        note = "Use RetainedLogPolicy::retention_max_age for logger-managed retained-log maintenance."
    )]
    pub max_age_days: u32,
}

impl Default for RetentionPolicy {
    #[expect(
        deprecated,
        reason = "legacy RetentionPolicy remains supported for direct JsonlFileSink construction"
    )]
    fn default() -> Self {
        Self {
            max_age_days: constants::DEFAULT_RETENTION_MAX_AGE_DAYS,
        }
    }
}

/// Strongly typed byte count used by retained-log policy fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ByteCount(u64);

impl ByteCount {
    /// Creates a byte count from a raw byte value.
    pub const fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Creates a byte count from mebibytes.
    pub const fn from_mib(mebibytes: u64) -> Self {
        Self(mebibytes * 1024 * 1024)
    }

    /// Returns the raw byte value.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Strongly typed maintenance pass cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceCadence(#[serde(with = "duration_millis_serde")] Duration);

impl MaintenanceCadence {
    /// Creates a cadence from one duration.
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns the wrapped duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

/// Strongly typed maintenance-worker join timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceJoinTimeout(#[serde(with = "duration_millis_serde")] Duration);

impl MaintenanceJoinTimeout {
    /// Creates a join timeout from one duration.
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns the wrapped duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

/// Retained-log rotation, pruning, and maintenance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedLogPolicy {
    /// Maximum size of the active JSONL file before rotation.
    pub rotation_max_bytes: ByteCount,
    /// Maximum number of rotated files retained beside the active log.
    pub rotation_max_files: usize,
    /// Maximum age of retained rotated files.
    #[serde(with = "duration_millis_serde")]
    pub retention_max_age: Duration,
    /// How often the background maintenance worker runs a pass.
    pub maintenance_cadence: MaintenanceCadence,
    /// How long shutdown waits for the maintenance worker to stop.
    pub maintenance_join_timeout: MaintenanceJoinTimeout,
    /// Optional cap on files processed during one maintenance pass.
    pub maintenance_max_work_per_pass: Option<usize>,
}

impl Default for RetainedLogPolicy {
    fn default() -> Self {
        Self {
            rotation_max_bytes: ByteCount::from_bytes(constants::DEFAULT_ROTATION_MAX_BYTES),
            rotation_max_files: constants::DEFAULT_ROTATION_MAX_FILES_USIZE,
            retention_max_age: constants::DEFAULT_RETENTION_MAX_AGE,
            maintenance_cadence: MaintenanceCadence::new(constants::DEFAULT_MAINTENANCE_CADENCE),
            maintenance_join_timeout: MaintenanceJoinTimeout::new(
                constants::DEFAULT_MAINTENANCE_JOIN_TIMEOUT,
            ),
            maintenance_max_work_per_pass: constants::DEFAULT_MAINTENANCE_MAX_WORK_PER_PASS,
        }
    }
}

mod duration_millis_serde {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(
            value
                .as_millis()
                .try_into()
                .map_err(serde::ser::Error::custom)?,
        )
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Duration::from_millis(u64::deserialize(deserializer)?))
    }
}

/// Redacts one key/value pair before an event reaches registered sinks.
pub trait Redactor: Send + Sync {
    /// Redacts one event field in place.
    fn redact(&self, key: &str, value: &mut Value);
}

/// Redaction settings applied to log events before sink fan-out.
#[derive(Default)]
pub struct RedactionPolicy {
    /// Exact field names that must always be redacted.
    pub denylist_keys: Vec<String>,
    /// Whether bearer-token shaped values should be redacted.
    pub redact_bearer_tokens: bool,
    /// Additional caller-supplied redactors.
    pub custom_redactors: Vec<Box<dyn Redactor>>,
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

/// Filters events before they are written to one registered sink.
pub trait LogFilter: Send + Sync {
    /// Returns whether the sink should receive the event.
    fn accepts(&self, event: &LogEvent) -> bool;
}

/// One concrete event sink used by the logger runtime.
pub trait LogSink: Send + Sync {
    /// Writes one event to the sink.
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError>;

    /// Flushes any buffered sink state.
    fn flush(&self) -> Result<(), LogSinkError> {
        Ok(())
    }

    /// Returns the current sink health snapshot.
    fn health(&self) -> SinkHealth;
}

/// Construction-time sink registration pairing one sink with an optional filter.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "registration stores trait-object sinks and filters, so derived Debug would not provide a meaningful stable contract"
)]
pub struct SinkRegistration {
    /// Concrete sink implementation.
    pub(crate) sink: Arc<dyn LogSink>,
    /// Optional sink-local filter.
    pub(crate) filter: Option<Arc<dyn LogFilter>>,
}

impl SinkRegistration {
    /// Wraps a sink for logger registration.
    pub fn new(sink: Arc<dyn LogSink>) -> Self {
        Self { sink, filter: None }
    }

    /// Adds a sink-local filter to the registration.
    pub fn with_filter(mut self, filter: Arc<dyn LogFilter>) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Public configuration for the lightweight logging runtime.
#[derive(Debug)]
pub struct LoggerConfig {
    /// Stable service name attached to emitted records.
    pub service_name: ServiceName,
    /// Root directory that owns the service log tree.
    pub log_root: PathBuf,
    /// Minimum severity level emitted by the logger.
    pub level: LevelFilter,
    /// Reserved for future async/backpressure implementation. Phase 1 execution is synchronous; this value is stored but not yet applied.
    pub queue_capacity: usize,
    /// Retained-log rotation, pruning, and background maintenance settings.
    pub retained_log_policy: RetainedLogPolicy,
    /// Redaction policy applied before sink fan-out.
    pub redaction: RedactionPolicy,
    /// Process identity policy for emitted records.
    pub process_identity: ProcessIdentityPolicy,
    /// Whether the built-in JSONL file sink is enabled.
    pub enable_file_sink: bool,
    /// Whether the built-in console sink is enabled.
    pub enable_console_sink: bool,
    #[cfg(test)]
    maintenance_test_pass_delay: Option<Duration>,
}

impl LoggerConfig {
    /// Builds the documented v1 defaults for a service-scoped logger
    /// configuration.
    ///
    /// If [`constants::SC_LOG_ROOT_ENV_VAR`] is set, it is used only when
    /// `log_root` is empty. A non-empty `log_root` parameter is treated as
    /// explicit configuration and takes precedence over the environment helper
    /// per LOG-009.
    pub fn default_for(service_name: ServiceName, log_root: PathBuf) -> Self {
        let resolved_log_root = if log_root.as_os_str().is_empty() {
            std::env::var(constants::SC_LOG_ROOT_ENV_VAR)
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
            retained_log_policy: RetainedLogPolicy::default(),
            redaction: RedactionPolicy {
                redact_bearer_tokens: true,
                ..RedactionPolicy::default()
            },
            process_identity: ProcessIdentityPolicy::Auto,
            enable_file_sink: constants::DEFAULT_ENABLE_FILE_SINK,
            enable_console_sink: constants::DEFAULT_ENABLE_CONSOLE_SINK,
            #[cfg(test)]
            maintenance_test_pass_delay: None,
        }
    }
}

/// Running logger typestate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Running;

/// Stopped logger typestate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stopped;

/// Lightweight structured logging runtime with built-in query and follow support.
#[expect(
    missing_debug_implementations,
    reason = "logger owns runtime handles and trait-object sinks whose internal state is not a stable public debug contract"
)]
pub struct Logger<State = Running> {
    config: LoggerConfig,
    sinks: Vec<SinkRegistration>,
    shutdown: Arc<AtomicBool>,
    runtime: LoggerRuntime,
    state: PhantomData<State>,
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

impl sealed_emitters::Sealed for Logger<Running> {}

impl LogEmitter for Logger<Running> {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError> {
        self.emit(event)
    }
}

pub(crate) fn default_log_file_name(service_name: &ServiceName) -> String {
    format!(
        "{}{}",
        service_name.as_str(),
        constants::DEFAULT_LOG_FILE_SUFFIX
    )
}

pub(crate) fn default_log_path(log_root: &Path, service_name: &ServiceName) -> PathBuf {
    log_root
        .join(constants::DEFAULT_LOG_DIR_NAME)
        .join(default_log_file_name(service_name))
}

pub(crate) fn rotated_log_path(active_path: &Path, index: usize) -> PathBuf {
    let parent = active_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = active_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("active.log.jsonl");
    parent.join(format!("{file_name}.{index}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sinks::ConsoleWriter;
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, ErrorContext, Level, LogEvent, LogOrder, LogQuery,
        LogSnapshot, ProcessIdentity, QueryError, QueryHealthState, Remediation, SinkName,
        TargetCategory, Timestamp,
    };
    use serde_json::{Map, json};
    use std::fs::{self, OpenOptions};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime};
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
                name: sink_name("fail"),
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
                name: sink_name("recording-flush"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn schema_version() -> sc_observability_types::SchemaVersion {
        sc_observability_types::SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version")
    }

    fn outcome_label(value: &str) -> sc_observability_types::OutcomeLabel {
        sc_observability_types::OutcomeLabel::new(value).expect("valid outcome label")
    }

    fn correlation_id(value: &str) -> sc_observability_types::CorrelationId {
        sc_observability_types::CorrelationId::new(value).expect("valid correlation id")
    }

    fn sink_name(value: &str) -> SinkName {
        SinkName::new(value).expect("valid sink name")
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

    #[cfg(unix)]
    fn unix_file_identity(path: &Path) -> crate::query::FileIdentity {
        crate::query::file_identity_for_path(path)
    }

    #[cfg(unix)]
    fn recreate_with_distinct_unix_identity(active_path: &Path) {
        let previous_identity = unix_file_identity(active_path);
        fs::remove_file(active_path).expect("remove active log");

        let active_name = active_path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("active log file name");

        for attempt in 0..256 {
            let replacement =
                active_path.with_file_name(format!("{active_name}.replacement-{attempt}"));
            fs::File::create(&replacement).expect("create replacement file");
            if unix_file_identity(&replacement) != previous_identity {
                fs::rename(&replacement, active_path).expect("install replacement active log");
                return;
            }
            fs::remove_file(&replacement).expect("remove reused replacement inode");
        }

        panic!("failed to create replacement active log with distinct Unix identity");
    }

    #[cfg(not(unix))]
    fn recreate_with_distinct_unix_identity(active_path: &Path) {
        // Non-Unix follow tests only verify that truncate/recreate remains
        // callable. Identity-distinctness is intentionally not asserted here,
        // and both cfg variants must stay behaviorally aligned when this helper
        // changes.
        fs::remove_file(active_path).expect("remove active log");
        fs::File::create(active_path).expect("recreate active log");
    }

    fn with_sc_log_root<T>(value: Option<&Path>, f: impl FnOnce() -> T) -> T {
        match value {
            Some(path) => with_var(constants::SC_LOG_ROOT_ENV_VAR, Some(path), f),
            None => with_var_unset(constants::SC_LOG_ROOT_ENV_VAR, f),
        }
    }

    fn log_event(service_name: ServiceName) -> LogEvent {
        LogEvent {
            version: schema_version(),
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
            outcome: Some(outcome_label("ok")),
            diagnostic: Some(Diagnostic {
                timestamp: Timestamp::UNIX_EPOCH,
                code: ErrorCode::new_static("SC_TEST"),
                message: "diagnostic".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect logs"]),
                docs: None,
                details: Map::default(),
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
        event.request_id = Some(correlation_id(request_id));
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
            .map(|event| event.request_id.clone().expect("request_id").to_string())
            .collect()
    }

    fn drain_follow_until_request_id(
        follow: &mut LogFollowSession,
        expected_request_id: &str,
    ) -> Vec<String> {
        let mut drained = Vec::new();
        for _ in 0..10 {
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

    fn wait_for(mut predicate: impl FnMut() -> bool, message: &str) {
        for _ in 0..100 {
            if predicate() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        panic!("{message}");
    }

    fn bytes(value: u64) -> ByteCount {
        ByteCount::from_bytes(value)
    }

    fn cadence_ms(value: u64) -> MaintenanceCadence {
        MaintenanceCadence::new(Duration::from_millis(value))
    }

    fn cadence_secs(value: u64) -> MaintenanceCadence {
        MaintenanceCadence::new(Duration::from_secs(value))
    }

    fn join_ms(value: u64) -> MaintenanceJoinTimeout {
        MaintenanceJoinTimeout::new(Duration::from_millis(value))
    }

    fn join_secs(value: u64) -> MaintenanceJoinTimeout {
        MaintenanceJoinTimeout::new(Duration::from_secs(value))
    }

    fn existing_log_paths(active_path: &Path, max_files: usize) -> Vec<PathBuf> {
        crate::query::query_active_and_rotated_paths(active_path, max_files)
            .into_iter()
            .filter(|path| path.exists())
            .collect()
    }

    #[test]
    fn logger_config_default_for_sets_documented_defaults() {
        let root = temp_path("defaults");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        assert_eq!(config.level, LevelFilter::Info);
        assert_eq!(config.queue_capacity, constants::DEFAULT_LOG_QUEUE_CAPACITY);
        assert_eq!(
            config.retained_log_policy.rotation_max_bytes,
            ByteCount::from_bytes(constants::DEFAULT_ROTATION_MAX_BYTES)
        );
        assert_eq!(
            config.retained_log_policy.rotation_max_files,
            constants::DEFAULT_ROTATION_MAX_FILES as usize
        );
        assert_eq!(
            config.retained_log_policy.retention_max_age,
            constants::DEFAULT_RETENTION_MAX_AGE
        );
        assert!(config.enable_file_sink);
        assert!(!config.enable_console_sink);
        assert_eq!(
            default_log_path(&root, &config.service_name),
            root.join(constants::DEFAULT_LOG_DIR_NAME)
                .join(default_log_file_name(&config.service_name))
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
    fn retained_log_policy_round_trips_through_serde() {
        let policy = RetainedLogPolicy {
            rotation_max_bytes: bytes(1024),
            rotation_max_files: 7,
            retention_max_age: Duration::from_secs(42),
            maintenance_cadence: cadence_secs(60),
            maintenance_join_timeout: join_secs(5),
            maintenance_max_work_per_pass: Some(3),
        };

        let encoded = serde_json::to_string(&policy).expect("serialize retained-log policy");
        let decoded: RetainedLogPolicy =
            serde_json::from_str(&encoded).expect("deserialize retained-log policy");

        assert_eq!(decoded, policy);
    }

    #[test]
    fn file_only_logging_writes_jsonl_to_default_path() {
        let root = temp_path("file-only");
        let config = LoggerConfig::default_for(service_name(), root.clone());
        let logger = Logger::new(config).expect("logger");
        logger.emit(log_event(service_name())).expect("emit");

        let path = default_log_path(&root, &service_name());
        let contents = fs::read_to_string(&path).expect("read log file");
        assert!(contents.contains("\"level\":\"Info\""));
        assert!(contents.contains("[REDACTED]"));
    }

    #[test]
    fn file_and_console_fan_out_both_receive_event() {
        let root = temp_path("fanout");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.enable_console_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(SinkRegistration::new(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));
        let logger = builder.build();

        logger.emit(log_event(service_name())).expect("emit");

        let path = default_log_path(&root, &service_name());
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
            .push(Box::new(PrefixRedactor));
        let mut builder = Logger::builder(config).expect("logger builder");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(SinkRegistration::new(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));
        let logger = builder.build();

        logger.emit(log_event(service_name())).expect("emit");

        let file_path = default_log_path(&root, &service_name());
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
        event.version = sc_observability_types::SchemaVersion::new("v0").expect("valid version");
        assert!(logger.emit(event).is_err());
    }

    #[test]
    fn sink_failures_are_fail_open_and_counted_in_health() {
        let root = temp_path("fail-open");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::new(Arc::new(FailSink)));
        let logger = builder.build();

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
                    name: sink_name("flush-fail"),
                    state: SinkHealthState::DegradedDropping,
                    last_error: None,
                }
            }
        }

        let root = temp_path("flush-fail");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::new(Arc::new(FlushFailSink)));
        let logger = builder.build();

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
            PathBuf::from("observability-root/logs/custom-service.log.jsonl")
        );
    }

    #[test]
    fn rotated_log_paths_keep_the_active_filename_prefix() {
        let sink = JsonlFileSink::new(
            PathBuf::from("observability-root/logs/custom-service.log.jsonl"),
            RotationPolicy::default(),
            RetentionPolicy::default(),
        );

        assert_eq!(
            sink.rotated_path(1),
            PathBuf::from("observability-root/logs/custom-service.log.jsonl.1")
        );
        assert_eq!(
            sink.rotated_path(2),
            PathBuf::from("observability-root/logs/custom-service.log.jsonl.2")
        );
    }

    #[test]
    fn console_sink_stderr_constructor_is_operational() {
        let sink = ConsoleSink::stderr();

        sink.write(&log_event(service_name()))
            .expect("stderr write");

        assert_eq!(sink.health().state, SinkHealthState::Healthy);
    }

    #[cfg(feature = "fault-injection")]
    #[test]
    fn retained_sink_fault_injector_forces_degraded_logging_health() {
        let root = temp_path("fault-degraded");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        let injector = RetainedSinkFaultInjector::new();
        builder.register_sink(SinkRegistration::new(
            injector.wrap(Arc::new(RecordingFlushSink::default())),
        ));
        let logger = builder.build();

        injector.force_degraded();
        logger
            .emit(log_event(service_name()))
            .expect("emit remains fail-open");

        let health = logger.health();
        assert_eq!(health.state, LoggingHealthState::DegradedDropping);
        assert_eq!(
            health.sink_statuses[0].state,
            SinkHealthState::DegradedDropping
        );
        assert_eq!(health.dropped_events_total, 1);
        assert!(health.last_error.is_some());
    }

    #[cfg(feature = "fault-injection")]
    #[test]
    fn retained_sink_fault_injector_forces_unavailable_logging_health() {
        let root = temp_path("fault-unavailable");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        let injector = RetainedSinkFaultInjector::new();
        builder.register_sink(SinkRegistration::new(
            injector.wrap(Arc::new(RecordingFlushSink::default())),
        ));
        let logger = builder.build();

        injector.force_unavailable();
        logger
            .emit(log_event(service_name()))
            .expect("emit remains fail-open");

        let health = logger.health();
        assert_eq!(health.state, LoggingHealthState::Unavailable);
        assert_eq!(health.sink_statuses[0].state, SinkHealthState::Unavailable);
        assert_eq!(health.dropped_events_total, 1);
        assert!(health.last_error.is_some());
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
        let mut builder = Logger::builder(config).expect("logger builder");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(
            SinkRegistration::new(Arc::new(ConsoleSink::from_writer(Box::new(SharedBuffer {
                lines: lines.clone(),
            }))))
            .with_filter(Arc::new(DenyAll)),
        );
        let logger = builder.build();

        logger.emit(log_event(service_name())).expect("emit");

        assert!(lines.lock().expect("lines poisoned").is_empty());
    }

    #[test]
    fn shutdown_blocks_future_emits() {
        let root = temp_path("shutdown");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");
        let stopped = logger.shutdown().expect("shutdown");
        assert_eq!(stopped.health().state, LoggingHealthState::Unavailable);
    }

    #[test]
    fn shutdown_flushes_registered_sinks_before_marking_shutdown() {
        let root = temp_path("shutdown-flush");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        let sink = Arc::new(RecordingFlushSink::default());
        builder.register_sink(SinkRegistration::new(sink.clone()));
        let logger = builder.build();

        let _stopped = logger.shutdown().expect("shutdown");

        assert_eq!(sink.flush_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn maintenance_health_reflects_last_pass_and_last_error() {
        let root = temp_path("maintenance-health");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 2;
        config.retained_log_policy.retention_max_age = Duration::from_secs(0);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "req-1", 220))
            .expect("emit 1");
        logger
            .emit(log_event_with_request(service_name(), "req-2", 220))
            .expect("emit 2");

        wait_for(
            || {
                logger
                    .health()
                    .maintenance
                    .as_ref()
                    .is_some_and(|maintenance| maintenance.last_pass_at.is_some())
            },
            "expected maintenance pass to run",
        );

        let health = logger.health();
        let maintenance = health.maintenance.expect("maintenance health");
        assert_eq!(maintenance.state, MaintenanceWorkerState::Running);
        assert!(maintenance.last_pass_at.is_some());
        assert!(maintenance.rotated_files_total >= 1);

        let active_path = default_log_path(&root, &service_name());
        let blocked_rotation_path = active_path.with_file_name(format!(
            "{}.2",
            active_path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("active log file name")
        ));
        if blocked_rotation_path.exists() {
            fs::remove_file(&blocked_rotation_path).expect("remove retained file");
        }
        fs::create_dir(&blocked_rotation_path).expect("create blocking retained directory");

        logger
            .emit(log_event_with_request(service_name(), "req-3", 400))
            .expect("emit after blocking retained path");

        wait_for(
            || {
                logger
                    .health()
                    .maintenance
                    .as_ref()
                    .is_some_and(|maintenance| maintenance.last_error.is_some())
            },
            "expected maintenance error to be recorded",
        );

        let degraded = logger.health().maintenance.expect("maintenance health");
        assert_eq!(degraded.state, MaintenanceWorkerState::Degraded);
        assert!(degraded.last_error.is_some());
    }

    #[test]
    fn maintenance_prunes_retained_files_by_max_files() {
        let root = temp_path("maintenance-prune-max-files");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 2;
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let logger = Logger::new(config).expect("logger");

        for request_id in [
            "req-1", "req-2", "req-3", "req-4", "req-5", "req-6", "req-7",
        ] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for(
            || {
                let paths = existing_log_paths(&active_path, 8);
                paths
                    .iter()
                    .any(|path| path.ends_with("sc-observability.log.jsonl"))
                    && !paths
                        .iter()
                        .any(|path| path.ends_with("sc-observability.log.jsonl.3"))
            },
            "expected retained files to be pruned to the configured max_files budget",
        );
    }

    #[test]
    fn maintenance_prunes_retained_files_by_age() {
        let root = temp_path("maintenance-prune-age");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 4;
        config.retained_log_policy.retention_max_age = Duration::from_millis(10);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3", "req-4", "req-5"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }
        std::thread::sleep(Duration::from_millis(30));

        let active_path = default_log_path(&root, &service_name());
        wait_for(
            || {
                let paths = existing_log_paths(&active_path, 8);
                logger
                    .health()
                    .maintenance
                    .as_ref()
                    .is_some_and(|maintenance| {
                        maintenance.rotated_files_total >= 1
                            && maintenance.pruned_files_total >= 1
                            && paths.len() == 1
                    })
            },
            "expected stale retained files to be pruned by age",
        );
    }

    #[test]
    fn shutdown_joins_maintenance_worker_within_timeout() {
        let root = temp_path("shutdown-joins-maintenance");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.retained_log_policy.maintenance_join_timeout = join_secs(1);
        config.maintenance_test_pass_delay = Some(Duration::from_millis(100));
        let logger = Logger::new(config).expect("logger");

        logger.emit(log_event(service_name())).expect("emit");
        wait_for(
            crate::maintenance::test_pass_delay_active,
            "expected maintenance worker to enter the delayed test pass",
        );

        let started = Instant::now();
        let stopped = logger.shutdown().expect("shutdown");
        crate::maintenance::clear_test_pass_delay();

        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            stopped
                .health()
                .maintenance
                .expect("maintenance health")
                .state,
            MaintenanceWorkerState::Stopped
        );
    }

    #[test]
    fn shutdown_records_join_timeout_without_blocking() {
        let root = temp_path("shutdown-maintenance-timeout");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.retained_log_policy.maintenance_join_timeout = join_ms(20);
        config.maintenance_test_pass_delay = Some(Duration::from_millis(200));
        let logger = Logger::new(config).expect("logger");

        logger.emit(log_event(service_name())).expect("emit");
        wait_for(
            crate::maintenance::test_pass_delay_active,
            "expected maintenance worker to enter the delayed test pass",
        );

        let started = Instant::now();
        let stopped = logger.shutdown().expect("shutdown");
        crate::maintenance::clear_test_pass_delay();

        assert!(started.elapsed() < Duration::from_millis(150));
        let maintenance = stopped.health().maintenance.expect("maintenance health");
        assert_eq!(maintenance.state, MaintenanceWorkerState::Degraded);
        assert!(maintenance.last_error.is_some());
    }

    #[test]
    fn emit_path_remains_available_during_maintenance_pass() {
        let root = temp_path("maintenance-nonblocking");
        let mut config = LoggerConfig::default_for(service_name(), root);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.maintenance_test_pass_delay = Some(Duration::from_millis(200));
        let logger = Logger::new(config).expect("logger");

        logger.emit(log_event(service_name())).expect("emit");
        wait_for(
            crate::maintenance::test_pass_delay_active,
            "expected maintenance worker to enter the delayed test pass",
        );

        let started = Instant::now();
        logger
            .emit(log_event_with_request(service_name(), "during-pass", 10))
            .expect("emit during delayed maintenance pass");
        crate::maintenance::clear_test_pass_delay();

        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn historical_query_reads_active_and_rotated_files() {
        let root = temp_path("query-rotated");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 4;
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 240))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for(
            || existing_log_paths(&active_path, 4).len() > 1,
            "expected rotation to produce retained files",
        );
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

        let mut asc = None;
        wait_for(
            || match logger.query(&query_all(LogOrder::OldestFirst)) {
                Ok(snapshot) if request_ids(&snapshot) == ["req-1", "req-2", "req-3"] => {
                    asc = Some(snapshot);
                    true
                }
                _ => false,
            },
            "expected oldest-first query to settle after asynchronous rotation",
        );
        let asc = asc.expect("captured oldest-first snapshot");
        assert_eq!(request_ids(&asc), ["req-1", "req-2", "req-3"]);

        wait_for(
            || {
                logger
                    .query(&LogQuery {
                        order: LogOrder::NewestFirst,
                        limit: Some(2),
                        ..LogQuery::default()
                    })
                    .map(|snapshot| request_ids(&snapshot) == ["req-3", "req-2"])
                    .unwrap_or(false)
            },
            "expected newest-first query to settle after asynchronous rotation",
        );
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
    fn historical_query_preserves_order_across_multiple_rotated_files() {
        let root = temp_path("query-multi-rotation-order");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 6;
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3", "req-4", "req-5"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let mut oldest_first = None;
        wait_for(
            || match logger.query(&query_all(LogOrder::OldestFirst)) {
                Ok(snapshot)
                    if request_ids(&snapshot) == ["req-1", "req-2", "req-3", "req-4", "req-5"] =>
                {
                    oldest_first = Some(snapshot);
                    true
                }
                _ => false,
            },
            "expected query view to settle across multiple retained files before asserting order",
        );
        let oldest_first = oldest_first.expect("captured oldest-first snapshot");
        assert_eq!(
            request_ids(&oldest_first),
            ["req-1", "req-2", "req-3", "req-4", "req-5"]
        );

        let newest_first = logger
            .query(&LogQuery {
                order: LogOrder::NewestFirst,
                ..LogQuery::default()
            })
            .expect("newest-first query");
        assert_eq!(
            request_ids(&newest_first),
            ["req-5", "req-4", "req-3", "req-2", "req-1"]
        );
    }

    #[test]
    fn logger_and_jsonl_reader_query_have_parity() {
        let root = temp_path("query-parity");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 4;
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-a", "req-b", "req-c"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for(
            || existing_log_paths(&active_path, 4).len() > 1,
            "expected retained files before parity query",
        );

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
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 6;
        config.retained_log_policy.maintenance_cadence = cadence_secs(3600);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "backlog", 20))
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

        let mut snapshot = None;
        wait_for(
            || match follow.poll() {
                Ok(polled)
                    if request_ids(&polled)
                        .iter()
                        .any(|request_id| request_id.starts_with("fresh-")) =>
                {
                    snapshot = Some(polled);
                    true
                }
                Ok(_) | Err(_) => false,
            },
            "expected follow poll to observe fresh events after asynchronous maintenance",
        );
        let snapshot = snapshot.expect("captured follow snapshot");
        let followed = request_ids(&snapshot);
        assert!(
            followed == vec!["fresh-1", "fresh-2", "fresh-3"]
                || followed == vec!["backlog", "fresh-1", "fresh-2", "fresh-3"]
        );
        assert_eq!(follow.health().state, QueryHealthState::Healthy);
    }

    #[test]
    fn rotation_triggers_when_active_file_exceeds_max_bytes() {
        let root = temp_path("rotation-threshold");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 4;
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "req-1", 260))
            .expect("emit first");
        logger
            .emit(log_event_with_request(service_name(), "req-2", 260))
            .expect("emit second");

        let active_path = default_log_path(&root, &service_name());
        wait_for(
            || active_path.exists() && rotated_log_path(&active_path, 1).exists(),
            "expected active log rotation to create a .1 retained file",
        );
    }

    #[test]
    fn logger_and_jsonl_reader_follow_have_parity() {
        let root = temp_path("follow-parity");
        let mut config = LoggerConfig::default_for(service_name(), root.clone());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = 6;
        config.retained_log_policy.maintenance_cadence = cadence_secs(3600);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "backlog", 20))
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

        let logger_events = drain_follow_until_request_id(&mut logger_follow, "reader-2");
        let reader_events = drain_follow_until_request_id(&mut reader_follow, "reader-2");

        assert_eq!(logger_events, ["reader-1", "reader-2"]);
        assert_eq!(reader_events, logger_events);
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

        let stopped = logger.shutdown().expect("shutdown");
        assert_eq!(
            stopped.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
    }

    #[test]
    fn logger_health_reports_unavailable_after_shutdown() {
        let root = temp_path("query-shutdown-variant");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");

        let stopped = logger.shutdown().expect("shutdown");

        assert_eq!(
            stopped.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
    }

    #[test]
    fn logger_follow_session_becomes_unavailable_after_shutdown() {
        let root = temp_path("follow-shutdown");
        let config = LoggerConfig::default_for(service_name(), root);
        let logger = Logger::new(config).expect("logger");

        let mut follow = logger
            .follow(query_all(LogOrder::OldestFirst))
            .expect("follow");
        assert!(follow.poll().expect("initial poll").events.is_empty());

        let _stopped = logger.shutdown().expect("shutdown");

        assert!(matches!(follow.poll(), Err(QueryError::Shutdown)));
        assert_eq!(follow.health().state, QueryHealthState::Unavailable);
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

    // This test exercises the Unix-specific replacement helper above. Windows
    // follow identity now uses filesystem identity metadata, but the distinct-
    // inode recreation harness remains Unix-only.
    #[cfg_attr(windows, ignore)]
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
        let truncate_health = follow.health();
        assert_eq!(truncate_health.state, QueryHealthState::Degraded);
        assert!(
            truncate_health
                .last_error
                .expect("truncate health summary")
                .message
                .contains("truncation")
        );

        recreate_with_distinct_unix_identity(&active_path);
        logger
            .emit(log_event_with_request(service_name(), "after-recreate", 20))
            .expect("emit after recreate");
        let after_recreate = drain_follow_until_request_id(&mut follow, "after-recreate");
        assert_eq!(after_recreate, vec!["after-recreate"]);
        let recreate_health = follow.health();
        assert_eq!(recreate_health.state, QueryHealthState::Degraded);
        assert!(
            recreate_health
                .last_error
                .expect("recreate health summary")
                .message
                .contains("identity changed")
        );
    }
}
