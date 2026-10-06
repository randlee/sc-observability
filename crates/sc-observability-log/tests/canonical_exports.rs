//! Compile-only public-signature proof for the opt-in bridge facade.
#![cfg(feature = "v1")]

use std::time::Duration;

use sc_observability_log::v2::{
    BridgeEvent, BridgeOptions, FlushError, LogAttachment, LogControl, LogGuard, ShutdownError,
};
use sc_observability_log::{ActionName, EventLevel, LoggerConfig, ServiceName, TargetCategory};

fn requires_clone<T: Clone>() {}

#[test]
#[allow(deprecated)] // Retained v1 conversion proof; the canonical assertions use v2.
fn canonical_log_exports_have_real_public_signatures() {
    let _: fn(&LogGuard) -> LogControl = LogGuard::control;
    let _: fn(&LogGuard, Duration) -> Result<(), FlushError> = LogGuard::flush;
    let _: fn(LogGuard, Duration) -> Result<(), ShutdownError> = LogGuard::shutdown;
    let _: fn(&LogControl, Duration) -> Result<(), FlushError> = LogControl::flush;
    let _: fn(&LogAttachment) -> LogControl = LogAttachment::control;
    let _: fn(sc_observability_log::LogControl) -> LogControl =
        sc_observability_log::LogControl::into_v2;

    requires_clone::<BridgeEvent>();
    requires_clone::<BridgeOptions>();
    requires_clone::<LogControl>();

    let directory = tempfile::tempdir().expect("temporary log directory");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("canonical-api").expect("service name"),
        directory.path().to_path_buf(),
    );
    config.enable_console_sink = false;
    let guard = sc_observability_log::v2::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("canonical.record").expect("action"),
            parse_bracket_action: false,
        },
    )
    .expect("canonical init");
    let control = guard.control();
    control
        .try_log(BridgeEvent {
            level: EventLevel::Info,
            target: TargetCategory::new("canonical.test").expect("target"),
            action: None,
            message: Some("canonical bridge path".to_owned()),
            outcome: None,
            fields: serde_json::Map::new(),
            request_id: None,
            correlation_id: None,
            trace: None,
        })
        .expect("canonical admission");
    control
        .flush(Duration::from_secs(2))
        .expect("canonical flush");
    guard
        .shutdown(Duration::from_secs(2))
        .expect("canonical shutdown");
    assert!(matches!(
        control.flush(Duration::from_secs(1)),
        Err(FlushError::Drain { .. })
    ));
}
