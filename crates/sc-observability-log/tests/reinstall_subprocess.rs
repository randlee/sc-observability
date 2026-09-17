//! Install-once and post-stop direct-admission evidence in a fresh child process.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "subprocess fixture controls its own process-global logger"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use sc_observability_log::{
    ActionName, BridgeEvent, BridgeOptions, EventLevel, LevelFilter, LoggerConfig, ServiceName,
    TargetCategory,
};
use serde_json::{Value, json};

const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_REINSTALL_CHILD_ROOT";
const RESULT_PREFIX: &str = "REINSTALL_CHILD_RESULT ";
const TEST_NAME: &str = "bridge_cannot_be_reinstalled_or_replaced_after_shutdown";
const CHILD_DEADLINE: Duration = Duration::from_secs(60);

struct ForeignLogger;

impl log::Log for ForeignLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}

fn config(root: &Path) -> (LoggerConfig, BridgeOptions) {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("reinstall-child").unwrap(),
        root.to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    (config, options)
}

fn event(message: &str) -> BridgeEvent {
    BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("reinstall").unwrap(),
        action: None,
        message: Some(message.to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    }
}

fn run_child(root: &Path) {
    let (first_config, options) = config(root);
    let guard = sc_observability_log::init(first_config, options).unwrap();
    let control = guard.control();
    assert!(control.try_log(event("direct before shutdown")).is_ok());
    guard.shutdown(Duration::from_secs(5)).unwrap();
    let (second_config, second_options) = config(root);
    let reinit = sc_observability_log::init(second_config, second_options).unwrap_err();
    let post_stop = control.try_log(event("direct after shutdown")).unwrap_err();
    let flush = control.flush(Duration::from_millis(10)).unwrap_err();
    println!(
        "{RESULT_PREFIX}{}",
        json!({
            "reinit_code": reinit.code(),
            "foreign_logger_rejected": log::set_boxed_logger(Box::new(ForeignLogger)).is_err(),
            "post_stop": post_stop,
            "flush": flush,
            "health": control.health().unwrap(),
            "active_log_path": control.active_log_path().unwrap(),
        })
    );
}

fn run_parent() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, root.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (stdout_tx, stdout_rx) = sync_channel(1);
    let drain = std::thread::spawn(move || {
        use std::io::Read;
        let mut stdout = stdout;
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = stdout_tx.send(result);
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() >= CHILD_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_rx
        .recv_timeout(CHILD_DEADLINE)
        .expect("stdout drain thread timed out")
        .expect("could not drain child stdout");
    drain.join().expect("stdout drain thread panicked");
    assert!(status.success());
    let stdout = String::from_utf8(stdout).unwrap();
    let line = stdout
        .lines()
        .find_map(|line| line.split_once(RESULT_PREFIX).map(|(_, value)| value))
        .unwrap();
    let result: Value = serde_json::from_str(line).unwrap();
    assert_eq!(
        result["reinit_code"],
        "SC_OBSERVABILITY_LOG_ALREADY_INITIALIZED"
    );
    assert_eq!(result["foreign_logger_rejected"], true);
    assert_eq!(result["post_stop"]["kind"], "not_running");
    assert_eq!(result["flush"]["kind"], "not_running");
    assert_eq!(result["health"]["lifecycle"], "stopped");
    let path = PathBuf::from(result["active_log_path"].as_str().unwrap());
    let contents = std::fs::read_to_string(path).unwrap();
    assert!(contents.contains("direct before shutdown"));
    assert!(!contents.contains("direct after shutdown"));
}

#[test]
fn bridge_cannot_be_reinstalled_or_replaced_after_shutdown() {
    match std::env::var_os(CHILD_ENV) {
        Some(root) => run_child(Path::new(&root)),
        None => run_parent(),
    }
}
