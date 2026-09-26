//! Policy admission and panic containment fixtures for an attached host logger.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "integration fixtures keep setup failures explicit"
)]

use std::sync::Arc;
use std::time::Duration;

use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEvent, BridgeEventDecision, BridgeEventPolicy,
    BridgeOptions, EventLevel, LoggerConfig, PolicyRejection, ServiceName, TargetCategory,
    attach_logger,
};
use sc_observability_types::LogEvent;

struct Deny;

impl BridgeEventPolicy for Deny {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        BridgeEventDecision::Reject(PolicyRejection::Denied)
    }
}

struct PanicPolicy;

impl BridgeEventPolicy for PanicPolicy {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        panic!("policy panic fixture")
    }
}

fn event() -> BridgeEvent {
    BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("policy.test").expect("target"),
        action: Some(ActionName::new("policy.event").expect("action")),
        message: Some("policy event".to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    }
}

fn attach_with(
    policy: Arc<dyn BridgeEventPolicy>,
) -> (
    sc_observability_log::LogAttachment,
    Arc<sc_observability::Logger>,
) {
    let root = tempfile::tempdir().expect("temp root");
    let root = Box::leak(Box::new(root));
    let logger = Arc::new(
        sc_observability::Logger::new_typed(LoggerConfig::default_for(
            ServiceName::new("policy").expect("service"),
            root.path().to_path_buf(),
        ))
        .expect("host logger"),
    );
    // Keep the host Arc in the attachment fixture; successful detach proves
    // that the attachment itself released its Arc in the lifecycle fixture.
    let options = AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("log.record").expect("action"),
            parse_bracket_action: true,
        },
        policy,
    );
    let attachment = attach_logger(Arc::clone(&logger), options).expect("attach");
    (attachment, logger)
}

#[test]
fn policy_rejection_and_panic_are_counted_once_at_the_boundary() {
    let (mut attachment, host) = attach_with(Arc::new(Deny));
    let control = attachment.control();
    let before = control
        .dropped_events()
        .get(sc_observability_log::DropCause::InvalidEvent);
    assert!(matches!(
        control.try_log(event()),
        Err(sc_observability_log::EmitError::InvalidEvent { .. })
    ));
    let after = control
        .dropped_events()
        .get(sc_observability_log::DropCause::InvalidEvent);
    assert_eq!(after, before + 1);
    attachment.detach(Duration::from_secs(2)).expect("detach");
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    let _ = host.shutdown();

    let (mut attachment, host) = attach_with(Arc::new(PanicPolicy));
    let control = attachment.control();
    assert!(matches!(
        control.try_log(event()),
        Err(sc_observability_log::EmitError::Panicked)
    ));
    attachment.detach(Duration::from_secs(2)).expect("detach");
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    let _ = host.shutdown();
}
