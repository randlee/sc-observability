//! Foreign process-global facade rejection for host attachment.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration fixture keeps setup failures explicit"
)]

use std::sync::Arc;

use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEventPolicy, BridgeOptions, InitError, LoggerConfig,
    ServiceName, attach_logger,
};
use sc_observability_types::LogEvent;

struct Admit;

impl BridgeEventPolicy for Admit {
    fn decide(&self, _: &LogEvent) -> sc_observability_log::BridgeEventDecision {
        sc_observability_log::BridgeEventDecision::Admit
    }
}

struct Foreign;

impl log::Log for Foreign {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, _: &log::Record<'_>) {}

    fn flush(&self) {}
}

static FOREIGN: Foreign = Foreign;

#[test]
fn attachment_rejects_a_foreign_process_global_logger() {
    log::set_logger(&FOREIGN).expect("foreign logger installs in fresh test process");
    log::set_max_level(log::LevelFilter::Trace);
    let host = Arc::new(
        sc_observability::Logger::new_typed(LoggerConfig::default_for(
            ServiceName::new("foreign-facade").expect("service"),
            tempfile::tempdir().expect("temp root").path().to_path_buf(),
        ))
        .expect("host logger"),
    );
    let options = AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("log.record").expect("action"),
            parse_bracket_action: false,
        },
        Arc::new(Admit),
    );

    assert!(matches!(
        attach_logger(Arc::clone(&host), options),
        Err(InitError::ForeignLoggerInstalled)
    ));
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("failed attach retains no host Arc"));
    host.shutdown();
}
