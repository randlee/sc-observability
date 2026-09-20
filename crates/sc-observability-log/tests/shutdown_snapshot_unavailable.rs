//! A lost final health snapshot is retained as a terminal waiter result.
#![cfg(feature = "test_hooks")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration fixture owns one process-global bridge"
)]

use std::sync::mpsc::sync_channel;
use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, LevelFilter, LoggerConfig, ServiceName, WaitError,
    block_next_shutdown_save, fail_next_health_snapshot, notify_next_wait_stopped,
};

const FIXTURE_DEADLINE: Duration = Duration::from_secs(5);

#[test]
fn health_snapshot_failure_notifies_and_retains_unavailable_for_all_controls() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("shutdown-snapshot-unavailable").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    let guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let first_control = guard.control();
    let second_control = first_control.clone();

    let (save_entered_tx, save_entered_rx) = sync_channel(1);
    let (save_release_tx, save_release_rx) = sync_channel(1);
    block_next_shutdown_save(save_entered_tx, save_release_rx);
    fail_next_health_snapshot();
    let (shutdown_tx, shutdown_rx) = sync_channel(1);
    let _shutdown = std::thread::spawn(move || {
        let _ = shutdown_tx.send(guard.shutdown(FIXTURE_DEADLINE));
    });
    save_entered_rx
        .recv_timeout(FIXTURE_DEADLINE)
        .expect("shutdown did not reach the terminal-save gate");

    let (wait_entered_tx, wait_entered_rx) = sync_channel(1);
    notify_next_wait_stopped(wait_entered_tx);
    let (waiter_tx, waiter_rx) = sync_channel(1);
    let _waiter = std::thread::spawn(move || {
        let _ = waiter_tx.send(first_control.wait_stopped(FIXTURE_DEADLINE));
    });
    wait_entered_rx
        .recv_timeout(FIXTURE_DEADLINE)
        .expect("control did not begin waiting for terminal shutdown");

    save_release_tx.send(()).unwrap();
    let first = waiter_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("already-waiting control was not notified promptly");
    shutdown_rx
        .recv_timeout(FIXTURE_DEADLINE)
        .expect("shutdown did not finish after terminal-save release")
        .unwrap();

    let second = second_control.wait_stopped(Duration::ZERO);
    let first_diagnostic = match first {
        Err(WaitError::Unavailable { diagnostic }) => diagnostic,
        other => panic!("expected retained unavailable result, got {other:?}"),
    };
    let second_diagnostic = match second {
        Err(WaitError::Unavailable { diagnostic }) => diagnostic,
        other => panic!("expected repeated unavailable result, got {other:?}"),
    };
    assert_eq!(
        first_diagnostic.code.as_str(),
        "SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE"
    );
    assert_eq!(
        serde_json::to_value(first_diagnostic).unwrap(),
        serde_json::to_value(second_diagnostic).unwrap(),
        "repeated wait_stopped must return the retained terminal diagnostic"
    );
}
