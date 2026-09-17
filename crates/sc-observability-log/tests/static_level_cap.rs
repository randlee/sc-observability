//! Release-build evidence that an executable static facade cap rejects an
//! unavailable startup baseline before it creates a usable bridge.
#![cfg(feature = "static_level_cap_test")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "one isolated integration fixture owns its process-global bridge"
)]

use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, InitError, LevelFilter, LoggerConfig, ServiceName,
};

#[test]
fn capped_release_rejects_trace_before_install_then_allows_info() {
    let root = tempfile::tempdir().unwrap();
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let mut capped = LoggerConfig::default_for(
        ServiceName::new("static-cap").unwrap(),
        root.path().to_path_buf(),
    );
    capped.level = LevelFilter::Trace;
    capped.enable_console_sink = false;
    assert!(matches!(
        sc_observability_log::init(capped, options.clone()),
        Err(InitError::UnsupportedLevel {
            configured: LevelFilter::Trace,
            available: LevelFilter::Info,
        })
    ));

    let mut supported = LoggerConfig::default_for(
        ServiceName::new("static-cap").unwrap(),
        root.path().to_path_buf(),
    );
    supported.level = LevelFilter::Info;
    supported.enable_console_sink = false;
    sc_observability_log::init(supported, options)
        .unwrap()
        .shutdown(Duration::from_secs(5))
        .unwrap();
}
