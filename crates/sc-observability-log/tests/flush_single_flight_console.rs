//! One `init` per test binary: a stuck console flush leaves one detached helper, never more.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

mod common;

use common::hold_stdout;
use std::time::{Duration, Instant};

use sc_observability::WriterShutdownTimeout;
use sc_observability_log::{
    ActionName, BridgeOptions, FlushError, LevelFilter, LoggerConfig, ServiceName, error_codes,
};

const STUCK_FLUSH_TIMEOUT: Duration = Duration::from_millis(100);
/// Generous: a retried flush must be rejected long before this could elapse.
const RETRY_TIMEOUT: Duration = Duration::from_secs(30);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const HELPER_FINISH_DEADLINE: Duration = Duration::from_secs(20);

#[test]
fn stuck_console_flush_keeps_one_flight_and_rejects_retries() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("flush-single-flight").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = true;
    config.retained_log_policy.writer_shutdown_timeout =
        WriterShutdownTimeout::new(Duration::from_secs(60));
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let control = guard.control();

    control.flush(IO_TIMEOUT).unwrap();
    let stdout_holder = hold_stdout();
    sc_observability_log::info!(target: "flush_single_flight", "record behind a stuck console sink");

    let first = control.flush(STUCK_FLUSH_TIMEOUT);
    let retries: Vec<_> = (0..3)
        .map(|_| {
            let started = Instant::now();
            let retry = guard.flush(RETRY_TIMEOUT);
            (started.elapsed(), retry)
        })
        .collect();

    stdout_holder.release().unwrap();

    let first_error = first.unwrap_err();
    assert!(
        matches!(first_error, FlushError::TimedOut { .. }),
        "{first_error:?}"
    );
    assert_eq!(
        first_error.code(),
        error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT
    );

    for (elapsed, retry) in retries {
        assert!(matches!(retry, Err(FlushError::InProgress)), "{retry:?}");
        assert!(
            elapsed < RETRY_TIMEOUT / 4,
            "InProgress must not wait for the flush timeout"
        );
        assert_eq!(
            retry.unwrap_err().code(),
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
    }

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
