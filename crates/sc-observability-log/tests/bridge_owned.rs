//! Owned initialization excludes a competing host attachment.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration fixture keeps setup failures explicit"
)]

use std::sync::Arc;
use std::time::Duration;

use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEventPolicy, BridgeOptions, InitError, LoggerConfig,
    ServiceName, attach_logger, init,
};
use sc_observability_types::LogEvent;

struct Admit;

impl BridgeEventPolicy for Admit {
    fn decide(&self, _: &LogEvent) -> sc_observability_log::BridgeEventDecision {
        sc_observability_log::BridgeEventDecision::Admit
    }
}

#[test]
fn owned_init_excludes_host_attachment() {
    let root = tempfile::tempdir().expect("temp root");
    let config = LoggerConfig::default_for(
        ServiceName::new("owned-facade").expect("service"),
        root.path().to_path_buf(),
    );
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").expect("action"),
        parse_bracket_action: false,
    };
    let guard = init(config, options).expect("owned init");

    let host_root = tempfile::tempdir().expect("host temp root");
    let host = Arc::new(
        sc_observability::Logger::new_typed(LoggerConfig::default_for(
            ServiceName::new("attached-conflict").expect("service"),
            host_root.path().to_path_buf(),
        ))
        .expect("host logger"),
    );
    let attachment_options = AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("log.record").expect("action"),
            parse_bracket_action: false,
        },
        Arc::new(Admit),
    );

    assert!(matches!(
        attach_logger(host.clone(), attachment_options),
        Err(InitError::AlreadyInitialized)
    ));
    guard
        .shutdown(Duration::from_secs(2))
        .expect("owned shutdown");
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("rejected attach retains no host Arc"));
    host.shutdown();
}
