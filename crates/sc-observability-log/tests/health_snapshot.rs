//! Native health keeps the core report intact and retains it after shutdown.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration fixture"
)]

use std::time::Duration;

use sc_observability_log::{
    ActionName, BRIDGE_HEALTH_SCHEMA_VERSION, BridgeOptions, DropCause, LevelFilter,
    LifecyclePhase, LoggerConfig, ServiceName,
};

#[test]
fn health_uses_the_core_report_and_retains_it_after_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("health-snapshot").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    config.queue_capacity = 64;
    let guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();
    let control = guard.control();

    log::info!(target: "health", "a record before the snapshot");
    guard.flush(Duration::from_secs(5)).unwrap();
    let running = guard.health().unwrap();
    assert_eq!(running.schema_version, BRIDGE_HEALTH_SCHEMA_VERSION);
    assert_eq!(running.lifecycle, LifecyclePhase::Running);
    assert_eq!(running.logging.queue_capacity, 64);
    assert_eq!(running.active_log_path.as_deref(), Some(path.as_path()));
    assert_eq!(running.dropped, guard.dropped_events());
    let json = serde_json::to_value(&running).unwrap();
    assert_eq!(json["lifecycle"], "running");
    assert!(json["logging"]["writer_state"].is_string());
    assert!(json["dropped"]["not_installed"].is_u64());

    guard.shutdown(Duration::from_secs(5)).unwrap();
    let stopped = control.health().unwrap();
    assert_eq!(stopped.lifecycle, LifecyclePhase::Stopped);
    assert_eq!(
        serde_json::to_value(stopped.logging.writer_state).unwrap(),
        "Stopped"
    );
    assert_eq!(stopped.logging.queue_depth, 0);
    assert_eq!(stopped.active_log_path.as_deref(), Some(path.as_path()));
    assert!(control.flush(Duration::from_secs(1)).is_err());

    let dropped = stopped.dropped;
    log::error!(target: "health", "a record after shutdown");
    assert_eq!(control.health().unwrap().dropped, dropped);
    assert_eq!(dropped.get(DropCause::LoggerPanicked), 0);
}
