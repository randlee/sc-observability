//! The released root control converts to the canonical facade without another owner.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "one isolated integration test owns the process-global bridge"
)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use sc_observability_log::v2::FlushError;
use sc_observability_log::{
    ActionName, BridgeOptions, InitError, LoggerConfig, Remediation, ServiceName,
};

const INIT_CASE_ENV: &str = "SC_OBSERVABILITY_LOG_COMPAT_V2_INIT_CASE";

fn options() -> BridgeOptions {
    BridgeOptions {
        default_action: ActionName::new("compat.record").expect("action"),
        parse_bracket_action: false,
    }
}

fn run_isolated_init_case(case: &str, test_name: &str) {
    let status = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
        .env(INIT_CASE_ENV, case)
        .status()
        .expect("spawn isolated public-init regression");
    assert!(
        status.success(),
        "isolated public-init regression failed: {status}"
    );
}

fn assert_logger_diagnostic(
    result: Result<sc_observability_log::LogGuard, InitError>,
    message: &str,
    remediation: &Remediation,
) {
    let InitError::Logger { diagnostic } =
        result.expect_err("public init must reject configuration")
    else {
        panic!("public init must preserve the released Logger variant");
    };
    assert_eq!(
        diagnostic.code.as_str(),
        "SC_OBSERVABILITY_LOGGER_INIT_FAILED"
    );
    assert_eq!(diagnostic.message, message);
    assert_eq!(&diagnostic.remediation, remediation);
}

fn zero_queue_config(root: &Path) -> LoggerConfig {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-v2-zero-capacity").expect("service name"),
        root.to_path_buf(),
    );
    config.queue_capacity = 0;
    config
}

fn no_sink_config(root: &Path) -> LoggerConfig {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-v2-no-sink").expect("service name"),
        root.to_path_buf(),
    );
    config.enable_console_sink = false;
    config.enable_file_sink = false;
    config
}

#[test]
fn released_zero_queue_init_preserves_logger_diagnostic() {
    if std::env::var(INIT_CASE_ENV).as_deref() != Ok("zero-queue") {
        return run_isolated_init_case(
            "zero-queue",
            "released_zero_queue_init_preserves_logger_diagnostic",
        );
    }

    let directory = tempfile::tempdir().expect("temporary log directory");
    assert_logger_diagnostic(
        sc_observability_log::init(zero_queue_config(directory.path()), options()),
        "logger queue capacity must be greater than zero",
        &Remediation::recoverable(
            "set LoggerConfig.queue_capacity to a positive value before constructing the logger",
            ["increase queue_capacity to at least 1"],
        ),
    );
}

#[test]
fn released_no_sink_init_preserves_logger_diagnostic() {
    if std::env::var(INIT_CASE_ENV).as_deref() != Ok("no-sink") {
        return run_isolated_init_case(
            "no-sink",
            "released_no_sink_init_preserves_logger_diagnostic",
        );
    }

    let directory = tempfile::tempdir().expect("temporary log directory");
    assert_logger_diagnostic(
        sc_observability_log::init(no_sink_config(directory.path()), options()),
        "logger must have at least one registered sink",
        &Remediation::recoverable(
            "enable a built-in sink or register a sink before building the logger",
            [
                "set LoggerConfig.enable_file_sink or enable_console_sink to true",
                "register a sink with v2::LoggerBuilder::register_sink",
            ],
        ),
    );
}

#[test]
fn released_control_moves_into_canonical_error_surface() {
    let directory = tempfile::tempdir().expect("temporary log directory");

    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-v2-conversion").expect("service name"),
        directory.path().to_path_buf(),
    );
    config.enable_console_sink = false;
    let guard = sc_observability_log::init(config, options()).expect("released init");
    let canonical_control = guard.control().into_v2();
    guard
        .shutdown(Duration::from_secs(2))
        .expect("released shutdown");

    let error = canonical_control
        .flush(Duration::from_secs(1))
        .expect_err("stopped control must report the canonical drain error");
    assert!(matches!(error, FlushError::Drain { .. }));
    assert_eq!(
        error.diagnostic().code.as_str(),
        "SC_OBSERVABILITY_LOG_NOT_RUNNING"
    );
}
