//! The released root control converts to the canonical facade without another owner.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "one isolated integration test owns the process-global bridge"
)]

use std::time::Duration;

use sc_observability_log::v2::FlushError;
use sc_observability_log::{ActionName, BridgeOptions, InitError, LoggerConfig, ServiceName};

#[test]
fn released_control_moves_into_canonical_error_surface() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let options = BridgeOptions {
        default_action: ActionName::new("compat.record").expect("action"),
        parse_bracket_action: false,
    };

    let mut zero_capacity = LoggerConfig::default_for(
        ServiceName::new("compat-v2-zero-capacity").expect("service name"),
        directory.path().to_path_buf(),
    );
    zero_capacity.queue_capacity = 0;
    assert!(matches!(
        sc_observability_log::init(zero_capacity, options.clone()),
        Err(InitError::Logger { .. })
    ));

    let mut no_sink = LoggerConfig::default_for(
        ServiceName::new("compat-v2-no-sink").expect("service name"),
        directory.path().to_path_buf(),
    );
    no_sink.enable_console_sink = false;
    no_sink.enable_file_sink = false;
    assert!(matches!(
        sc_observability_log::init(no_sink, options.clone()),
        Err(InitError::Logger { .. })
    ));

    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-v2-conversion").expect("service name"),
        directory.path().to_path_buf(),
    );
    config.enable_console_sink = false;
    let guard = sc_observability_log::init(config, options).expect("released init");
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
