//! Native bridge health: the core report plus bridge-owned lifecycle evidence.
//!
//! This deliberately does not duplicate the core's logging-health schema. The
//! bridge adds only its own counters, lifecycle and runtime-level snapshot.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

#[cfg(feature = "test_hooks")]
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability_types::{LevelFilter, LoggingHealthReport, Remediation};
use serde::{Deserialize, Serialize};

use crate::{ControlError, DroppedEvents, LifecyclePhase, error_codes, handle};

/// Version of the native bridge-health shape.
pub const BRIDGE_HEALTH_SCHEMA_VERSION: u32 = 1;

/// Point-in-time bridge-owned health evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeHealthReport {
    /// Always [`BRIDGE_HEALTH_SCHEMA_VERSION`] for this initial native shape.
    pub schema_version: u32,
    /// Unmodified core logger health, including sink, queue and writer evidence.
    pub logging: LoggingHealthReport,
    /// Exact-once bridge rejection counters.
    pub dropped: DroppedEvents,
    /// Bridge admission and shutdown lifecycle.
    pub lifecycle: LifecyclePhase,
    /// Init-cached file path, or `None` when the file sink is disabled.
    pub active_log_path: Option<PathBuf>,
    /// Immutable core baseline chosen at initialization.
    pub configured_level: LevelFilter,
    /// Core's current effective level.
    pub effective_level: LevelFilter,
    /// Core's coherent level-state revision.
    pub level_revision: u64,
}

/// Internal lifecycle encoding used by the retained shutdown coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeLifecycle {
    Running,
    ShuttingDown,
    Failed,
    Stopped,
}

/// Init-cached data that remains observable after ownership is consumed.
#[derive(Debug)]
pub(crate) struct SinkConfig {
    pub(crate) active_log_path: Option<PathBuf>,
    pub(crate) level_state: sc_observability_types::LevelState,
}

static SNAPSHOT_CONFIG: OnceLock<SinkConfig> = OnceLock::new();
static LAST_REPORT: Mutex<Option<LoggingHealthReport>> = Mutex::new(None);
static LAST_LEVEL_STATE: Mutex<Option<sc_observability_types::LevelState>> = Mutex::new(None);

#[cfg(feature = "test_hooks")]
static FAIL_NEXT_SNAPSHOT: AtomicBool = AtomicBool::new(false);

/// Makes the next health snapshot fail, for the isolated shutdown fixture.
#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub fn fail_next_health_snapshot() {
    FAIL_NEXT_SNAPSHOT.store(true, Ordering::SeqCst);
}

/// Records initial bridge evidence after successful global installation.
pub(crate) fn set_snapshot_config(config: SinkConfig) {
    store_level_state(config.level_state);
    let _ = SNAPSHOT_CONFIG.set(config);
}

/// Retains the most recent readable core report for stopped or failed lifecycle inspection.
pub(crate) fn store_report(report: LoggingHealthReport) {
    *LAST_REPORT.lock().unwrap_or_else(PoisonError::into_inner) = Some(report);
}

/// Retains the latest coherent core level state once the logger is consumed.
pub(crate) fn store_level_state(level_state: sc_observability_types::LevelState) {
    *LAST_LEVEL_STATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(level_state);
}

/// Init-cached active JSONL path.
pub(crate) fn active_log_path() -> Result<Option<PathBuf>, ControlError> {
    SNAPSHOT_CONFIG
        .get()
        .map(|config| config.active_log_path.clone())
        .ok_or_else(unavailable)
}

/// Reads core health without permitting an upstream mutex panic to unwind the bridge.
pub(crate) fn read_report<State>(
    logger: &sc_observability::Logger<State>,
) -> Option<LoggingHealthReport> {
    catch_unwind(AssertUnwindSafe(|| logger.health())).ok()
}

fn unavailable() -> ControlError {
    ControlError::Unavailable {
        diagnostic: sc_observability_types::OperationDiagnostic {
            code: error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE,
            message: "the bridge could not read retained logger health".to_owned(),
            remediation: Remediation::not_recoverable(
                "inspect lifecycle evidence and restart the process if logging is required",
            ),
            at: sc_observability_types::Timestamp::now_utc(),
        },
    }
}

/// Takes a bridge snapshot, preserving core health rather than re-projecting it.
pub(crate) fn snapshot() -> Result<BridgeHealthReport, ControlError> {
    #[cfg(feature = "test_hooks")]
    if FAIL_NEXT_SNAPSHOT.swap(false, Ordering::SeqCst) {
        return Err(unavailable());
    }
    let installed = handle::current_installed();
    let (report, level_state) = if let Some(installed) = installed {
        let level_state = installed.logger.level_state();
        store_level_state(level_state);
        let report = read_report(&installed.logger);
        if let Some(report) = &report {
            store_report(report.clone());
        }
        (report, level_state)
    } else {
        let config = SNAPSHOT_CONFIG.get().ok_or_else(unavailable)?;
        (
            LAST_REPORT
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            LAST_LEVEL_STATE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .unwrap_or(config.level_state),
        )
    };
    let logging = report.ok_or_else(unavailable)?;
    let config = SNAPSHOT_CONFIG.get().ok_or_else(unavailable)?;
    Ok(BridgeHealthReport {
        schema_version: BRIDGE_HEALTH_SCHEMA_VERSION,
        logging,
        dropped: handle::dropped_events(),
        lifecycle: handle::lifecycle_phase(),
        active_log_path: config.active_log_path.clone(),
        configured_level: level_state.configured_level,
        effective_level: level_state.effective_level,
        level_revision: level_state.revision,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sc_observability_types::{LoggingHealthState, WriterState};

    use super::*;

    #[test]
    fn unavailable_has_the_target_code_and_remediation() {
        let error = unavailable();
        assert_eq!(
            error.code(),
            error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE
        );
        assert!(matches!(
            error.remediation(),
            Remediation::NotRecoverable { .. }
        ));
    }

    #[test]
    fn native_shape_round_trips() {
        fn serde_bounds<T: serde::Serialize + serde::de::DeserializeOwned>() {}
        serde_bounds::<BridgeHealthReport>();
        assert_eq!(BRIDGE_HEALTH_SCHEMA_VERSION, 1);

        let report = BridgeHealthReport {
            schema_version: BRIDGE_HEALTH_SCHEMA_VERSION,
            logging: LoggingHealthReport {
                state: LoggingHealthState::Healthy,
                dropped_events_total: 2,
                flush_errors_total: 3,
                active_log_path: PathBuf::from("/tmp/native-health.jsonl"),
                sink_statuses: Vec::new(),
                queue_depth: 0,
                queue_capacity: 8,
                queue_high_water_mark: 4,
                queue_full_drops_total: 1,
                writer_state: WriterState::Stopped,
                last_writer_error: None,
                query: None,
                maintenance: None,
                last_error: None,
            },
            dropped: DroppedEvents::default(),
            lifecycle: LifecyclePhase::Stopped,
            active_log_path: Some(PathBuf::from("/tmp/native-health.jsonl")),
            configured_level: LevelFilter::Info,
            effective_level: LevelFilter::Warn,
            level_revision: 7,
        };
        let encoded = serde_json::to_value(&report).unwrap();
        let decoded: BridgeHealthReport = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, report);
    }
}
