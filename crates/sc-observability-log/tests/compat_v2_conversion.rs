//! The released root control converts to the canonical facade without another owner.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "one isolated integration test owns the process-global bridge"
)]

use std::time::Duration;

use sc_observability_log::v2::FlushError;
use sc_observability_log::{ActionName, BridgeOptions, LoggerConfig, ServiceName};

#[test]
fn released_control_moves_into_canonical_error_surface() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-v2-conversion").expect("service name"),
        directory.path().to_path_buf(),
    );
    config.enable_console_sink = false;
    let guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("compat.record").expect("action"),
            parse_bracket_action: false,
        },
    )
    .expect("released init");
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
