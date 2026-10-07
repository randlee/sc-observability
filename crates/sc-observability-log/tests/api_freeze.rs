//! Compile-time lock for the accepted B.P3 native bridge surface.
#![cfg(feature = "v1")]
#![allow(
    deprecated,
    clippy::items_after_statements,
    reason = "signature probes are adjacent to the contract they pin"
)]

use std::path::PathBuf;
use std::time::Duration;

use sc_observability_log::{
    AdmissionOutcome, BridgeEvent, BridgeHealthReport, BridgeOptions, ControlError, DropCause,
    DroppedEvents, EmitError, ErrorCode, EventLevel, FieldKeyError, FlushError, HelperHealth,
    InitError, LifecyclePhase, LogControl, LogGuard, LogQuery, LogSnapshot, LoggerConfig,
    LoggingHealthReport, ShutdownError, WaitError,
};

#[allow(
    clippy::needless_pass_by_value,
    reason = "the owned signature pins exhaustive released variants"
)]
fn released_init_error(error: InitError) {
    match error {
        InitError::AlreadyInitialized
        | InitError::ForeignLoggerInstalled
        | InitError::UnsupportedLevel { .. }
        | InitError::IdentityResolution { .. }
        | InitError::Logger { .. }
        | InitError::RuntimeStart { .. } => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the owned signature pins exhaustive released variants"
)]
fn released_flush_error(error: FlushError) {
    match error {
        FlushError::TimedOut { .. }
        | FlushError::Logger { .. }
        | FlushError::HelperSpawn { .. }
        | FlushError::HelperLost { .. }
        | FlushError::InProgress
        | FlushError::NotRunning { .. } => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the owned signature pins exhaustive released variants"
)]
fn released_shutdown_error(error: ShutdownError) {
    match error {
        ShutdownError::TimedOut { .. }
        | ShutdownError::FinalFlush { .. }
        | ShutdownError::HelperSpawn { .. }
        | ShutdownError::HelperLost { .. } => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the owned signature pins exhaustive released variants"
)]
fn released_emit_error(error: EmitError) {
    match error {
        EmitError::InvalidField { .. }
        | EmitError::InvalidEvent { .. }
        | EmitError::QueueFull { .. }
        | EmitError::WriterDegraded { .. }
        | EmitError::ShutdownTimedOut { .. }
        | EmitError::NotRunning { .. }
        | EmitError::Reentrant
        | EmitError::Panicked => {}
    }
}

#[test]
fn bp3_public_api_is_frozen() {
    let _: fn(InitError) = released_init_error;
    let _: fn(FlushError) = released_flush_error;
    let _: fn(ShutdownError) = released_shutdown_error;
    let _: fn(EmitError) = released_emit_error;
    let _: fn(LoggerConfig, BridgeOptions) -> Result<LogGuard, InitError> =
        sc_observability_log::init;
    let _: sc_observability_log::v2::BridgeHealthReport;
    let _: sc_observability_log::v2::HelperHealth;
    let _: sc_observability_log::v2::LoggerConfig;
    let _: sc_observability_log::v2::LevelFilter;
    let _: sc_observability_log::v2::ServiceName;
    let _: fn(&LogGuard) -> LogControl = LogGuard::control;
    let _: fn(&LogGuard) -> Result<BridgeHealthReport, ControlError> = LogGuard::health;
    let _: fn(&LogGuard, Duration) -> Result<(), FlushError> = LogGuard::flush;
    let _: fn(LogGuard, Duration) -> Result<(), ShutdownError> = LogGuard::shutdown;
    let _: fn(&LogGuard) -> DroppedEvents = LogGuard::dropped_events;
    let _: for<'a> fn(&'a LogGuard) -> Option<&'a std::path::Path> = LogGuard::active_log_path;

    let _: fn(&LogControl, BridgeEvent) -> Result<AdmissionOutcome, EmitError> =
        LogControl::try_log;
    let _: fn(&LogControl, &LogQuery) -> Result<LogSnapshot, ControlError> = LogControl::query;
    let _: fn(&LogControl, Duration) -> Result<(), FlushError> = LogControl::flush;
    let _: fn(&LogControl) -> Result<BridgeHealthReport, ControlError> = LogControl::health;
    let _: fn(&LogControl) -> Result<Option<PathBuf>, ControlError> = LogControl::active_log_path;
    let _: fn(&LogControl) -> DroppedEvents = LogControl::dropped_events;
    let _: fn(&LogControl, Duration) -> Result<sc_observability_log::ShutdownReport, WaitError> =
        LogControl::wait_stopped;

    fn data<T: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned>() {}
    fn data_eq<
        T: std::fmt::Debug + Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned,
    >() {
    }
    fn error<T: std::error::Error + Send + Sync + 'static>() {}
    fn control<T: std::fmt::Debug + Clone + Send + Sync + 'static>() {}
    fn guard<T: std::fmt::Debug + Send + Sync + 'static>() {}
    data_eq::<BridgeEvent>();
    data_eq::<BridgeHealthReport>();
    data_eq::<HelperHealth>();
    data::<FieldKeyError>();
    data::<LifecyclePhase>();
    for_error_bounds();
    control::<LogControl>();
    guard::<LogGuard>();

    fn for_error_bounds() {
        error::<InitError>();
        error::<FlushError>();
        error::<ShutdownError>();
        error::<EmitError>();
        error::<ControlError>();
        error::<WaitError>();
    }

    let _ = |event: BridgeEvent| {
        let _: EventLevel = event.level;
        let _: serde_json::Map<String, serde_json::Value> = event.fields;
    };
    let _ = |health: BridgeHealthReport| {
        let _: (
            u32,
            LoggingHealthReport,
            DroppedEvents,
            LifecyclePhase,
            Option<PathBuf>,
        ) = (
            health.schema_version,
            health.logging,
            health.dropped,
            health.lifecycle,
            health.active_log_path,
        );
        let _: (
            sc_observability_log::LevelFilter,
            sc_observability_log::LevelFilter,
            u64,
        ) = (
            health.configured_level,
            health.effective_level,
            health.level_revision,
        );
        let _: HelperHealth = health.helpers;
    };
    let _ = |helpers: HelperHealth| -> (bool, u64) { (helpers.flush_in_flight, helpers.detached) };
    let _: fn(&DroppedEvents, DropCause) -> u64 = DroppedEvents::get;
    let _: fn(&DroppedEvents) -> u64 = DroppedEvents::total;
    let _: [DropCause; 7] = DropCause::ALL;
    let _: &[ErrorCode] = sc_observability_log::error_codes::ALL;
}
