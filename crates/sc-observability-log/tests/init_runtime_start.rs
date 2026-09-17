//! The required lifecycle worker is reserved before facade installation.
#![cfg(feature = "test_hooks")]

use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, InitError, LevelFilter, LoggerConfig, ServiceName,
    fail_next_shutdown_coordinator_reservation,
};

#[test]
fn coordinator_failure_returns_runtime_start_before_global_install() {
    let root = tempfile::tempdir().unwrap();
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let mut failed = LoggerConfig::default_for(
        ServiceName::new("runtime-start").unwrap(),
        root.path().to_path_buf(),
    );
    failed.level = LevelFilter::Info;
    failed.enable_console_sink = false;
    fail_next_shutdown_coordinator_reservation();
    assert!(matches!(
        sc_observability_log::init(failed, options.clone()),
        Err(InitError::RuntimeStart { .. })
    ));

    let mut retry = LoggerConfig::default_for(
        ServiceName::new("runtime-start").unwrap(),
        root.path().to_path_buf(),
    );
    retry.level = LevelFilter::Info;
    retry.enable_console_sink = false;
    sc_observability_log::init(retry, options)
        .unwrap()
        .shutdown(Duration::from_secs(5))
        .unwrap();
}
