//! Release-build evidence that an executable static facade cap rejects an
//! unavailable startup baseline before it creates a usable bridge.
// `log::STATIC_MAX_LEVEL` is profile-dependent: this fixture intentionally
// proves the `Info` cap supplied by `release_max_level_info`, so it must only
// compile in a release profile.  Running it in debug would correctly expose
// a `Trace` cap and make the release assertion meaningless.
#![cfg(all(feature = "static_level_cap_test", not(debug_assertions)))]
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
    let error = sc_observability_log::init(capped, options.clone()).unwrap_err();
    assert!(matches!(error, InitError::Configuration { .. }));
    assert_eq!(
        error.diagnostic().code.as_str(),
        "SC_OBSERVABILITY_LOG_UNSUPPORTED_LEVEL"
    );
    assert!(error.diagnostic().message.contains("Trace"));
    assert!(error.diagnostic().message.contains("Info"));

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
