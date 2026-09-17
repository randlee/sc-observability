//! One `init` per test binary: a stuck flush leaves one detached helper, never more.
//!
//! QA-2 RSH-004. The active JSONL path is replaced by a FIFO with no reader, so
//! the writer thread blocks opening it and sc-observability's flush never
//! returns: a stuck sink. The first flush times out and detaches its helper;
//! a retried flush returns `FlushError::InProgress` at once without starting a
//! thread (`helpers.detached` stays 1). Opening the FIFO releases the writer:
//! the helper finishes, the counter returns to 0 and the next flush succeeds.
//!
//! Unix only: the stuck sink is a named pipe (`mkfifo`).
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::fs::OpenOptions;
use std::time::{Duration, Instant};

use sc_observability_log::{
    ActionName, BridgeOptions, FlushError, LevelFilter, LoggerConfig, ServiceName, error_codes,
};

const STUCK_FLUSH_TIMEOUT: Duration = Duration::from_millis(100);
/// Generous: a retried flush must be rejected long before this could elapse.
const RETRY_TIMEOUT: Duration = Duration::from_secs(30);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const HELPER_FINISH_DEADLINE: Duration = Duration::from_secs(20);

#[test]
fn stuck_flush_keeps_one_detached_helper_and_rejects_retries() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("flush-single-flight").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let control = guard.control();
    let path = control.active_log_path().unwrap().unwrap();

    // Nothing queued or in flight; then swap the active file for a reader-less FIFO.
    control.flush(IO_TIMEOUT).unwrap();
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success(), "mkfifo failed: {status}");

    // The writer blocks opening the FIFO for this record, so flush cannot return.
    sc_observability_log::info!(target: "flush_single_flight", "record behind a stuck sink");

    // (a) The first flush times out and detaches exactly one helper.
    let first = control.flush(STUCK_FLUSH_TIMEOUT);
    assert!(
        matches!(first, Err(FlushError::TimedOut { timeout }) if timeout == STUCK_FLUSH_TIMEOUT),
        "{first:?}"
    );

    // A retry is rejected at once and spawns nothing.
    for _ in 0..3 {
        let started = Instant::now();
        let retry = guard.flush(RETRY_TIMEOUT);
        assert!(matches!(retry, Err(FlushError::InProgress)), "{retry:?}");
        assert!(
            started.elapsed() < RETRY_TIMEOUT / 4,
            "InProgress must not wait for the flush timeout"
        );
        assert_eq!(
            retry.unwrap_err().code(),
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
    }

    // (b) Open the FIFO read-write (never blocks, and keeps a writer so reads
    // never hit EOF): the writer's open completes and the detached helper finishes.
    let _reader = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let deadline = Instant::now() + HELPER_FINISH_DEADLINE;
    loop {
        if control.flush(IO_TIMEOUT).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the detached helper never finished"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    guard.shutdown(IO_TIMEOUT).unwrap();
}
