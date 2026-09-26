//! Policy admission and panic containment fixtures for an attached host logger.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "integration fixtures keep setup failures explicit"
)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[allow(deprecated)]
use sc_observability::{LogSink, SinkHealth, SinkHealthState, SinkName, SinkRegistration};
use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEvent, BridgeEventDecision, BridgeEventPolicy,
    BridgeOptions, EventLevel, LoggerConfig, PolicyRejection, ServiceName, TargetCategory,
    attach_logger,
};
use sc_observability_types::LogEvent;
use serde_json::json;

static TEST_LOCK: Mutex<()> = Mutex::new(());

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

struct RecordingSink {
    events: Arc<Mutex<Vec<LogEvent>>>,
}

#[allow(deprecated)]
impl LogSink for RecordingSink {
    fn write(&self, event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
        self.events
            .lock()
            .expect("recording lock")
            .push(event.clone());
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("policy-recording").expect("sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

struct AllowlistAndBound {
    max_message_bytes: usize,
}

impl BridgeEventPolicy for AllowlistAndBound {
    fn decide(&self, event: &LogEvent) -> BridgeEventDecision {
        if event.target.as_str() != "policy.test" {
            return BridgeEventDecision::Reject(PolicyRejection::Denied);
        }
        if event
            .message
            .as_deref()
            .is_some_and(|message| message.len() > self.max_message_bytes)
        {
            return BridgeEventDecision::Reject(PolicyRejection::PayloadTooLarge);
        }
        BridgeEventDecision::Admit
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
    Arc<Mutex<Vec<LogEvent>>>,
) {
    attach_with_config(policy, |_| {})
}

fn attach_with_config(
    policy: Arc<dyn BridgeEventPolicy>,
    configure: impl FnOnce(&mut LoggerConfig),
) -> (
    sc_observability_log::LogAttachment,
    Arc<sc_observability::Logger>,
    Arc<Mutex<Vec<LogEvent>>>,
) {
    let root = tempfile::tempdir().expect("temp root");
    let root = Box::leak(Box::new(root));
    let mut config = LoggerConfig::default_for(
        ServiceName::new("policy").expect("service"),
        root.path().to_path_buf(),
    );
    configure(&mut config);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut builder = sc_observability::LoggerBuilder::new_typed(config).expect("builder");
    builder.register_sink(SinkRegistration::new(Arc::new(RecordingSink {
        events: Arc::clone(&events),
    })));
    let logger = Arc::new(builder.build_typed().expect("host logger"));
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
    (attachment, logger, events)
}

#[test]
fn policy_rejection_and_panic_are_counted_once_at_the_boundary() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let (mut attachment, host, events) = attach_with(Arc::new(Deny));
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
    assert!(events.lock().expect("recording lock").is_empty());
    attachment.detach(Duration::from_secs(2)).expect("detach");
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    let _ = host.shutdown();

    let (mut attachment, host, events) = attach_with(Arc::new(PanicPolicy));
    let control = attachment.control();
    assert!(matches!(
        control.try_log(event()),
        Err(sc_observability_log::EmitError::Panicked)
    ));
    assert!(events.lock().expect("recording lock").is_empty());
    attachment.detach(Duration::from_secs(2)).expect("detach");
    let host =
        Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases attachment logger"));
    let _ = host.shutdown();
}

#[test]
fn policy_allowlist_and_bound_run_before_host_redaction_and_sink_admission() {
    let _serial = TEST_LOCK.lock().expect("test lock");
    let (mut attachment, host, events) = attach_with_config(
        Arc::new(AllowlistAndBound {
            max_message_bytes: 64,
        }),
        |config| config.redaction.denylist_keys.push("token".to_owned()),
    );
    let control = attachment.control();
    let before = control
        .dropped_events()
        .get(sc_observability_log::DropCause::InvalidEvent);

    let mut denied = event();
    denied.target = TargetCategory::new("policy.other").expect("target");
    assert!(matches!(
        control.try_log(denied),
        Err(sc_observability_log::EmitError::InvalidEvent { .. })
    ));

    let mut oversized = event();
    oversized.message = Some("x".repeat(65));
    assert!(matches!(
        control.try_log(oversized),
        Err(sc_observability_log::EmitError::InvalidEvent { .. })
    ));
    assert_eq!(
        control
            .dropped_events()
            .get(sc_observability_log::DropCause::InvalidEvent),
        before + 2
    );
    assert!(events.lock().expect("recording lock").is_empty());

    let path = host.health().active_log_path;
    let mut admitted = event();
    admitted.message = Some("Bearer message-secret".to_owned());
    admitted
        .fields
        .insert("token".to_owned(), json!("Bearer field-secret"));
    control.try_log(admitted).expect("allowlisted event");
    control.flush(Duration::from_secs(2)).expect("flush");
    let events = events.lock().expect("recording lock");
    assert_eq!(events.len(), 1, "only the admitted event reaches the sink");
    assert_eq!(events[0].message.as_deref(), Some("Bearer [REDACTED]"));
    assert!(
        !events[0]
            .fields
            .values()
            .any(|value| { value.as_str() == Some("Bearer field-secret") })
    );

    attachment.detach(Duration::from_secs(2)).expect("detach");
    let host = Arc::try_unwrap(host).unwrap_or_else(|_| panic!("detach releases logger"));
    host.shutdown();
    let output = std::fs::read_to_string(path).expect("read redacted log");
    assert!(output.contains("Bearer [REDACTED]"));
    assert!(!output.contains("message-secret"));
    assert!(!output.contains("field-secret"));
}
