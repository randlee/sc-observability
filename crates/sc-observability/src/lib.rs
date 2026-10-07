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
#![cfg_attr(
    feature = "v1",
    expect(
        deprecated,
        reason = "the crate implements its retained 1.x compatibility facade internally"
    )
)]

pub mod constants;
pub mod error_codes;

use std::marker::PhantomData;

mod builder;
mod follow;
mod health;
mod jsonl_reader;
mod maintenance;
mod query;
mod redact;
mod runtime;
mod settings;
mod sink;
mod sinks;
#[cfg(feature = "v1")]
pub mod v1;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

#[doc(inline)]
pub use builder::SinkRegistrationError;
#[doc(inline)]
pub use follow::LogFollowSession;
#[doc(inline)]
pub use jsonl_reader::JsonlLogReader;
#[doc(inline)]
pub use sc_observability_types::{
    ActionName, AdmissionOutcome, ChangeDiagnostic, Diagnostic, DiagnosticSummary, ErrorCode,
    ErrorContext, FileCount, Level, LevelChange, LevelChangeError, LevelChangeSource, LevelState,
    LogEvent, LogQuery, LogSnapshot, LoggingHealthReport, LoggingHealthState,
    MaintenanceHealthReport, MaintenanceWorkerState, OBSERVATION_ENVELOPE_VERSION,
    OperationDiagnostic, OutcomeLabel, ProcessIdentity, Remediation, SchemaVersion, ServiceName,
    SinkHealth, SinkHealthState, SinkName, TargetCategory, Timestamp, WriterState,
};
#[cfg(feature = "v1")]
pub use v1::typed;
#[cfg(feature = "v1")]
#[doc(inline)]
pub use v1::{LogError, LogSink, TryLogError};
#[cfg(feature = "v1")]
pub use v1::{LogFailure, TryLogFailure};

use sc_observability_types::{LevelFilter, ProcessIdentityPolicy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
#[doc(inline)]
pub use settings::{
    EnvSnapshot, LogRoot, LogSettings, LogSettingsError, LogSettingsInputs, ResolvedLogSettings,
};
#[cfg(feature = "fault-injection")]
#[doc(hidden)]
#[doc(inline)]
pub use sinks::RetainedSinkFaultInjector;
#[doc(inline)]
pub use sinks::{ConsoleSink, JsonlFileSink};

#[cfg(feature = "v1")]
#[doc(inline)]
pub use v1::Logger;
#[cfg(feature = "v1")]
#[doc(inline)]
pub use v1::LoggerBuilder;
#[cfg(feature = "v1")]
#[doc(inline)]
pub use v1::{RetentionPolicy, RotationPolicy};

/// Opt-in canonical logging facade for the compatible transition.
///
/// This namespace exposes the canonical error contracts without creating a
/// second logger runtime.
pub mod v2 {
    #[doc(inline)]
    pub use crate::builder::CanonicalLoggerBuilder as LoggerBuilder;
    #[doc(inline)]
    pub use crate::canonical::Logger;
    #[doc(inline)]
    pub use crate::sink::LogSink;
    #[doc(inline)]
    pub use crate::{
        ConsoleSink, JsonlFileSink, LoggerConfig, RetainedLogPolicy, SinkRegistration,
    };
    #[doc(inline)]
    pub use sc_observability_types::v2::{EventError, FlushError, InitError, LogSinkError};
}

pub(crate) use maintenance::DiagnosticAdmitter;
pub(crate) use runtime::{LevelControl, LoggerRuntime};

/// Strongly typed byte count used by retained-log policy fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
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

impl std::fmt::Display for ByteCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

/// Strongly typed maintenance pass cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintenanceCadence(Duration);

impl MaintenanceCadence {
    /// Creates a cadence from one duration.
    ///
    /// # Panics
    ///
    /// Panics if `duration` is zero.
    pub const fn new(duration: Duration) -> Self {
        assert!(!duration.is_zero(), "MaintenanceCadence must be non-zero");
        Self(duration)
    }

    /// Returns the wrapped duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

impl Serialize for MaintenanceCadence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration_as_millis(self.0))
    }
}

impl<'de> Deserialize<'de> for MaintenanceCadence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer).map_err(|error| {
            serde::de::Error::custom(format!(
                "MaintenanceCadence expects a u64 millisecond count: {error}"
            ))
        })?;
        if millis == 0 {
            return Err(serde::de::Error::custom(
                "MaintenanceCadence must be a non-zero u64 millisecond count",
            ));
        }
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl std::fmt::Display for MaintenanceCadence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", duration_as_millis(self.0))
    }
}

/// Strongly typed maintenance-worker join timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterShutdownTimeout(Duration);

impl WriterShutdownTimeout {
    /// Creates a join timeout from one duration.
    ///
    /// # Panics
    ///
    /// Panics if `duration` is zero.
    pub const fn new(duration: Duration) -> Self {
        assert!(
            !duration.is_zero(),
            "WriterShutdownTimeout must be non-zero"
        );
        Self(duration)
    }

    /// Returns the wrapped duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

impl Serialize for WriterShutdownTimeout {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration_as_millis(self.0))
    }
}

impl<'de> Deserialize<'de> for WriterShutdownTimeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer).map_err(|error| {
            serde::de::Error::custom(format!(
                "WriterShutdownTimeout expects a u64 millisecond count: {error}"
            ))
        })?;
        if millis == 0 {
            return Err(serde::de::Error::custom(
                "WriterShutdownTimeout must be a non-zero u64 millisecond count",
            ));
        }
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl std::fmt::Display for WriterShutdownTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", duration_as_millis(self.0))
    }
}

/// Strongly typed retained-log max age.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionMaxAge(Duration);

impl RetentionMaxAge {
    /// Creates one retention max age from calendar days.
    ///
    /// # Panics
    ///
    /// Panics if `days` is zero.
    pub const fn from_days(days: u64) -> Self {
        assert!(days != 0, "RetentionMaxAge must be non-zero");
        Self(Duration::from_secs(days * constants::SECS_PER_DAY))
    }

    /// Creates one retention max age from an arbitrary duration.
    ///
    /// # Panics
    ///
    /// Panics if `duration` is zero.
    pub const fn from_duration(duration: Duration) -> Self {
        assert!(!duration.is_zero(), "RetentionMaxAge must be non-zero");
        Self(duration)
    }

    /// Returns the wrapped duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

impl Serialize for RetentionMaxAge {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration_as_millis(self.0))
    }
}

impl<'de> Deserialize<'de> for RetentionMaxAge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer).map_err(|error| {
            serde::de::Error::custom(format!(
                "RetentionMaxAge expects a u64 millisecond count: {error}"
            ))
        })?;
        if millis == 0 {
            return Err(serde::de::Error::custom(
                "RetentionMaxAge must be a non-zero u64 millisecond count",
            ));
        }
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl std::fmt::Display for RetentionMaxAge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", duration_as_millis(self.0))
    }
}

/// Retained-log rotation, pruning, and maintenance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedLogPolicy {
    /// Maximum size of the active JSONL file before rotation.
    pub rotation_max_bytes: ByteCount,
    /// Maximum number of rotated files retained beside the active log.
    pub rotation_max_files: FileCount,
    /// Maximum age of retained rotated files.
    pub retention_max_age: RetentionMaxAge,
    /// How often the background maintenance worker runs a pass.
    pub maintenance_cadence: MaintenanceCadence,
    /// The shutdown-duration threshold used to flag degraded shutdown health
    /// before the logger continues waiting for writer-thread completion.
    pub writer_shutdown_timeout: WriterShutdownTimeout,
    /// Optional cap on files processed during one maintenance pass.
    pub maintenance_max_work_per_pass: Option<usize>,
}

impl Default for RetainedLogPolicy {
    fn default() -> Self {
        Self {
            rotation_max_bytes: ByteCount::from_bytes(constants::DEFAULT_ROTATION_MAX_BYTES),
            rotation_max_files: FileCount::from_usize(constants::DEFAULT_ROTATION_MAX_FILES_USIZE),
            retention_max_age: RetentionMaxAge::from_duration(constants::DEFAULT_RETENTION_MAX_AGE),
            maintenance_cadence: MaintenanceCadence::new(constants::DEFAULT_MAINTENANCE_CADENCE),
            writer_shutdown_timeout: WriterShutdownTimeout::new(
                constants::DEFAULT_WRITER_SHUTDOWN_TIMEOUT,
            ),
            maintenance_max_work_per_pass: constants::DEFAULT_MAINTENANCE_MAX_WORK_PER_PASS,
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "policy-bounded durations stay in the millisecond-to-days range, so u128->u64 overflow is unreachable in practice"
)]
fn duration_as_millis(duration: Duration) -> u64 {
    // Policy-bounded durations (ms to days); u128->u64 overflow is unreachable in practice.
    duration.as_millis() as u64
}

/// Redacts one key/value pair before an event reaches registered sinks.
///
/// This trait is intentionally open for downstream implementations. Adding
/// required methods or tightening object-safety guarantees is therefore a
/// semver-significant public API change.
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
///
/// This trait is intentionally open for downstream implementations. Adding
/// required methods or tightening object-safety guarantees is therefore a
/// semver-significant public API change.
pub trait LogFilter: Send + Sync {
    /// Returns whether the sink should receive the event.
    fn accepts(&self, event: &LogEvent) -> bool;
}

/// Construction-time sink registration pairing one sink with an optional filter.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "registration stores trait-object sinks and filters, so derived Debug would not provide a meaningful stable contract"
)]
pub struct SinkRegistration {
    /// Canonical sink implementation stored exactly as registered.
    pub(crate) sink: Arc<dyn crate::sink::LogSink>,
    /// Optional sink-local filter.
    pub(crate) filter: Option<Arc<dyn LogFilter>>,
}

impl SinkRegistration {
    /// Adds a sink-local filter to the registration.
    pub fn with_filter(mut self, filter: Arc<dyn LogFilter>) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// A logger's queue capacity, guaranteed to be greater than zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueCapacity(NonZeroUsize);

impl QueueCapacity {
    /// Creates a queue capacity when `value` is positive.
    #[must_use]
    pub const fn new(value: usize) -> Option<Self> {
        match NonZeroUsize::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the number of records this queue can hold.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl Default for QueueCapacity {
    fn default() -> Self {
        Self::new(constants::DEFAULT_LOG_QUEUE_CAPACITY)
            .expect("default logger queue capacity must be positive")
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
    /// Bounded writer-thread queue capacity for admitted log records.
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
    #[cfg(test)]
    maintenance_test_pass_signal: Option<Arc<crate::maintenance::TestPassDelaySignal>>,
    #[cfg(test)]
    writer_start_should_fail: bool,
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
            #[cfg(test)]
            maintenance_test_pass_signal: None,
            #[cfg(test)]
            writer_start_should_fail: false,
        }
    }
}

/// Running logger typestate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Running;

/// Stopped logger typestate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stopped;

mod canonical {
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    use super::{
        DiagnosticAdmitter, LevelControl, LoggerConfig, LoggerRuntime, PhantomData, Running,
        SinkRegistration,
    };

    #[expect(
        missing_debug_implementations,
        reason = "logger owns runtime handles and trait-object sinks whose internal state is not a stable public debug contract"
    )]
    /// Structured logging runtime with query, follow, and typestate-checked shutdown.
    pub struct Logger<State = Running> {
        pub(crate) config: Arc<LoggerConfig>,
        pub(crate) sinks: Vec<SinkRegistration>,
        pub(crate) shutdown: Arc<AtomicBool>,
        pub(crate) runtime: LoggerRuntime,
        pub(crate) diagnostic_admitter: Option<DiagnosticAdmitter>,
        // MUTEX: logger admission and LevelOwner changes share serialized state; each use keeps its explicit poison policy.
        pub(crate) level_control: Arc<Mutex<LevelControl>>,
        pub(crate) state: PhantomData<State>,
    }
}

pub(crate) use canonical::Logger as CanonicalLogger;

impl<State> CanonicalLogger<State> {
    /// Returns the configured service identity, independent of sink layout.
    #[must_use]
    pub fn service_name(&self) -> &ServiceName {
        &self.config.service_name
    }
}

/// Weak authority for changing one running logger's effective level.
///
/// The owner deliberately retains no writer, sender, or logger handle. Dropping
/// the logger therefore makes subsequent requests return `Stopped`. The opaque
/// public handle is declared here with the rest of the facade types; its
/// private runtime behavior lives in `runtime.rs`, beside the control state it
/// mutates. This keeps the public surface free of runtime implementation types.
#[derive(Debug)]
pub struct LevelOwner {
    control: Weak<Mutex<LevelControl>>,
}

impl LevelOwner {
    pub(crate) fn new(control: &Arc<Mutex<LevelControl>>) -> Self {
        Self {
            control: Arc::downgrade(control),
        }
    }
}

fn writer_degraded_error_context(message: &str) -> ErrorContext {
    ErrorContext::new(
        error_codes::LOGGER_WRITER_DEGRADED,
        message,
        Remediation::recoverable(
            "inspect logger writer-thread health",
            [
                "inspect logger.health().writer_state",
                "inspect logger.health().last_writer_error",
            ],
        ),
    )
}

fn shutdown_timed_out_error_context(message: &str) -> ErrorContext {
    ErrorContext::new(
        error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
        message,
        Remediation::recoverable(
            "wait for the writer thread to recover or recreate the logger",
            [
                "inspect logger.health().last_writer_error",
                "review writer-thread shutdown timing",
            ],
        ),
    )
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
#[cfg_attr(
    feature = "v1",
    expect(
        deprecated,
        reason = "compatibility coverage intentionally exercises Logger::emit() during the deprecation window"
    )
)]
mod tests {
    use super::*;
    #[cfg(not(feature = "v1"))]
    use crate::CanonicalLogger as Logger;

    #[cfg(not(feature = "v1"))]
    trait CanonicalEmit {
        fn emit(&self, event: LogEvent) -> Result<(), CanonicalEventError>;
    }

    #[cfg(not(feature = "v1"))]
    impl CanonicalEmit for Logger {
        fn emit(&self, event: LogEvent) -> Result<(), CanonicalEventError> {
            self.log(event)?;
            self.flush().map_err(|error| CanonicalEventError::Routing {
                context: error.into_context(),
            })
        }
    }
    #[cfg(feature = "v1")]
    use crate::runtime::LevelLifecycle;
    use crate::v2::LogSink;
    use sc_observability_types::v2::LogSinkError;
    use sc_observability_types::v2::{
        EventError as CanonicalEventError, InitError as CanonicalInitError,
    };
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, LogOrder, LogQuery, LogSnapshot,
        ProcessIdentity, ProcessIdentityPolicy, QueryError, QueryHealthState, Remediation,
        SinkName, TargetCategory, Timestamp,
    };
    #[cfg(feature = "v1")]
    use sc_observability_types::{DiagnosticInfo, ErrorContext};
    use serde_json::{Map, json};
    use std::fs::{self, OpenOptions};
    use std::ops::Deref;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use temp_env::{with_var, with_var_unset};

    #[cfg(feature = "v1")]
    fn legacy_sink_error(context: Box<ErrorContext>) -> LogSinkError {
        LogSinkError::Write { context }
    }

    struct SourceRedactor {
        control: Mutex<Option<std::sync::Weak<Mutex<LevelControl>>>>,
        observed_unlocked: AtomicBool,
    }

    impl SourceRedactor {
        fn attach(&self, control: &Arc<Mutex<LevelControl>>) {
            *self.control.lock().expect("redactor control") = Some(Arc::downgrade(control));
        }
    }

    impl Redactor for SourceRedactor {
        fn redact(&self, key: &str, value: &mut Value) {
            if key == "source" {
                let is_unlocked = self
                    .control
                    .lock()
                    .expect("redactor control")
                    .as_ref()
                    .and_then(std::sync::Weak::upgrade)
                    .is_some_and(|control| control.try_lock().is_ok());
                self.observed_unlocked.store(is_unlocked, Ordering::SeqCst);
                *value = Value::String("redacted-source".to_string());
            }
        }
    }

    impl Redactor for Arc<SourceRedactor> {
        fn redact(&self, key: &str, value: &mut Value) {
            (**self).redact(key, value);
        }
    }

    #[derive(Default)]
    struct RecordingFlushSink {
        flush_calls: AtomicU64,
    }

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<LogEvent>>,
    }

    impl LogSink for RecordingEventSink {
        fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
            self.events
                .lock()
                .expect("recording events mutex poisoned")
                .push(event.clone());
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: sink_name("recording-events"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
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

    struct TestRoot(tempfile::TempDir);

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

    fn temp_path(name: &str) -> TestRoot {
        TestRoot(
            tempfile::Builder::new()
                .prefix(&format!("sc-observability-{name}-"))
                .tempdir()
                .expect("create temporary test root"),
        )
    }

    #[cfg(feature = "v1")]
    fn file_identity(path: &Path) -> crate::query::FileIdentity {
        crate::query::file_identity_for_path(path)
    }

    #[cfg(feature = "v1")]
    fn recreate_with_distinct_identity(active_path: &Path) {
        let replacement_path = active_path.with_extension("replacement");
        let previous_identity = file_identity(active_path);
        fs::File::create(&replacement_path).expect("create replacement log");
        let replacement_identity = file_identity(&replacement_path);
        assert_ne!(
            replacement_identity, previous_identity,
            "replacement created while the active log exists has distinct identity"
        );

        // The sink opens the active path only while writing, so removing the
        // path before installing the already-created replacement works on
        // both Unix and Windows without depending on rename-overwrite rules.
        fs::remove_file(active_path).expect("remove active log");
        fs::rename(&replacement_path, active_path).expect("install replacement log");
        assert_ne!(
            file_identity(active_path),
            previous_identity,
            "installed replacement identity remains distinct"
        );
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

    #[test]
    fn settings_json_parse_failure_preserves_serde_source() {
        let snapshot = EnvSnapshot::from_pairs([(
            std::ffi::OsString::from("SC_LOG_ROTATION_MAX_FILES"),
            std::ffi::OsString::from("not-json"),
        )]);
        let error = LogSettings::from_env(
            &snapshot,
            &sc_observability_types::EnvPrefix::new("SC").expect("valid prefix"),
        )
        .expect_err("invalid JSON must fail");

        assert_eq!(error.code(), error_codes::LOG_INVALID_VALUE);
        assert!(
            std::error::Error::source(&error)
                .expect("serde parse source must be preserved")
                .to_string()
                .contains("expected")
        );
    }

    #[test]
    fn settings_level_env_uses_fallible_string_deserialization() {
        let snapshot = EnvSnapshot::from_pairs([(
            std::ffi::OsString::from("SC_LOG_LEVEL"),
            std::ffi::OsString::from("Info"),
        )]);

        let settings = LogSettings::from_env(
            &snapshot,
            &sc_observability_types::EnvPrefix::new("SC").expect("valid prefix"),
        )
        .expect("level string must deserialize");

        assert_eq!(settings.level, Some(LevelFilter::Info));
    }

    #[test]
    fn settings_level_env_preserves_string_deserializer_error() {
        let snapshot = EnvSnapshot::from_pairs([(
            std::ffi::OsString::from("SC_LOG_LEVEL"),
            std::ffi::OsString::from("Verbose"),
        )]);

        let error = LogSettings::from_env(
            &snapshot,
            &sc_observability_types::EnvPrefix::new("SC").expect("valid prefix"),
        )
        .expect_err("invalid level must fail");

        assert_eq!(error.code(), error_codes::LOG_INVALID_VALUE);
        assert!(
            std::error::Error::source(&error)
                .expect("string deserializer source must be preserved")
                .to_string()
                .contains("unknown variant")
        );
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

    /// Upper bound for one test handshake; reached only when a test fails.
    const TEST_WATCHDOG: Duration = Duration::from_secs(30);

    /// Drains a follow session until it yields `expected_request_id`.
    ///
    /// Callers flush the logger first, so the records are already on disk and
    /// each poll advances the session without waiting.
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

    /// Reports every maintenance pass boundary through a non-blocking test signal.
    fn pass_signal(config: &mut LoggerConfig) -> Arc<crate::maintenance::TestPassDelaySignal> {
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        config.maintenance_test_pass_signal = Some(signal.clone());
        signal
    }

    /// Re-checks `predicate` at each maintenance pass boundary until it holds.
    fn wait_for_pass(
        signal: &crate::maintenance::TestPassDelaySignal,
        mut predicate: impl FnMut() -> bool,
        message: &str,
    ) {
        assert!(
            signal.wait_for_state(TEST_WATCHDOG, |_| predicate()),
            "{message}"
        );
    }

    #[cfg(feature = "v1")]
    fn release_test_pass_delay(signal: &Arc<crate::maintenance::TestPassDelaySignal>) {
        signal.release_delay();
        assert!(
            signal.wait_for_state(TEST_WATCHDOG, |signal| !signal.is_active()),
            "expected maintenance worker to leave the released test gate"
        );
        assert!(
            !signal.wait_timed_out(),
            "the normal fixture path must not continue after a test-gate timeout"
        );
    }

    #[cfg(feature = "v1")]
    #[test]
    fn test_pass_delay_timeout_is_explicit_and_bounded() {
        let signal = crate::maintenance::TestPassDelaySignal::default();
        signal.block_delay_until_released();

        assert_eq!(
            signal.wait_until_released_for(Duration::from_millis(10)),
            crate::maintenance::TestPassDelayWait::TimedOut
        );
        assert!(signal.wait_timed_out());
    }

    #[test]
    fn test_pass_signal_condition_wait_has_one_deadline() {
        let signal = crate::maintenance::TestPassDelaySignal::default();
        assert!(!signal.wait_for_state(
            Duration::from_millis(10),
            crate::maintenance::TestPassDelaySignal::is_active
        ));
        assert!(signal.wait_for_state(Duration::ZERO, |signal| !signal.is_active()));
    }

    #[test]
    fn test_pass_signal_condition_wait_observes_release_notification() {
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let waiting = signal.clone();
        let worker = std::thread::spawn(move || waiting.wait_until_released());
        let unwound = std::panic::catch_unwind(move || {
            let _release = signal.release_on_drop();
            panic!("injected failure while maintenance is gated");
        });
        assert!(unwound.is_err());
        assert_eq!(
            worker.join().expect("released waiter"),
            crate::maintenance::TestPassDelayWait::Released
        );
    }

    #[cfg(feature = "v1")]
    #[test]
    fn test_pass_delay_release_guard_unblocks_waiter_on_drop() {
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let release_guard = signal.release_on_drop();
        let waiting_signal = signal.clone();
        let waiter = std::thread::spawn(move || waiting_signal.wait_until_released());

        drop(release_guard);
        assert_eq!(
            waiter.join().expect("test-gate waiter"),
            crate::maintenance::TestPassDelayWait::Released
        );
        assert!(!signal.wait_timed_out());
    }

    fn bytes(value: u64) -> ByteCount {
        ByteCount::from_bytes(value)
    }

    fn file_count(value: usize) -> FileCount {
        FileCount::from_usize(value)
    }

    fn retention_secs(value: u64) -> RetentionMaxAge {
        RetentionMaxAge::from_duration(Duration::from_secs(value))
    }

    fn retention_ms(value: u64) -> RetentionMaxAge {
        RetentionMaxAge::from_duration(Duration::from_millis(value))
    }

    fn cadence_ms(value: u64) -> MaintenanceCadence {
        MaintenanceCadence::new(Duration::from_millis(value))
    }

    fn cadence_secs(value: u64) -> MaintenanceCadence {
        MaintenanceCadence::new(Duration::from_secs(value))
    }

    #[cfg(feature = "v1")]
    fn join_ms(value: u64) -> WriterShutdownTimeout {
        WriterShutdownTimeout::new(Duration::from_millis(value))
    }

    fn join_secs(value: u64) -> WriterShutdownTimeout {
        WriterShutdownTimeout::new(Duration::from_secs(value))
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
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        assert_eq!(config.level, LevelFilter::Info);
        assert_eq!(config.queue_capacity, constants::DEFAULT_LOG_QUEUE_CAPACITY);
        assert_eq!(
            config.retained_log_policy.rotation_max_bytes,
            ByteCount::from_bytes(constants::DEFAULT_ROTATION_MAX_BYTES)
        );
        assert_eq!(
            config.retained_log_policy.rotation_max_files,
            FileCount::from_usize(constants::DEFAULT_ROTATION_MAX_FILES_USIZE)
        );
        assert_eq!(
            config.retained_log_policy.retention_max_age,
            RetentionMaxAge::from_duration(constants::DEFAULT_RETENTION_MAX_AGE)
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
            assert_eq!(config.log_root, env_root.path_buf());
        });
    }

    #[test]
    fn logger_config_default_for_prefers_explicit_log_root_over_env() {
        let env_root = temp_path("env-root-override");
        let explicit_root = temp_path("explicit-root");

        with_sc_log_root(Some(&env_root), || {
            let config = LoggerConfig::default_for(service_name(), explicit_root.path_buf());
            assert_eq!(config.log_root, explicit_root.path_buf());
        });
    }

    #[test]
    fn logger_config_debug_renders_redaction_summary() {
        let root = temp_path("debug");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());

        let rendered = format!("{config:?}");

        assert!(rendered.contains("LoggerConfig"));
        assert!(rendered.contains("RedactionPolicy"));
        assert!(rendered.contains("custom_redactors: 0"));
    }

    #[test]
    fn retained_log_policy_round_trips_through_serde() {
        let policy = RetainedLogPolicy {
            rotation_max_bytes: bytes(1024),
            rotation_max_files: file_count(7),
            retention_max_age: retention_secs(42),
            maintenance_cadence: cadence_secs(60),
            writer_shutdown_timeout: join_secs(5),
            maintenance_max_work_per_pass: Some(3),
        };

        let encoded = serde_json::to_string(&policy).expect("serialize retained-log policy");
        let value: serde_json::Value =
            serde_json::from_str(&encoded).expect("decode retained-log policy json");
        assert!(value["retention_max_age"].is_u64());
        assert!(value["maintenance_cadence"].is_u64());
        assert!(value["writer_shutdown_timeout"].is_u64());
        let decoded: RetainedLogPolicy =
            serde_json::from_str(&encoded).expect("deserialize retained-log policy");

        assert_eq!(decoded, policy);
    }

    #[test]
    fn maintenance_cadence_deserialize_rejects_zero() {
        let error =
            serde_json::from_str::<MaintenanceCadence>("0").expect_err("zero cadence rejected");
        assert!(
            error
                .to_string()
                .contains("MaintenanceCadence must be a non-zero u64 millisecond count")
        );
    }

    #[test]
    fn writer_shutdown_timeout_deserialize_rejects_zero() {
        let error = serde_json::from_str::<WriterShutdownTimeout>("0")
            .expect_err("zero join timeout rejected");
        assert!(
            error
                .to_string()
                .contains("WriterShutdownTimeout must be a non-zero u64 millisecond count")
        );
    }

    #[test]
    fn retention_max_age_deserialize_rejects_zero() {
        let error =
            serde_json::from_str::<RetentionMaxAge>("0").expect_err("zero max age rejected");
        assert!(
            error
                .to_string()
                .contains("RetentionMaxAge must be a non-zero u64 millisecond count")
        );
    }

    #[test]
    #[should_panic(expected = "MaintenanceCadence must be non-zero")]
    fn maintenance_cadence_new_rejects_zero() {
        let _ = MaintenanceCadence::new(Duration::ZERO);
    }

    #[test]
    #[should_panic(expected = "WriterShutdownTimeout must be non-zero")]
    fn writer_shutdown_timeout_new_rejects_zero() {
        let _ = WriterShutdownTimeout::new(Duration::ZERO);
    }

    #[test]
    #[should_panic(expected = "RetentionMaxAge must be non-zero")]
    fn retention_max_age_from_duration_rejects_zero() {
        let _ = RetentionMaxAge::from_duration(Duration::ZERO);
    }

    #[test]
    fn maintenance_cadence_deserialize_reports_type_context() {
        let error = serde_json::from_str::<MaintenanceCadence>("\"bad\"").expect_err("type error");
        assert!(
            error
                .to_string()
                .contains("MaintenanceCadence expects a u64 millisecond count")
        );
    }

    #[test]
    fn writer_shutdown_timeout_deserialize_reports_type_context() {
        let error =
            serde_json::from_str::<WriterShutdownTimeout>("\"bad\"").expect_err("type error");
        assert!(
            error
                .to_string()
                .contains("WriterShutdownTimeout expects a u64 millisecond count")
        );
    }

    #[test]
    fn retention_max_age_deserialize_reports_type_context() {
        let error = serde_json::from_str::<RetentionMaxAge>("\"bad\"").expect_err("type error");
        assert!(
            error
                .to_string()
                .contains("RetentionMaxAge expects a u64 millisecond count")
        );
    }

    #[cfg(feature = "v1")]
    #[test]
    fn invalid_event_returns_event_error() {
        let root = temp_path("invalid");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = Logger::new(config).expect("logger");
        let mut event = log_event(service_name());
        event.version = sc_observability_types::SchemaVersion::new("v0").expect("valid version");
        assert!(logger.emit(event).is_err());
    }

    #[cfg(feature = "v1")]
    #[test]
    fn logger_emit_admits_events_and_preserves_event_errors() {
        let root = temp_path("injected-log-emitter");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = Logger::new(config).expect("logger");

        let mut event = log_event(service_name());
        event.request_id = Some(correlation_id("injected-producer"));
        logger.emit(event).expect("logger admission");
        logger.flush().expect("flush emitted event");

        let snapshot = logger
            .query(&query_all(LogOrder::OldestFirst))
            .expect("query injected event");
        assert_eq!(
            request_ids(&snapshot),
            ["injected-producer"],
            "emitted event must be queryable through the logger"
        );

        let mut invalid = log_event(service_name());
        invalid.version = SchemaVersion::new("v0").expect("valid test schema value");
        let error = logger
            .emit(invalid)
            .expect_err("invalid input retains the released EventError mapping");
        assert_eq!(error.0.diagnostic().code, error_codes::LOGGER_INVALID_EVENT);
    }

    #[cfg(feature = "v1")]
    #[test]
    fn flush_failures_propagate_and_are_counted_in_health() {
        struct FlushFailSink;

        impl LogSink for FlushFailSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                Ok(())
            }

            fn flush(&self) -> Result<(), LogSinkError> {
                Err(legacy_sink_error(Box::new(ErrorContext::new(
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::typed(Arc::new(FlushFailSink)));
        let logger = builder.build();

        let error = logger.flush().expect_err("flush error should propagate");
        assert_eq!(error.diagnostic().code, error_codes::LOGGER_FLUSH_FAILED);

        let typed_error = logger
            .flush()
            .expect_err("typed flush error should propagate");
        assert_eq!(
            typed_error.diagnostic().code,
            error_codes::LOGGER_FLUSH_FAILED
        );

        let health = logger.health();
        assert_eq!(health.dropped_events_total, 0);
        assert_eq!(health.flush_errors_total, 2);
        assert!(health.last_error.is_some());
    }

    #[cfg(feature = "v1")]
    #[test]
    fn flush_times_out_when_writer_is_blocked() {
        let root = temp_path("flush-writer-timeout");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.retained_log_policy.writer_shutdown_timeout = join_ms(10);
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let logger = Logger::new(config).expect("logger");

        logger.log(log_event(service_name())).expect("initial log");
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::is_active
            ),
            "expected maintenance worker to enter the delayed test pass"
        );

        let error = logger
            .flush()
            .expect_err("flush must not wait indefinitely for a blocked writer");
        assert_eq!(error.diagnostic().code, error_codes::LOGGER_WRITER_DEGRADED);
        assert_eq!(logger.health().state, LoggingHealthState::DegradedDropping);

        release_test_pass_delay(&signal);
        let _stopped = logger.shutdown();
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

    #[cfg(feature = "v1")]
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        let injector = RetainedSinkFaultInjector::new();
        builder.register_sink(SinkRegistration::typed(Arc::new(
            injector.fault_sink(Arc::new(RecordingFlushSink::default())),
        )));
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        let mut builder = Logger::builder(config).expect("logger builder");
        let injector = RetainedSinkFaultInjector::new();
        builder.register_sink(SinkRegistration::typed(Arc::new(
            injector.fault_sink(Arc::new(RecordingFlushSink::default())),
        )));
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
    fn explicit_flush_command_flushes_each_registered_sink_once_after_admission() {
        let root = temp_path("explicit-flush-once");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        let sink = Arc::new(RecordingFlushSink::default());
        builder.register_sink(SinkRegistration::typed(sink.clone()));
        let logger = builder.build().expect("logger build");

        logger.log(log_event(service_name())).expect("admit event");
        logger.flush().expect("flush barrier");

        assert_eq!(sink.flush_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn maintenance_health_reflects_last_pass_and_last_error() {
        let root = temp_path("maintenance-health");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(2);
        config.retained_log_policy.retention_max_age = retention_secs(60);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "req-1", 220))
            .expect("emit 1");
        logger
            .emit(log_event_with_request(service_name(), "req-2", 220))
            .expect("emit 2");

        wait_for_pass(
            &signal,
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
        assert!(maintenance.rotated_files_total >= file_count(1));

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

        wait_for_pass(
            &signal,
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(2);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        for request_id in [
            "req-1", "req-2", "req-3", "req-4", "req-5", "req-6", "req-7",
        ] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for_pass(
            &signal,
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(4);
        config.retained_log_policy.retention_max_age = retention_ms(10);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let retention_age = config.retained_log_policy.retention_max_age.as_duration();
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");
        let started = Instant::now();

        for request_id in ["req-1", "req-2", "req-3", "req-4", "req-5"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for_pass(
            &signal,
            || {
                let paths = existing_log_paths(&active_path, 8);
                let health = logger.health();
                health.maintenance.as_ref().is_some_and(|maintenance| {
                    started.elapsed() >= retention_age
                        && maintenance.rotated_files_total >= file_count(1)
                        && maintenance.pruned_files_total >= file_count(1)
                        && paths.len() == 1
                })
            },
            "expected stale retained files to be pruned by age",
        );
    }

    #[test]
    fn shutdown_joins_maintenance_worker_within_timeout() {
        let root = temp_path("shutdown-joins-maintenance");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.retained_log_policy.writer_shutdown_timeout = join_secs(1);
        config.maintenance_test_pass_delay = Some(Duration::from_millis(100));
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        config.maintenance_test_pass_signal = Some(signal.clone());
        let logger = Logger::new(config).expect("logger");

        logger.emit(log_event(service_name())).expect("emit");
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::is_active
            ),
            "expected maintenance worker to enter the delayed test pass"
        );

        let stopped = logger.shutdown();

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
    fn disconnected_writer_returns_canonical_admission_and_flush_failures() {
        struct PanicSink {
            entered: std::sync::mpsc::Sender<()>,
        }

        impl LogSink for PanicSink {
            fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
                let _ = self.entered.send(());
                panic!("injected sink panic terminates writer");
            }

            fn health(&self) -> SinkHealth {
                SinkHealth {
                    name: sink_name("panic-sink"),
                    state: SinkHealthState::Unavailable,
                    last_error: None,
                }
            }
        }

        let root = temp_path("disconnected-writer");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::typed(Arc::new(PanicSink {
            entered: entered_tx,
        })));
        let logger = builder.build().expect("logger");

        logger
            .log(log_event(service_name()))
            .expect("initial admission");
        entered_rx
            .recv_timeout(TEST_WATCHDOG)
            .expect("writer should enter the injected sink");

        // A failed flush is the synchronization point: send failure proves the
        // worker has unwound and dropped its receiver.
        let legacy_flush = logger.flush().expect_err("legacy flush is disconnected");
        assert_eq!(
            legacy_flush.diagnostic().code,
            error_codes::LOGGER_WRITER_DEGRADED
        );
        let admission = logger
            .try_log(log_event(service_name()))
            .expect_err("canonical admission is disconnected");
        assert!(matches!(admission, CanonicalEventError::Routing { .. }));
        assert_eq!(
            admission.diagnostic().code,
            error_codes::LOGGER_WRITER_DEGRADED
        );

        // Exercise canonical admission after a real worker failure.
        let CanonicalEventError::Routing { context } = logger
            .log(log_event(service_name()))
            .expect_err("canonical admission observes the disconnected writer")
        else {
            panic!("disconnected writer must retain its admission failure kind");
        };
        let diagnostic = context.diagnostic();
        assert_eq!(diagnostic.code, error_codes::LOGGER_WRITER_DEGRADED);
        // Separate admissions create their own diagnostic timestamps.
        assert_eq!(
            diagnostic.message,
            "writer thread disconnected while admitting log work"
        );
        assert_eq!(
            diagnostic.remediation,
            Remediation::recoverable(
                "inspect logger writer-thread health",
                [
                    "inspect logger.health().writer_state",
                    "inspect logger.health().last_writer_error",
                ],
            )
        );

        // Consume the runtime after the intentionally panicked worker has
        // been observed. Its completion channel is already disconnected, so
        // shutdown joins the terminated worker without an unbounded wait.
        let _stopped = logger.shutdown();
    }

    #[test]
    fn canonical_logger_admission_preserves_filtering_and_invalid_event_failure() {
        let root = temp_path("typed-admission");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        config.level = LevelFilter::Off;
        let logger = CanonicalLogger::new(config).expect("typed logger");

        assert_eq!(
            logger
                .try_log_with_outcome(log_event(service_name()))
                .expect("filtered event succeeds"),
            AdmissionOutcome::Filtered
        );

        let mut invalid_schema = log_event(service_name());
        invalid_schema.version = SchemaVersion::new("v0").expect("valid test schema value");
        assert!(matches!(
            logger.log(invalid_schema),
            Err(CanonicalEventError::Validation { .. })
        ));

        let wrong_service = ServiceName::new("other-service").expect("valid service");
        assert!(matches!(
            logger.log(log_event(wrong_service)),
            Err(CanonicalEventError::Validation { .. })
        ));

        let root = temp_path("typed-accepted-admission");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        let logger = CanonicalLogger::new(config).expect("typed logger");
        assert_eq!(
            logger
                .try_log_with_outcome(log_event(service_name()))
                .expect("accepted event"),
            AdmissionOutcome::Accepted
        );
    }

    #[cfg(feature = "v1")]
    #[test]
    fn owner_construction_returns_the_injected_writer_start_source() {
        let root = temp_path("writer-start-failure");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        config.writer_start_should_fail = true;

        let Err(error) = Logger::new_with_level_owner(config) else {
            panic!("writer start must fail");
        };
        assert_eq!(error.diagnostic().code, error_codes::LOGGER_INIT_FAILED);
        let context = std::error::Error::source(&error).expect("preserved writer start context");
        assert!(
            context
                .to_string()
                .contains("injected writer start failure")
        );
        assert!(
            context
                .source()
                .is_some_and(<dyn std::error::Error>::is::<std::io::Error>)
        );
        assert_eq!(
            error.0.diagnostic().message,
            "failed to start logger writer thread"
        );
    }

    #[test]
    fn owner_construction_preserves_the_injected_writer_start_source() {
        let root = temp_path("typed-writer-start-failure");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        config.writer_start_should_fail = true;

        let Err(error) = CanonicalLogger::new_with_level_owner(config) else {
            panic!("writer start must fail");
        };
        assert!(matches!(&error, CanonicalInitError::Runtime { .. }));
        assert_eq!(error.diagnostic().code, error_codes::LOGGER_INIT_FAILED);
        assert_eq!(
            std::error::Error::source(&error)
                .expect("preserved writer start source")
                .to_string(),
            "injected writer start failure"
        );
    }

    #[test]
    fn level_owner_changes_only_its_logger_and_filters_with_shared_admission() {
        let root = temp_path("level-owner");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        config.level = LevelFilter::Info;
        let (logger, mut owner) =
            Logger::new_with_level_owner(config).expect("construct logger with owner");

        assert_eq!(logger.level_state().revision, 0);
        let changed = owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::UserRequest)
            .expect("elevate level");
        assert!(matches!(changed, LevelChange::Changed { .. }));
        assert_eq!(logger.level_state().effective_level, LevelFilter::Debug);
        assert_eq!(logger.level_state().revision, 1);

        let mut event = log_event(service_name());
        event.level = Level::Debug;
        assert_eq!(
            logger.try_log_with_outcome(event).expect("debug admitted"),
            AdmissionOutcome::Accepted
        );

        let mut typed_event = log_event(service_name());
        typed_event.level = Level::Debug;
        assert_eq!(
            logger
                .try_log_with_outcome(typed_event)
                .expect("typed debug admitted"),
            AdmissionOutcome::Accepted
        );

        assert!(matches!(
            owner.elevate_level(LevelFilter::Off, LevelChangeSource::Application),
            Err(LevelChangeError::BelowBaseline { .. })
        ));
        assert!(matches!(
            owner.reset_level(LevelChangeSource::Application),
            Ok(LevelChange::Changed { .. })
        ));
        let mut filtered_event = log_event(service_name());
        filtered_event.level = Level::Debug;
        assert_eq!(
            logger
                .try_log_with_outcome(filtered_event)
                .expect("typed filtered admission"),
            AdmissionOutcome::Filtered
        );
        let stopped = logger.shutdown();
        assert!(matches!(
            owner.reset_level(LevelChangeSource::Application),
            Err(LevelChangeError::Stopped)
        ));
        assert_eq!(stopped.level_state().effective_level, LevelFilter::Info);
    }

    #[test]
    fn admission_and_level_mutation_contend_on_one_control_state() {
        use std::sync::{Barrier, mpsc};

        fn assert_contention<F, E>(name: &str, admit: F)
        where
            F: Fn(&CanonicalLogger, LogEvent) -> Result<AdmissionOutcome, E>
                + Send
                + Sync
                + 'static,
            E: std::fmt::Debug + Send + 'static,
        {
            let root = temp_path(name);
            let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
            config.enable_file_sink = false;
            config.enable_console_sink = true;
            let (logger, owner) =
                CanonicalLogger::new_with_level_owner(config).expect("construct logger with owner");
            let logger = Arc::new(logger);
            let barrier = Arc::new(Barrier::new(3));
            let (mutation_tx, mutation_rx) = mpsc::channel();
            let (admission_tx, admission_rx) = mpsc::channel();
            let held_control = logger.level_control.lock().expect("hold control state");

            let mutation_barrier = barrier.clone();
            let mutation = std::thread::spawn(move || {
                let mut owner = owner;
                mutation_barrier.wait();
                mutation_tx
                    .send(owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application))
                    .expect("send mutation result");
            });
            let admission_barrier = barrier.clone();
            let admission_logger = logger.clone();
            let admission = std::thread::spawn(move || {
                let mut event = log_event(service_name());
                event.level = Level::Debug;
                admission_barrier.wait();
                admission_tx
                    .send(admit(&admission_logger, event))
                    .expect("send admission result");
            });

            barrier.wait();
            drop(held_control);
            let mutation_result = mutation_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("mutation result must arrive within the contention bound");
            let admission_result = admission_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("admission result must arrive within the contention bound");
            mutation.join().expect("mutation thread");
            admission.join().expect("admission thread");

            assert!(matches!(mutation_result, Ok(LevelChange::Changed { .. })));
            assert!(matches!(
                admission_result,
                Ok(AdmissionOutcome::Accepted | AdmissionOutcome::Filtered)
            ));
            let snapshot = logger.level_state();
            assert_eq!(snapshot.configured_level, LevelFilter::Info);
            assert_eq!(snapshot.effective_level, LevelFilter::Debug);
            assert_eq!(snapshot.revision, 1);
            let logger = Arc::try_unwrap(logger).unwrap_or_else(|_| panic!("sole logger owner"));
            let _ = logger.shutdown();
        }

        assert_contention("legacy-level-contention", |logger, event| {
            logger.try_log_with_outcome(event)
        });
        assert_contention("typed-level-contention", |logger, event| {
            logger.try_log_with_outcome(event)
        });
    }

    #[test]
    fn typed_admission_and_flush_can_run_concurrently() {
        use std::sync::{Barrier, mpsc};

        let root = temp_path("typed-admission-flush-concurrency");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        let logger = Arc::new(Logger::new(config).expect("typed logger"));
        let barrier = Arc::new(Barrier::new(3));
        let (flush_tx, flush_rx) = mpsc::channel();
        let (admission_tx, admission_rx) = mpsc::channel();

        let flush_logger = logger.clone();
        let flush_barrier = barrier.clone();
        let flush = std::thread::spawn(move || {
            flush_barrier.wait();
            let _ = flush_tx.send(flush_logger.flush());
        });

        let admission_logger = logger.clone();
        let admission_barrier = barrier.clone();
        let admission = std::thread::spawn(move || {
            admission_barrier.wait();
            let _ =
                admission_tx.send(admission_logger.try_log_with_outcome(log_event(service_name())));
        });

        barrier.wait();
        flush_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("flush completion must be bounded")
            .expect("typed flush");
        assert_eq!(
            admission_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("admission completion must be bounded")
                .expect("typed admission"),
            AdmissionOutcome::Accepted
        );
        flush.join().expect("flush thread");
        admission.join().expect("admission thread");

        let Ok(logger) = Arc::try_unwrap(logger) else {
            panic!("all concurrent handles dropped");
        };
        logger.shutdown();
    }

    #[test]
    fn separate_level_owners_do_not_cross_logger_boundaries() {
        let first_root = temp_path("level-isolation-first");
        let second_root = temp_path("level-isolation-second");
        let mut first_config = LoggerConfig::default_for(service_name(), first_root.path_buf());
        first_config.enable_file_sink = false;
        first_config.enable_console_sink = true;
        let mut second_config = LoggerConfig::default_for(service_name(), second_root.path_buf());
        second_config.enable_file_sink = false;
        second_config.enable_console_sink = true;
        let (first, mut first_owner) =
            Logger::new_with_level_owner(first_config).expect("first logger");
        let (second, _second_owner) =
            Logger::new_with_level_owner(second_config).expect("second logger");

        first_owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
            .expect("change only first logger");
        assert_eq!(first.level_state().effective_level, LevelFilter::Debug);
        assert_eq!(second.level_state().effective_level, LevelFilter::Info);
        let mut event = log_event(service_name());
        event.level = Level::Debug;
        assert_eq!(
            second.try_log_with_outcome(event).expect("filtered event"),
            AdmissionOutcome::Filtered
        );
        let _ = first.shutdown();
        let _ = second.shutdown();
    }

    #[test]
    fn level_state_recovers_the_last_committed_snapshot_after_poisoning() {
        let root = temp_path("level-poison");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        let (logger, mut owner) =
            CanonicalLogger::new_with_level_owner(config).expect("construct logger with owner");
        owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
            .expect("change state");
        let control = logger.level_control.clone();
        let _ = std::thread::spawn(move || {
            let _guard = control.lock().expect("lock control");
            panic!("intentionally poison level control");
        })
        .join();

        assert_eq!(logger.level_state().effective_level, LevelFilter::Debug);
        assert_eq!(logger.level_state().revision, 1);
        assert!(matches!(
            owner.reset_level(LevelChangeSource::Application),
            Err(LevelChangeError::Unavailable { .. })
        ));
        let stopped = logger.shutdown();
        assert_eq!(stopped.level_state().revision, 1);
    }

    #[test]
    fn revision_exhaustion_preserves_state_and_uses_the_dedicated_code() {
        let root = temp_path("level-overflow");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
        let (logger, mut owner) =
            CanonicalLogger::new_with_level_owner(config).expect("construct logger with owner");
        logger
            .level_control
            .lock()
            .expect("level state")
            .state
            .revision = u64::MAX;

        assert!(matches!(
            owner.elevate_level(LevelFilter::Info, LevelChangeSource::Application),
            Ok(LevelChange::Unchanged { state }) if state.revision == u64::MAX
        ));

        let error = owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
            .expect_err("overflow must fail");
        assert_eq!(
            error.code().as_str(),
            "SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED"
        );
        assert_eq!(logger.level_state().revision, u64::MAX);
        assert_eq!(logger.level_state().effective_level, LevelFilter::Info);
        let _ = logger.shutdown();
    }

    #[test]
    fn each_changed_level_queues_one_fixed_info_diagnostic() {
        let root = temp_path("level-diagnostics");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        config.level = LevelFilter::Off;
        let sink = Arc::new(RecordingEventSink::default());
        let mut builder = Logger::builder(config).expect("builder");
        builder.register_sink(SinkRegistration::typed(sink.clone()));
        let (logger, mut owner) = builder.build_with_level_owner().expect("owner logger");

        owner
            .elevate_level(LevelFilter::Warn, LevelChangeSource::UserRequest)
            .expect("warn change");
        owner
            .elevate_level(LevelFilter::Error, LevelChangeSource::UserRequest)
            .expect("error change");
        owner
            .reset_level(LevelChangeSource::UserRequest)
            .expect("reset change");
        logger.flush().expect("flush queued diagnostics");

        let events = sink.events.lock().expect("recording events");
        let diagnostics: Vec<_> = events
            .iter()
            .filter(|event| event.action.as_str() == "logging.level_changed")
            .collect();
        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics.iter().all(|event| event.level == Level::Info));
        assert!(
            diagnostics
                .iter()
                .all(|event| event.target.as_str() == "sc_observability")
        );
        drop(events);
        let _ = logger.shutdown();
    }

    #[test]
    fn level_change_diagnostic_uses_configured_context_and_redacts_outside_state_lock() {
        let root = temp_path("level-diagnostic-context");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        config.level = LevelFilter::Off;
        config.process_identity = ProcessIdentityPolicy::Fixed {
            hostname: Some("diagnostic-host".to_string()),
            pid: Some(42),
        };
        let redactor = Arc::new(SourceRedactor {
            control: Mutex::new(None),
            observed_unlocked: AtomicBool::new(false),
        });
        config
            .redaction
            .custom_redactors
            .push(Box::new(redactor.clone()));
        let sink = Arc::new(RecordingEventSink::default());
        let mut builder = CanonicalLogger::builder(config).expect("builder");
        builder.register_sink(SinkRegistration::typed(sink.clone()));
        let (logger, mut owner) = builder.build_with_level_owner().expect("owner logger");
        redactor.attach(&logger.level_control);

        owner
            .elevate_level(LevelFilter::Warn, LevelChangeSource::UserRequest)
            .expect("level change");
        logger.flush().expect("flush diagnostic");

        let events = sink.events.lock().expect("recording events");
        let diagnostic = events
            .iter()
            .find(|event| event.action.as_str() == "logging.level_changed")
            .expect("level diagnostic");
        assert_eq!(diagnostic.service, service_name());
        assert_eq!(
            diagnostic.identity,
            ProcessIdentity {
                hostname: Some("diagnostic-host".to_string()),
                pid: Some(42),
            }
        );
        assert_eq!(
            diagnostic.fields.get("source").and_then(Value::as_str),
            Some("redacted-source")
        );
        assert!(redactor.observed_unlocked.load(Ordering::SeqCst));
        drop(events);
        let _ = logger.shutdown();
    }

    #[cfg(feature = "v1")]
    #[test]
    fn saturated_diagnostic_queue_keeps_the_level_change_committed() {
        let root = temp_path("level-diagnostic-saturation");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.queue_capacity = 1;
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let (logger, mut owner) =
            Logger::new_with_level_owner(config).expect("construct logger with owner");

        logger
            .log(log_event(service_name()))
            .expect("start maintenance");
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::is_active
            ),
            "expected writer maintenance gate before diagnostic saturation"
        );
        owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
            .expect("first diagnostic fills the queue");
        let changed = owner
            .elevate_level(LevelFilter::Trace, LevelChangeSource::Application)
            .expect("level changes despite diagnostic queue saturation");

        assert!(matches!(
            changed,
            LevelChange::Changed {
                current: LevelState { revision: 2, effective_level: LevelFilter::Trace, .. },
                diagnostic: ChangeDiagnostic::NotAccepted { ref diagnostic },
                ..
            } if diagnostic.code == error_codes::LOGGER_QUEUE_FULL
        ));
        release_test_pass_delay(&signal);
        let _ = logger.shutdown();
    }

    #[cfg(feature = "v1")]
    #[test]
    fn level_owner_rejects_changes_during_an_actual_shutdown_stopping_window() {
        let root = temp_path("level-owner-stopping-window");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_console_sink = false;
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let (logger, mut owner) =
            CanonicalLogger::new_with_level_owner(config).expect("construct logger with owner");
        let initial_state = logger.level_state();
        let control = logger.level_control.clone();

        logger
            .log(log_event(service_name()))
            .expect("start maintenance");
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::is_active
            ),
            "expected writer maintenance gate before shutdown"
        );

        let shutdown = std::thread::spawn(move || logger.shutdown());
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::level_stopping
            ),
            "expected logger shutdown to publish the stopping lifecycle"
        );
        assert_eq!(
            control.lock().expect("level control").lifecycle,
            LevelLifecycle::Stopping
        );

        assert!(matches!(
            owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application),
            Err(LevelChangeError::Stopping)
        ));
        release_test_pass_delay(&signal);
        let stopped = shutdown.join().expect("shutdown thread");
        assert_eq!(stopped.level_state(), initial_state);
        assert!(matches!(
            owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application),
            Err(LevelChangeError::Stopped)
        ));
    }

    #[cfg(feature = "v1")]
    #[test]
    fn emit_path_remains_available_during_maintenance_pass() {
        let root = temp_path("maintenance-nonblocking");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let logger = Logger::new(config).expect("logger");

        logger.emit(log_event(service_name())).expect("emit");
        assert!(
            signal.wait_for_state(
                TEST_WATCHDOG,
                crate::maintenance::TestPassDelaySignal::is_active
            ),
            "expected maintenance worker to enter the delayed test pass"
        );

        logger
            .emit(log_event_with_request(service_name(), "during-pass", 10))
            .expect("emit during delayed maintenance pass");
        assert!(
            signal.is_active(),
            "maintenance remains gated while the concurrent emit completes"
        );
        release_test_pass_delay(&signal);
    }

    #[test]
    fn historical_query_reads_active_and_rotated_files() {
        let root = temp_path("query-rotated");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(4);
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 240))
                .expect("emit");
        }

        let active_path = default_log_path(&root, &service_name());
        wait_for_pass(
            &signal,
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
        wait_for_pass(
            &signal,
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

        wait_for_pass(
            &signal,
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(6);
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-1", "req-2", "req-3", "req-4", "req-5"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        let mut oldest_first = None;
        wait_for_pass(
            &signal,
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(4);
        config.retained_log_policy.maintenance_cadence = cadence_ms(5);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        for request_id in ["req-a", "req-b", "req-c"] {
            logger
                .emit(log_event_with_request(service_name(), request_id, 220))
                .expect("emit");
        }

        // `flush` is a writer-thread barrier for writes, not for the separately
        // scheduled maintenance/rotation pass. Retry both readers until one
        // stable view covers the same expected records; do not change public
        // flush semantics merely to make this parity fixture deterministic.
        logger.flush().expect("drain writer before parity query");

        let query = LogQuery {
            order: LogOrder::NewestFirst,
            limit: Some(2),
            ..LogQuery::default()
        };
        let reader = JsonlLogReader::new(default_log_path(&root, &service_name()));
        let mut settled = None;
        wait_for_pass(
            &signal,
            || match (logger.query(&query), reader.query(&query)) {
                (Ok(logger_snapshot), Ok(reader_snapshot))
                    if request_ids(&logger_snapshot) == ["req-c", "req-b"]
                        && request_ids(&reader_snapshot) == ["req-c", "req-b"]
                        && reader_snapshot == logger_snapshot =>
                {
                    settled = Some((logger_snapshot, reader_snapshot));
                    true
                }
                _ => false,
            },
            "expected logger and reader parity after asynchronous maintenance settles",
        );
        let (logger_snapshot, reader_snapshot) = settled.expect("captured parity snapshots");
        assert_eq!(request_ids(&logger_snapshot), ["req-c", "req-b"]);
        assert_eq!(request_ids(&reader_snapshot), ["req-c", "req-b"]);
        assert_eq!(reader_snapshot, logger_snapshot);
    }

    #[test]
    fn follow_starts_at_tail_and_survives_multiple_rotations() {
        let root = temp_path("follow-rotation");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(6);
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

        logger.flush().expect("flush fresh events");
        let followed = drain_follow_until_request_id(&mut follow, "fresh-3");
        assert!(
            followed == vec!["fresh-1", "fresh-2", "fresh-3"]
                || followed == vec!["backlog", "fresh-1", "fresh-2", "fresh-3"]
        );
        assert_eq!(follow.health().state, QueryHealthState::Healthy);
    }

    #[test]
    fn rotation_triggers_when_active_file_exceeds_max_bytes() {
        let root = temp_path("rotation-threshold");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(4);
        config.retained_log_policy.maintenance_cadence = cadence_ms(50);
        let signal = pass_signal(&mut config);
        let logger = Logger::new(config).expect("logger");

        logger
            .emit(log_event_with_request(service_name(), "req-1", 260))
            .expect("emit first");
        logger
            .emit(log_event_with_request(service_name(), "req-2", 260))
            .expect("emit second");

        let active_path = default_log_path(&root, &service_name());
        wait_for_pass(
            &signal,
            || active_path.exists() && rotated_log_path(&active_path, 1).exists(),
            "expected active log rotation to create a .1 retained file",
        );
    }

    #[test]
    fn logger_and_jsonl_reader_follow_have_parity() {
        let root = temp_path("follow-parity");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.rotation_max_bytes = bytes(350);
        config.retained_log_policy.rotation_max_files = file_count(6);
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

        logger.flush().expect("flush fresh events");
        let logger_events = drain_follow_until_request_id(&mut logger_follow, "reader-2");
        let reader_events = drain_follow_until_request_id(&mut reader_follow, "reader-2");

        assert_eq!(logger_events, ["reader-1", "reader-2"]);
        assert_eq!(reader_events, logger_events);
    }

    #[test]
    fn query_health_tracks_decode_and_shutdown_failures() {
        use std::io::Write as _;

        let root = temp_path("query-health");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
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

        let stopped = logger.shutdown();
        assert_eq!(
            stopped.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
    }

    #[test]
    fn logger_health_reports_unavailable_after_shutdown() {
        let root = temp_path("query-shutdown-variant");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = Logger::new(config).expect("logger");

        let stopped = logger.shutdown();

        assert_eq!(
            stopped.health().query.expect("query health").state,
            QueryHealthState::Unavailable
        );
    }

    #[test]
    fn logger_follow_session_becomes_unavailable_after_shutdown() {
        let root = temp_path("follow-shutdown");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = Logger::new(config).expect("logger");

        let mut follow = logger
            .follow(query_all(LogOrder::OldestFirst))
            .expect("follow");
        assert!(follow.poll().expect("initial poll").events.is_empty());

        let _stopped = logger.shutdown();

        assert!(matches!(follow.poll(), Err(QueryError::Shutdown)));
        assert_eq!(follow.health().state, QueryHealthState::Unavailable);
    }

    #[test]
    fn logger_query_and_follow_reject_invalid_queries() {
        let root = temp_path("invalid-query");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = true;
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

    #[cfg(feature = "v1")]
    #[test]
    fn follow_recovers_after_active_file_truncate_and_recreate() {
        let root = temp_path("follow-truncate-recreate");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
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
        logger.flush().expect("flush after truncate");
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

        recreate_with_distinct_identity(&active_path);
        logger
            .emit(log_event_with_request(service_name(), "after-recreate", 20))
            .expect("emit after recreate");
        logger.flush().expect("flush after recreate");
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

#[cfg(test)]
mod canonical_behavior_tests {
    use super::*;
    use crate::sinks::ConsoleWriter;
    use sc_observability_types::v2::EventError as CanonicalEventError;
    use sc_observability_types::{ActionName, ProcessIdentity, TargetCategory};
    use serde_json::{Map, json};
    use std::ops::Deref;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Duration;

    const TEST_WATCHDOG: Duration = Duration::from_secs(30);

    struct TestRoot(tempfile::TempDir);

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

    fn temp_path(name: &str) -> TestRoot {
        TestRoot(
            tempfile::Builder::new()
                .prefix(&format!("sc-observability-{name}-"))
                .tempdir()
                .expect("create temporary test root"),
        )
    }

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn log_event(service: ServiceName) -> LogEvent {
        LogEvent {
            version: sc_observability_types::SchemaVersion::new(
                sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
            )
            .expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service,
            target: TargetCategory::new("logger.core").expect("valid target"),
            action: ActionName::new("emit").expect("valid action"),
            message: Some("Authorization: Bearer abc123".to_string()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields: Map::from_iter([
                ("token".to_string(), json!("Bearer secret")),
                ("secret".to_string(), json!("raw")),
            ]),
        }
    }

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

    struct CanonicalFailSink;

    impl crate::sink::LogSink for CanonicalFailSink {
        fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
            Err(sc_observability_types::v2::LogSinkError::Write {
                context: Box::new(ErrorContext::new(
                    error_codes::LOGGER_SINK_WRITE_FAILED,
                    "fail sink write failed",
                    Remediation::not_recoverable("test sink intentionally fails"),
                )),
            })
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("canonical-fail").expect("valid sink name"),
                state: SinkHealthState::DegradedDropping,
                last_error: None,
            }
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

    #[test]
    fn file_only_logging_writes_jsonl_to_default_path() {
        let root = temp_path("file-only");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = CanonicalLogger::new(config).expect("logger");
        logger.log(log_event(service_name())).expect("log");
        logger.flush().expect("flush");
        let path = default_log_path(&root, &service_name());
        let contents = std::fs::read_to_string(path).expect("read log file");
        assert!(contents.contains("\"level\":\"Info\""));
        assert!(contents.contains("[REDACTED]"));
    }

    #[test]
    fn file_and_console_fan_out_both_receive_event() {
        let root = temp_path("fanout");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_console_sink = false;
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(SinkRegistration::typed(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));
        let logger = builder.build().expect("logger");
        logger.log(log_event(service_name())).expect("log");
        logger.flush().expect("flush");
        assert!(default_log_path(&root, &service_name()).exists());
        let lines = lines.lock().expect("lines poisoned");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("logger.core"));
    }

    #[test]
    fn sink_failures_are_fail_open_and_counted_in_health() {
        let root = temp_path("fail-open");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::typed(Arc::new(CanonicalFailSink)));
        let logger = builder.build().expect("logger");

        logger
            .log(log_event(service_name()))
            .expect("emit remains fail-open");
        logger.flush().expect("wait for sink delivery");

        let health = logger.health();
        assert_eq!(health.state, LoggingHealthState::DegradedDropping);
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
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(
            SinkRegistration::typed(Arc::new(ConsoleSink::from_writer(Box::new(SharedBuffer {
                lines: lines.clone(),
            }))))
            .with_filter(Arc::new(DenyAll)),
        );
        let logger = builder.build().expect("logger");

        logger.log(log_event(service_name())).expect("log");

        assert!(lines.lock().expect("lines poisoned").is_empty());
    }

    #[test]
    fn shutdown_blocks_future_emits() {
        let root = temp_path("shutdown");
        let config = LoggerConfig::default_for(service_name(), root.path_buf());
        let logger = CanonicalLogger::new(config).expect("logger");
        let stopped = logger.shutdown();

        assert_eq!(stopped.health().state, LoggingHealthState::Unavailable);
    }

    #[test]
    fn shutdown_flushes_registered_sinks_before_marking_shutdown() {
        let root = temp_path("shutdown-flush");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let sink = Arc::new(CanonicalCountingSink::default());
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        builder.register_sink(SinkRegistration::typed(sink.clone()));
        let logger = builder.build().expect("logger");

        let stopped = logger.shutdown();

        assert_eq!(sink.flushes.load(Ordering::SeqCst), 1);
        assert_eq!(stopped.health().state, LoggingHealthState::Unavailable);
    }

    #[test]
    fn queue_capacity_rejects_zero_at_construction() {
        assert!(QueueCapacity::new(0).is_none());
        assert_eq!(QueueCapacity::new(1).expect("positive").get(), 1);
    }

    #[test]
    fn redaction_runs_before_sink_fan_out() {
        let root = temp_path("redaction");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.enable_console_sink = false;
        config.redaction.denylist_keys.push("token".to_string());
        config
            .redaction
            .custom_redactors
            .push(Box::new(PrefixRedactor));
        let mut builder = CanonicalLogger::builder(config).expect("logger builder");
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        builder.register_sink(SinkRegistration::typed(Arc::new(ConsoleSink::from_writer(
            Box::new(SharedBuffer {
                lines: lines.clone(),
            }),
        ))));
        let logger = builder.build().expect("logger");
        logger.log(log_event(service_name())).expect("log");
        logger.flush().expect("flush");
        let file_contents =
            std::fs::read_to_string(default_log_path(&root, &service_name())).expect("read file");
        let console_line = lines.lock().expect("lines poisoned")[0].clone();
        assert!(file_contents.contains("[REDACTED]"));
        assert!(file_contents.contains("custom-redacted"));
        assert!(console_line.contains("[REDACTED]"));
    }

    #[test]
    fn shutdown_returns_after_join_timeout_without_waiting_for_blocked_writer() {
        let root = temp_path("shutdown-maintenance-timeout");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.retained_log_policy.maintenance_cadence =
            MaintenanceCadence::new(Duration::from_millis(5));
        config.retained_log_policy.writer_shutdown_timeout =
            WriterShutdownTimeout::new(Duration::from_millis(10));
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let logger = CanonicalLogger::new(config).expect("logger");
        logger.log(log_event(service_name())).expect("log");
        assert!(signal.wait_for_state(
            TEST_WATCHDOG,
            crate::maintenance::TestPassDelaySignal::is_active
        ));
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let shutdown = std::thread::spawn(move || {
            let stopped = logger.shutdown();
            finished_tx
                .send(())
                .expect("test waits for shutdown to return");
            stopped
        });
        assert!(signal.wait_for_state(
            TEST_WATCHDOG,
            crate::maintenance::TestPassDelaySignal::shutdown_timeout_recorded
        ));
        finished_rx.recv_timeout(TEST_WATCHDOG).expect(
            "shutdown must return after configured timeout without joining the blocked writer",
        );
        let stopped = shutdown
            .join()
            .expect("shutdown thread should complete after timeout");
        let maintenance = stopped.health().maintenance.expect("maintenance health");
        assert!(maintenance.last_error.is_some());
        assert_eq!(stopped.health().writer_state, WriterState::Degraded);
        let timeout = stopped
            .health()
            .last_writer_error
            .expect("timeout retained in health");
        assert_eq!(timeout.code, Some(error_codes::LOGGER_SHUTDOWN_TIMED_OUT));
        assert_eq!(timeout.message, "writer thread did not stop within 10ms");
        assert!(
            signal.is_active(),
            "shutdown returned before blocked writer left pass"
        );
        signal.release_delay();
    }

    #[test]
    fn try_log_reports_queue_full_on_saturated_queue() {
        let root = temp_path("try-log-queue-full");
        let mut config = LoggerConfig::default_for(service_name(), root.path_buf());
        config.queue_capacity = 1;
        config.retained_log_policy.maintenance_cadence =
            MaintenanceCadence::new(Duration::from_millis(5));
        config.maintenance_test_pass_delay = Some(Duration::ZERO);
        let signal = Arc::new(crate::maintenance::TestPassDelaySignal::default());
        signal.block_delay_until_released();
        let _release_delay = signal.release_on_drop();
        config.maintenance_test_pass_signal = Some(signal.clone());
        let logger = CanonicalLogger::new(config).expect("logger");
        logger.log(log_event(service_name())).expect("initial log");
        assert!(signal.wait_for_state(
            TEST_WATCHDOG,
            crate::maintenance::TestPassDelaySignal::is_active
        ));
        let mut queued = log_event(service_name());
        queued.request_id =
            Some(sc_observability_types::CorrelationId::new("queued").expect("valid request id"));
        logger
            .try_log(queued)
            .expect("first queued event should fit");
        let mut full = log_event(service_name());
        full.request_id =
            Some(sc_observability_types::CorrelationId::new("full").expect("valid request id"));
        let result = logger.try_log(full);
        assert!(
            matches!(result, Err(CanonicalEventError::Routing { ref context })
            if context.diagnostic().code == error_codes::LOGGER_QUEUE_FULL)
        );
        signal.release_delay();
    }

    #[derive(Default)]
    struct CanonicalCountingSink {
        writes: AtomicU64,
        flushes: AtomicU64,
    }

    impl crate::sink::LogSink for CanonicalCountingSink {
        fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn flush(&self) -> Result<(), sc_observability_types::v2::LogSinkError> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("canonical-counting").expect("sink name"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    struct AcceptAll;
    impl LogFilter for AcceptAll {
        fn accepts(&self, _event: &LogEvent) -> bool {
            true
        }
    }

    fn thin_ptr<T: ?Sized>(value: &Arc<T>) -> *const () {
        Arc::as_ptr(value).cast::<()>()
    }

    #[test]
    fn typed_registration_stores_the_identical_arc_and_filter_without_an_adapter() {
        let sink = Arc::new(CanonicalCountingSink::default());
        let canonical: Arc<dyn crate::sink::LogSink> = sink.clone();
        let filter: Arc<dyn LogFilter> = Arc::new(AcceptAll);
        let registration = SinkRegistration::typed(canonical.clone()).with_filter(filter.clone());
        let stored: &Arc<dyn crate::sink::LogSink> = &registration.sink;
        assert!(Arc::ptr_eq(stored, &canonical));
        assert_eq!(thin_ptr(stored), thin_ptr(&sink));
        assert_eq!(Arc::strong_count(&sink), 3);
        assert!(Arc::ptr_eq(
            registration.filter.as_ref().expect("filter stored"),
            &filter
        ));
        assert!(Arc::ptr_eq(&registration.clone().sink, &canonical));
    }
}
