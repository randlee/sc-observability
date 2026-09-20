//! One `init` per test binary: the bridge's emit guard covers user formatting.
//!
//! R-A4-003: `Bridge::log` renders `record.args()` and key-values inside the one
//! emit guard, so a panicking or logging `Display` implementation is contained
//! and counted exactly once. Every sub-case runs inside the single test fn.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::fmt;
use std::path::Path;
use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, DropCause, LevelFilter, LogGuard, LoggerConfig, ServiceName,
};
use serde_json::Value;

/// A `Display` implementation that always panics.
struct PanickingDisplay;

impl fmt::Display for PanickingDisplay {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("PanickingDisplay::fmt (expected by this test)");
    }
}

/// A `Display` implementation that logs while it is being formatted.
struct LoggingDisplay;

impl fmt::Display for LoggingDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        log::info!(target: "bridge_guard", "nested record from Display");
        f.write_str("formatted")
    }
}

fn read_messages(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter_map(|event| event["message"].as_str().map(ToOwned::to_owned))
        .collect()
}

fn count(guard: &LogGuard, cause: DropCause) -> u64 {
    guard.dropped_events().get(cause)
}

/// Runs `body` under the caller's own `catch_unwind` and asserts nothing unwinds.
fn assert_no_unwind(label: &str, body: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    assert!(result.is_ok(), "{label}: a panic unwound out of log!");
}

fn panicking_message_is_contained(guard: &LogGuard) {
    let panicked = count(guard, DropCause::LoggerPanicked);
    let reentrant = count(guard, DropCause::ReentrantEmit);
    assert_no_unwind("panicking message", || {
        log::info!(target: "bridge_guard", "message {PanickingDisplay}");
    });
    assert_eq!(count(guard, DropCause::LoggerPanicked), panicked + 1);
    assert_eq!(count(guard, DropCause::ReentrantEmit), reentrant);
}

fn panicking_kv_value_is_contained(guard: &LogGuard) {
    let panicked = count(guard, DropCause::LoggerPanicked);
    let reentrant = count(guard, DropCause::ReentrantEmit);
    assert_no_unwind("panicking kv value", || {
        log::info!(target: "bridge_guard", value:% = PanickingDisplay; "kv record with panicking value");
    });
    assert_eq!(count(guard, DropCause::LoggerPanicked), panicked + 1);
    assert_eq!(count(guard, DropCause::ReentrantEmit), reentrant);
}

fn logging_formatter_is_reentrant(guard: &LogGuard) {
    let panicked = count(guard, DropCause::LoggerPanicked);
    let reentrant = count(guard, DropCause::ReentrantEmit);
    assert_no_unwind("logging formatter", || {
        log::info!(target: "bridge_guard", "outer record {LoggingDisplay}");
    });
    assert_eq!(count(guard, DropCause::ReentrantEmit), reentrant + 1);
    assert_eq!(count(guard, DropCause::LoggerPanicked), panicked);
}

#[test]
fn bridge_guard_contains_user_formatting() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("bridge-guard").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info; // every record below is enabled
    config.enable_console_sink = false;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();

    // Silence the default hook's stderr report for the two expected panics. The
    // hook itself runs inside the emit guard and does not log.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    panicking_message_is_contained(&guard);
    panicking_kv_value_is_contained(&guard);
    std::panic::set_hook(previous_hook);

    logging_formatter_is_reentrant(&guard);
    log::info!(target: "bridge_guard", "sentinel after the guarded cases");

    guard.flush(Duration::from_secs(5)).unwrap();
    let messages = read_messages(&path);
    let occurrences = |needle: &str| messages.iter().filter(|m| m.as_str() == needle).count();
    assert_eq!(
        occurrences("outer record formatted"),
        1,
        "outer record written once"
    );
    assert_eq!(
        occurrences("nested record from Display"),
        0,
        "nested record dropped"
    );
    assert!(
        messages.iter().all(|m| !m.contains("panicking value")),
        "a record whose kv value panicked is dropped"
    );
    assert_eq!(occurrences("sentinel after the guarded cases"), 1);

    guard.shutdown(Duration::from_secs(5)).unwrap();
}
