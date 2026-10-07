//! Compile-only public-signature proof for the opt-in bridge facade.
#![cfg(feature = "v1")]

use std::sync::Arc;
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
    let _: fn(&LogGuard) -> Result<(), FlushError> = LogGuard::flush;
    let _: fn(&LogGuard, Duration) -> Result<(), FlushError> = LogGuard::flush_with_timeout;
    let _: fn(&LogGuard) -> Result<(), ShutdownError> = LogGuard::shutdown;
    let _: fn(&LogGuard, Duration) -> Result<(), ShutdownError> = LogGuard::shutdown_with_timeout;
    let _: fn(&LogControl) -> Result<(), FlushError> = LogControl::flush;
    let _: fn(&LogControl, Duration) -> Result<(), FlushError> = LogControl::flush_with_timeout;
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
    let guard = Arc::new(
        sc_observability_log::v2::init(
            config,
            BridgeOptions {
                default_action: ActionName::new("canonical.record").expect("action"),
                parse_bracket_action: false,
            },
        )
        .expect("canonical init"),
    );
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
        .flush_with_timeout(Duration::from_secs(2))
        .expect("canonical flush");
    let shutdown_guard = Arc::clone(&guard);
    std::thread::spawn(move || shutdown_guard.shutdown_with_timeout(Duration::from_secs(2)))
        .join()
        .expect("shutdown thread must not panic")
        .expect("canonical shutdown");
    guard.shutdown().expect("second shutdown is idempotent");
    assert!(
        control
            .try_log(BridgeEvent {
                level: EventLevel::Info,
                target: TargetCategory::new("canonical.test").expect("target"),
                action: None,
                message: Some("canonical bridge after shutdown".to_owned()),
                outcome: None,
                fields: serde_json::Map::new(),
                request_id: None,
                correlation_id: None,
                trace: None,
            })
            .is_err()
    );
    assert!(matches!(
        control.flush_with_timeout(Duration::from_secs(1)),
        Err(FlushError::Drain { .. })
    ));
}
