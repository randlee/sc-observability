//! One `init` per test binary: the `log` facade logger can be installed once per
//! process, so every sub-case for this configuration runs inside the single test fn.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::path::Path;
use std::time::{Duration, Instant};

use sc_observability_log::{
    ActionName, BridgeOptions, InitError, LevelFilter, LogGuard, LoggerConfig, ServiceName,
};
use serde_json::Value;

fn config(root: &Path) -> LoggerConfig {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("bridge-jsonl").unwrap(),
        root.to_path_buf(),
    );
    config.level = LevelFilter::Debug;
    config
}

fn options() -> BridgeOptions {
    BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: true,
    }
}

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn find<'a>(events: &'a [Value], message: &str) -> Option<&'a Value> {
    events.iter().find(|event| event["message"] == message)
}

fn log_records() {
    log::error!(target: "app_lib::commands", "error record");
    log::warn!(target: "app_lib::commands", "warn record");
    log::info!(target: "app_lib::commands", "info record");
    log::debug!(target: "app_lib::commands", "debug record");
    log::trace!(target: "app_lib::commands", "trace record");
    log::info!(target: "app_lib::sync", "[sync.start] tagged record");
    log::info!(target: "kv target", count = 3, ratio = 0.5, ok = true, name = "beads"; "kv record");
}

fn assert_mapped(events: &[Value]) {
    let pid = u64::from(std::process::id());
    for (message, level) in [
        ("error record", "Error"),
        ("warn record", "Warn"),
        ("info record", "Info"),
        ("debug record", "Debug"),
    ] {
        let event = find(events, message).unwrap_or_else(|| panic!("missing {message}"));
        assert_eq!(event["level"], level);
        assert_eq!(event["target"], "app_lib.commands");
        assert_eq!(event["action"], "log.record");
        assert_eq!(event["service"], "bridge-jsonl");
        assert_eq!(event["identity"]["pid"], pid);
        assert_eq!(event["fields"]["code.module"], "bridge_jsonl");
        assert!(
            event["fields"]["code.file"]
                .as_str()
                .unwrap()
                .ends_with("bridge_jsonl.rs")
        );
        assert!(event["fields"]["code.line"].is_u64());
    }
    assert!(
        find(events, "trace record").is_none(),
        "Trace is below LevelFilter::Debug"
    );

    let tagged = find(events, "tagged record").expect("tagged record");
    assert_eq!(tagged["target"], "app_lib.sync");
    assert_eq!(tagged["action"], "sync.start");
    assert_eq!(tagged["identity"]["pid"], pid);

    let kv = find(events, "kv record").expect("kv record");
    assert_eq!(kv["target"], "kv_target");
    assert_eq!(kv["action"], "log.record");
    assert_eq!(kv["fields"]["count"], 3);
    assert_eq!(kv["fields"]["ratio"], 0.5);
    assert_eq!(kv["fields"]["ok"], true);
    assert_eq!(kv["fields"]["name"], "beads");
    assert_eq!(kv["identity"]["pid"], pid);
}

fn assert_facade_flush_is_noop() {
    let started = Instant::now();
    log::logger().flush();
    assert!(started.elapsed() < Duration::from_millis(250));
}

fn assert_second_init_rejected(root: &Path) {
    let second = sc_observability_log::init(config(root), options());
    assert!(matches!(second, Err(InitError::AlreadyInitialized)));
}

fn assert_shutdown_stops_writing(guard: LogGuard, root: &Path) {
    let path = guard.active_log_path().unwrap().to_path_buf();
    guard.shutdown(Duration::from_secs(5)).unwrap();
    log::error!(target: "app_lib::commands", "after shutdown");
    std::thread::sleep(Duration::from_millis(50));
    let events = read_events(&path);
    assert!(find(&events, "after shutdown").is_none());
    assert_second_init_rejected(root);
}

#[test]
fn bridge_writes_mapped_jsonl_and_is_install_once() {
    let root = tempfile::tempdir().unwrap();
    let guard = sc_observability_log::init(config(root.path()), options()).unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();
    assert!(path.starts_with(root.path()));

    log_records();
    assert_facade_flush_is_noop();
    guard.flush(Duration::from_secs(5)).unwrap();
    assert_mapped(&read_events(&path));

    assert_second_init_rejected(root.path());
    assert_shutdown_stops_writing(guard, root.path());
}
