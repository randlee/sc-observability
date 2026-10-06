//! Release-only consumer proof for the `log` facade's static maximum level.

use std::time::Duration;

use sc_observability_log::{ActionName, BridgeOptions, LevelFilter, LoggerConfig, ServiceName};

fn config(root: &std::path::Path, level: LevelFilter) -> LoggerConfig {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("static-cap").expect("valid fixture service"),
        root.to_path_buf(),
    );
    config.level = level;
    config.enable_console_sink = false;
    config
}

fn main() {
    assert!(
        !cfg!(debug_assertions),
        "the static-level-cap fixture must run in release mode"
    );
    let root = std::env::temp_dir().join(format!(
        "sc-observability-static-level-cap-{}",
        std::process::id()
    ));
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").expect("valid action"),
        parse_bracket_action: false,
    };
    let capped = sc_observability_log::v2::init(config(&root, LevelFilter::Trace), options.clone());
    assert!(capped.is_err(), "trace must be rejected by the Info static cap");
    sc_observability_log::v2::init(config(&root, LevelFilter::Info), options)
        .expect("info remains supported by the static cap")
        .shutdown(Duration::from_secs(5))
        .expect("fixture bridge shuts down");
    let _ = std::fs::remove_dir_all(root);
}
