//! `log` facade bridge for sc-observability structured JSONL logging.
//!
//! [`init`] installs a `log::Log` implementation that maps every `log` record to
//! a `sc_observability_types::LogEvent` and writes it through one process-wide
//! `sc_observability::Logger`. Existing `log::info!` (and friends) call sites keep
//! working unchanged; the tracing-compatible event macros, `#[instrument]` and
//! [`LogControl::try_log`] write through the same logger.
//!
//! # Quick start
//!
//! ```no_run
//! use std::time::Duration;
//! use sc_observability_log::{ActionName, BridgeEvent, BridgeOptions, EventLevel, LoggerConfig, ServiceName, TargetCategory};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LoggerConfig::default_for(
//!     ServiceName::new("my-app")?,
//!     std::env::temp_dir().join("my-app"),
//! );
//! let options = BridgeOptions {
//!     default_action: ActionName::new("log.record")?,
//!     parse_bracket_action: true,
//! };
//! // The single lifecycle owner.
//! let guard = sc_observability_log::init(config, options)?;
//! // Cloneable, non-owning control for everyone else.
//! let control = guard.control();
//!
//! log::info!(target: "my_app::sync", "[sync.start] syncing {} items", 3);
//! control.try_log(BridgeEvent {
//!     level: EventLevel::Info,
//!     target: TargetCategory::new("my_app.ui")?,
//!     action: None,
//!     message: Some("clicked".to_owned()),
//!     outcome: None,
//!     fields: serde_json::Map::new(),
//!     request_id: None,
//!     correlation_id: None,
//!     trace: None,
//! })?;
//! control.flush(Duration::from_secs(1))?;
//! println!("{}", serde_json::to_string(&control.health())?);
//!
//! guard.shutdown(Duration::from_secs(5))?;
//! # Ok(())
//! # }
//! ```
//!
//! # Public contract
//!
//! - **Install once, process-global.** [`init`] succeeds at most once per
//!   process. The `log` facade has no uninstall API, so after shutdown the
//!   bridge can be neither reinstalled ([`InitError::AlreadyInitialized`]) nor
//!   replaced by another `log::Log`.
//! - **One lifecycle owner.** [`LogGuard`] is not `Clone`; only
//!   [`LogGuard::shutdown`] (or, as a fallback, `Drop for LogGuard`) stops the
//!   logger, and it does so once.
//! - **Control is not ownership.** [`LogGuard::control`] returns a cloneable
//!   [`LogControl`] with bounded flush, health, the active path and direct event
//!   admission. No control operation can shut the logger down,
//!   keep it alive, or yield a `LogGuard` or the mutable `Logger`.
//! - **One submission core, one writer.** The facade, the macros and
//!   [`LogControl::try_log`] enter the same guarded core (panic containment and
//!   reentrancy detection, entered once per record) and reach the same
//!   `Logger`. Every rejection is counted under exactly one [`DropCause`]; the
//!   facade and macros keep their unit return and discard the result only after
//!   that accounting.
//! - **Identity is bridge-owned.** Envelope version, timestamp, service name,
//!   process identity (resolved once at `init`), trace context, redaction and
//!   sink routing are filled by the bridge; no producer can supply them.
//! - **Timeout versus final stop.** A shutdown that returns
//!   [`ShutdownError::TimedOut`] leaves [`LifecyclePhase::Stopping`] while a
//!   detached helper finishes; late completion is observable as
//!   [`LifecyclePhase::Stopped`]. `Stopped` is final.
//! - **Serializable contracts.** [`BridgeHealthReport`] (versioned by
//!   [`BRIDGE_HEALTH_SCHEMA_VERSION`]), [`BridgeEvent`] and the native operation
//!   errors are plain serde data with `snake_case` tagged discriminants and
//!   stable code / remediation fields.
//! - **Field keys.** One sanitizer and one reserved prefix
//!   (`sc_observability_log.`) for every producer; see `docs/mapping.md`,
//!   "Field keys", for the per-producer and collision rules.
//!
//! # Guarantees
//!
//! - The submission path never blocks on I/O or queue capacity and never panics;
//!   every dropped event is counted under one [`DropCause`].
//! - `log::logger().flush()` does nothing. Call [`LogControl::flush`] or
//!   [`LogGuard::flush`] for a flush bounded by a timeout.
//! - [`LogGuard::shutdown`] and `Drop for LogGuard` are bounded by a timeout.

pub mod error_codes;

mod bridge;
mod callsite;
mod context;
mod control;
mod error;
mod handle;
mod health;
mod mapping;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use crate::health::BridgeLifecycle;

#[doc(inline)]
pub use control::{BridgeEvent, EmitOutcome, LogControl};
#[doc(inline)]
pub use error::{
    ControlError, DropCause, EmitError, FieldKeyError, FlushError, InitError, LifecyclePhase,
    ShutdownError, WaitError,
};
#[doc(inline)]
pub use error::{ShutdownOutcome, ShutdownReport, UnconfirmedShutdown};
#[doc(inline)]
pub use health::{BRIDGE_HEALTH_SCHEMA_VERSION, BridgeHealthReport};
#[doc(inline)]
pub use sc_observability::LoggerConfig;
// Re-exported so consumers need no direct sc-observability-types dependency.
#[doc(inline)]
pub use sc_observability_types::{
    ActionName, CorrelationId, ErrorCode, LevelFilter, LoggingHealthReport, OutcomeLabel,
    ProcessIdentityPolicy, Remediation, ServiceName, TargetCategory, Timestamp, TraceContext,
};

#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub use handle::{
    block_next_shutdown_save, fail_next_shutdown_coordinator_reservation, notify_next_wait_stopped,
};
#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub use health::fail_next_health_snapshot;
#[doc(inline)]
pub use sc_observability_types::{
    AdmissionOutcome, Level as EventLevel, LevelChange, LevelChangeError, LevelChangeSource,
    LevelState, LogEvent, LogQuery, LogSnapshot, OperationDiagnostic,
};
// `sc_observability_types::Level` is intentionally NOT re-exported: the crate
// root `Level` below is the tracing-style type (associated consts TRACE..ERROR).

/// tracing 0.1 compatible event macros and `#[instrument]`; migrating is an import rename.
#[doc(inline)]
pub use sc_observability_log_macros::{debug, error, event, info, instrument, trace, warn};

/// tracing-compatible level type (mirrors the `tracing::Level` constants).
///
/// It has no `PartialOrd`/`Ord`: tracing orders by verbosity, which would
/// surprise here. Its associated consts make `const LVL: Level = Level::WARN;
/// event!(LVL, ..)` work as in tracing. It serializes with the `LogEvent.level`
/// spelling: `"Trace"`, `"Debug"`, `"Info"`, `"Warn"`, `"Error"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Level(LevelInner);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum LevelInner {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    /// The most verbose level.
    pub const TRACE: Level = Level(LevelInner::Trace);
    /// Diagnostic detail for development.
    pub const DEBUG: Level = Level(LevelInner::Debug);
    /// Normal operation.
    pub const INFO: Level = Level(LevelInner::Info);
    /// Degraded or unexpected behavior.
    pub const WARN: Level = Level(LevelInner::Warn);
    /// Failures.
    pub const ERROR: Level = Level(LevelInner::Error);
}

impl From<Level> for sc_observability_types::Level {
    fn from(level: Level) -> Self {
        match level.0 {
            LevelInner::Trace => sc_observability_types::Level::Trace,
            LevelInner::Debug => sc_observability_types::Level::Debug,
            LevelInner::Info => sc_observability_types::Level::Info,
            LevelInner::Warn => sc_observability_types::Level::Warn,
            LevelInner::Error => sc_observability_types::Level::Error,
        }
    }
}

use handle::{INSTALLED, Installed, SLOT};

/// Bridge behavior. The level threshold is `LoggerConfig.level` only.
#[derive(Debug, Clone)]
pub struct BridgeOptions {
    /// Action used when a record carries no leading `[tag]`.
    pub default_action: ActionName,
    /// Strip a leading `[tag] ` from the message into `LogEvent.action`.
    pub parse_bracket_action: bool,
}

/// Snapshot of dropped-event counters, keyed by [`DropCause`].
///
/// The derives are pinned by `tests/api_freeze.rs`. It serializes as an object
/// with one `u64` per cause, keyed by the `snake_case` cause name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DroppedEvents {
    queue_full: u64,
    invalid_event: u64,
    writer_degraded: u64,
    shutdown_timed_out: u64,
    not_installed: u64,
    logger_panicked: u64,
    reentrant_emit: u64,
}

impl DroppedEvents {
    pub(crate) fn from_counter(read: impl Fn(DropCause) -> u64) -> Self {
        Self {
            queue_full: read(DropCause::QueueFull),
            invalid_event: read(DropCause::InvalidEvent),
            writer_degraded: read(DropCause::WriterDegraded),
            shutdown_timed_out: read(DropCause::ShutdownTimedOut),
            not_installed: read(DropCause::NotInstalled),
            logger_panicked: read(DropCause::LoggerPanicked),
            reentrant_emit: read(DropCause::ReentrantEmit),
        }
    }

    /// Number of events dropped for `cause`.
    #[must_use]
    pub fn get(&self, cause: DropCause) -> u64 {
        match cause {
            DropCause::QueueFull => self.queue_full,
            DropCause::InvalidEvent => self.invalid_event,
            DropCause::WriterDegraded => self.writer_degraded,
            DropCause::ShutdownTimedOut => self.shutdown_timed_out,
            DropCause::NotInstalled => self.not_installed,
            DropCause::LoggerPanicked => self.logger_panicked,
            DropCause::ReentrantEmit => self.reentrant_emit,
        }
    }

    /// Saturating sum over [`DropCause::ALL`].
    #[must_use]
    pub fn total(&self) -> u64 {
        DropCause::ALL
            .iter()
            .fold(0_u64, |sum, cause| sum.saturating_add(self.get(*cause)))
    }
}

/// Timeout used by `Drop for LogGuard` when `shutdown` was not called.
///
/// `Drop for LogGuard` runs the same flush-and-shutdown sequence as
/// [`LogGuard::shutdown`], bounded by this timeout, but it has no `Result` to
/// return to a caller and therefore discards the outcome (`let _ = ..`):
/// an implicit teardown failure (a timeout, a final-flush error or a lost
/// helper thread) is silent. Call [`LogGuard::shutdown`] explicitly whenever
/// that `Result` matters, for example to log or retry on failure.
pub const DEFAULT_DROP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// The sole lifecycle owner of the installed bridge; dropping it shuts the logger down.
///
/// Returned once per process by [`init`]. It is deliberately not `Clone` (a
/// compile-fail test pins this): the code that owns it performs the one final
/// [`shutdown`](Self::shutdown) and records its `Result`. Hand every other
/// component a [`LogControl`] from [`control`](Self::control) instead of sharing
/// the guard, for example through an `Arc`: a shared guard turns the final
/// shutdown into `Drop` on whichever thread releases the last clone.
#[must_use = "dropping the guard shuts the logger down"]
#[derive(Debug)]
pub struct LogGuard {
    active_log_path: Option<PathBuf>,
    level_owner: sc_observability::LevelOwner,
    shut_down: bool,
}

impl LogGuard {
    /// A cloneable, non-owning [`LogControl`] for this bridge.
    ///
    /// The control cannot shut the logger down or keep it alive, so it may be
    /// cloned freely and may outlive the guard.
    #[must_use]
    pub fn control(&self) -> LogControl {
        LogControl::new()
    }

    /// Raises the staged core's effective filter.  Only the lifecycle owner
    /// holds this authority; `LogControl` cannot acquire it.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedLevel` before mutation when the executable cap
    /// cannot retain the requested level, or the staged core's typed lifecycle
    /// error when the owner can no longer change it.
    pub fn elevate_level(
        &mut self,
        level: LevelFilter,
        source: LevelChangeSource,
    ) -> Result<LevelChange, LevelChangeError> {
        ensure_static_level(level).map_err(|available| LevelChangeError::UnsupportedLevel {
            requested: level,
            available,
        })?;
        self.level_owner.elevate_level(level, source)
    }

    /// Restores the staged core's configured baseline through the sole owner.
    ///
    /// # Errors
    ///
    /// Returns the staged core's typed lifecycle error if reset cannot commit.
    pub fn reset_level(
        &mut self,
        source: LevelChangeSource,
    ) -> Result<LevelChange, LevelChangeError> {
        self.level_owner.reset_level(source)
    }

    /// Flushes on a helper thread, bounded by `timeout`; same as [`LogControl::flush`].
    ///
    /// # Errors
    ///
    /// Returns [`FlushError::TimedOut`] when the writer does not acknowledge within
    /// `timeout` (the helper is detached), [`FlushError::Logger`] when a sink flush
    /// fails, and [`FlushError::HelperSpawn`] / [`FlushError::HelperLost`] when the
    /// helper thread cannot start or ends without a result.
    /// [`FlushError::NotRunning`] cannot occur while the guard is alive.
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        handle::flush_installed(timeout)
    }

    /// The final shutdown: threshold → Off, empty the slot, flush, `Logger::shutdown`.
    ///
    /// Bounded by `timeout`. Consumes the guard, so it runs at most once. See
    /// [`LifecyclePhase`] for the lifecycle each result leaves behind.
    ///
    /// # Errors
    ///
    /// Returns [`ShutdownError::TimedOut`] when sole ownership, the final flush and
    /// the writer join do not finish within `timeout` (a detached helper keeps
    /// going; the lifecycle is `Stopping` until it completes and publishes
    /// `Stopped`), [`ShutdownError::FinalFlush`] when the final flush fails (the
    /// logger is still shut down), and [`ShutdownError::HelperSpawn`] /
    /// [`ShutdownError::HelperLost`] for helper thread failures.
    pub fn shutdown(mut self, timeout: Duration) -> Result<(), ShutdownError> {
        self.shut_down = true;
        handle::shutdown_sequence(timeout)
    }

    /// Snapshot of the process-wide dropped-event counters.
    #[must_use]
    pub fn dropped_events(&self) -> DroppedEvents {
        handle::dropped_events()
    }

    /// `Logger::health().active_log_path` captured at init; `None` when
    /// `LoggerConfig.enable_file_sink` is false.
    #[must_use]
    pub fn active_log_path(&self) -> Option<&Path> {
        self.active_log_path.as_deref()
    }

    /// Read-only health snapshot; same as [`LogControl::health`].
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::Unavailable`] when no readable core report is retained.
    pub fn health(&self) -> Result<BridgeHealthReport, ControlError> {
        health::snapshot()
    }
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        if !self.shut_down {
            self.shut_down = true;
            // The Result is intentionally discarded: `Drop` has no channel to report
            // failure to a caller. See `DEFAULT_DROP_SHUTDOWN_TIMEOUT` for the gap
            // this leaves and prefer an explicit `LogGuard::shutdown` call when the
            // outcome matters.
            let _ = handle::shutdown_sequence(DEFAULT_DROP_SHUTDOWN_TIMEOUT);
        }
    }
}

/// Installs the bridge as the process-wide `log` logger; succeeds at most once per process.
///
/// Resolves `config.process_identity` once, builds the `sc_observability::Logger`,
/// installs the bridge with `log::set_boxed_logger`, and derives the `log` facade
/// level and the emit threshold from `config.level`. The returned [`LogGuard`] is
/// the sole lifecycle owner. After it shuts down, the process keeps the stopped
/// bridge installed in the `log` facade: neither this crate nor another `log::Log`
/// can take its place.
///
/// # Errors
///
/// - [`InitError::AlreadyInitialized`] when `init` already succeeded, is running
///   concurrently, or earlier returned `ForeignLoggerInstalled`.
/// - [`InitError::ForeignLoggerInstalled`] when another `log::Log` is installed.
/// - [`InitError::IdentityResolution`] when the configured resolver fails or
///   automatic hostname discovery cannot produce a hostname (retry allowed).
/// - [`InitError::Logger`] when `Logger::new` fails (retry allowed).
pub fn init(config: LoggerConfig, options: BridgeOptions) -> Result<LogGuard, InitError> {
    if let Err(available) = ensure_static_level(config.level) {
        return Err(InitError::UnsupportedLevel {
            configured: config.level,
            available,
        });
    }
    if INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        // Covers a live guard, a guard already shut down (the facade logger cannot be
        // uninstalled), and an own init running concurrently on another thread.
        return Err(InitError::AlreadyInitialized);
    }
    let identity = match mapping::resolve_identity(&config.process_identity) {
        Ok(identity) => identity,
        Err(source) => {
            INSTALLED.store(false, Ordering::SeqCst); // recoverable: allow a retry
            return Err(InitError::IdentityResolution {
                diagnostic: error::diagnostic_from_info(&source),
            });
        }
    };
    let (service, enable_file_sink) = (config.service_name.clone(), config.enable_file_sink);
    let (logger, level_owner) = match sc_observability::Logger::new_with_level_owner(config) {
        Ok(logger) => logger,
        Err(source) => {
            INSTALLED.store(false, Ordering::SeqCst); // recoverable: allow a retry
            return Err(InitError::Logger {
                diagnostic: error::diagnostic_from_info(&source),
            });
        }
    };
    if let Err(source) = handle::reserve_shutdown_coordinator() {
        INSTALLED.store(false, Ordering::SeqCst);
        return Err(InitError::RuntimeStart {
            diagnostic: OperationDiagnostic {
                code: error_codes::SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED,
                message: source.to_string(),
                remediation: Remediation::recoverable(
                    "retry initialization after restoring thread resources",
                    std::iter::empty::<String>(),
                ),
                at: Timestamp::now_utc(),
            },
        });
    }
    let initial_report = logger.health();
    let active_log_path = enable_file_sink.then(|| initial_report.active_log_path.clone());
    let level_state = logger.level_state();
    let installed = Arc::new(Installed {
        logger,
        service,
        identity,
        options,
    });
    if let Err(_source) = log::set_boxed_logger(Box::new(bridge::Bridge)) {
        // INSTALLED stays set: the facade slot belongs to the other logger for the
        // rest of the process, so no retry can succeed. Later calls return
        // AlreadyInitialized without building and tearing down another Logger.
        let _ = handle::shutdown_installed(installed, DEFAULT_DROP_SHUTDOWN_TIMEOUT);
        return Err(InitError::ForeignLoggerInstalled);
    }
    // The facade stays at Trace so compiled debug/trace sites survive. The core
    // LevelOwner is the sole runtime filter for direct, facade, and macro paths.
    health::set_snapshot_config(health::SinkConfig {
        active_log_path: active_log_path.clone(),
        level_state,
    });
    health::store_report(initial_report);
    *SLOT.write().unwrap_or_else(PoisonError::into_inner) = Some(installed);
    handle::set_lifecycle(BridgeLifecycle::Running);
    log::set_max_level(log::LevelFilter::Trace);
    Ok(LogGuard {
        active_log_path,
        level_owner,
        shut_down: false,
    })
}

fn ensure_static_level(level: LevelFilter) -> Result<(), LevelFilter> {
    let available = match log::STATIC_MAX_LEVEL {
        log::LevelFilter::Off => LevelFilter::Off,
        log::LevelFilter::Error => LevelFilter::Error,
        log::LevelFilter::Warn => LevelFilter::Warn,
        log::LevelFilter::Info => LevelFilter::Info,
        log::LevelFilter::Debug => LevelFilter::Debug,
        log::LevelFilter::Trace => LevelFilter::Trace,
    };
    let requested_rank = match level {
        LevelFilter::Trace => 0,
        LevelFilter::Debug => 1,
        LevelFilter::Info => 2,
        LevelFilter::Warn => 3,
        LevelFilter::Error => 4,
        LevelFilter::Off => 5,
    };
    let available_rank = match available {
        LevelFilter::Trace => 0,
        LevelFilter::Debug => 1,
        LevelFilter::Info => 2,
        LevelFilter::Warn => 3,
        LevelFilter::Error => 4,
        LevelFilter::Off => 5,
    };
    if requested_rank < available_rank {
        Err(available)
    } else {
        Ok(())
    }
}

/// Hidden support for `sc-observability-log-macros` expansions.
///
/// Outside semver: `sc-observability-log` pins the macros crate with an exact
/// `=` version so expansions and this module always move in lockstep.
#[doc(hidden)]
pub mod __private {
    use crate::{DropCause, handle};

    pub use serde_json::{Map, Value};

    /// The `LogEvent` level type the expansions pass to `emit_callsite`.
    pub use sc_observability_types::Level;

    /// Call-site label caches and field-value dispatch for the event macros.
    pub use crate::callsite::{
        Callsite, DebugKind, DebugKindTag, DynamicKey, FieldDebug, FieldRecord, FieldValue,
        SerializeKind, SerializeKindTag, debug_value, display_value, emit_callsite,
        record_dynamic_field, record_field,
    };

    /// `#[instrument]` call context: trace ids, the thread-local stack and the completion event.
    pub use crate::context::{CallLevels, CallOutcome, CallSpan, Entered, current_trace};

    /// The single label sanitizer (`mapping.rs`).
    pub use crate::mapping::{
        LabelError, LabelKind, RESERVED_FIELD_PREFIX, action_label, field_key_label,
        sanitize_label, target_label,
    };

    /// Everything a call site controls; `emit` fills version, timestamp, service, identity and trace.
    #[derive(Debug)]
    pub struct EventParts {
        /// Event severity.
        pub level: sc_observability_types::Level,
        /// Sanitized target category.
        pub target: sc_observability_types::TargetCategory,
        /// Action; `None` uses `BridgeOptions.default_action`.
        pub action: Option<sc_observability_types::ActionName>,
        /// Formatted message.
        pub message: Option<String>,
        /// Optional outcome label.
        pub outcome: Option<sc_observability_types::OutcomeLabel>,
        /// Structured fields.
        pub fields: Map<String, Value>,
    }

    /// The staged core owns runtime filtering; this keeps macro call sites
    /// available through the conservative facade Trace ceiling.
    #[must_use]
    pub fn enabled(level: sc_observability_types::Level) -> bool {
        handle::core_enabled(level)
    }

    /// Submits one event to the installed `Logger` with `try_log`.
    ///
    /// `LogEvent.trace` is the innermost `#[instrument]` context entered on the
    /// calling thread (`current_trace()`), or `None` outside any instrumented call.
    ///
    /// Never blocks on I/O or queue capacity and never panics: the slot read, event
    /// assembly and `try_log` run inside the shared guarded submission core. It is
    /// not reentrant: a call on a thread already inside a submission (a panic hook,
    /// sink or redactor that logs) returns at once and is counted as
    /// `DropCause::ReentrantEmit`. Every dropped event is counted under exactly one
    /// [`DropCause`] before the result is discarded. Nothing is flushed; use
    /// `LogControl::flush(timeout)`.
    pub fn emit(parts: EventParts) {
        let _ = handle::submit_guarded(|| handle::submit_installed(parts));
    }

    /// Counts one dropped event; a-2/a-3 call it for `DropCause::InvalidEvent` label failures.
    ///
    /// A wrapper, not `pub use`: re-exporting the `pub(crate)` fn is E0364.
    pub fn record_drop(cause: DropCause) {
        handle::record_drop(cause);
    }
}

#[cfg(test)]
mod tests {
    use super::Level;

    #[test]
    fn level_serializes_with_the_log_event_spelling() {
        for (level, text) in [
            (Level::TRACE, "\"Trace\""),
            (Level::DEBUG, "\"Debug\""),
            (Level::INFO, "\"Info\""),
            (Level::WARN, "\"Warn\""),
            (Level::ERROR, "\"Error\""),
        ] {
            assert_eq!(serde_json::to_string(&level).unwrap(), text);
            assert_eq!(
                serde_json::to_string(&sc_observability_types::Level::from(level)).unwrap(),
                text
            );
            assert_eq!(serde_json::from_str::<Level>(text).unwrap(), level);
        }
    }

    #[test]
    fn level_converts_one_to_one() {
        for (level, expected) in [
            (Level::TRACE, sc_observability_types::Level::Trace),
            (Level::DEBUG, sc_observability_types::Level::Debug),
            (Level::INFO, sc_observability_types::Level::Info),
            (Level::WARN, sc_observability_types::Level::Warn),
            (Level::ERROR, sc_observability_types::Level::Error),
        ] {
            assert_eq!(sc_observability_types::Level::from(level), expected);
        }
    }
}
