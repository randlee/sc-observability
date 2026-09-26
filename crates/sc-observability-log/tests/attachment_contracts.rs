//! Wave-one private fixtures for the non-owning attachment contract.
//!
//! The fixture models attachment state without installing the process-global
//! bridge.  D2 binds these decisions to the production facade in wave two.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use sc_observability_log::BridgeOptions;
use sc_observability_types::{
    ActionName, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, SchemaVersion,
    ServiceName, TargetCategory, Timestamp,
};
use serde_json::Map;

trait BridgeEventPolicy: Send + Sync {
    fn decide(&self, event: &LogEvent) -> BridgeEventDecision;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeEventDecision {
    Admit,
    Reject(PolicyRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

struct AttachmentOptions {
    bridge: BridgeOptions,
    policy: Arc<dyn BridgeEventPolicy>,
}

impl AttachmentOptions {
    fn new(bridge: BridgeOptions, policy: Arc<dyn BridgeEventPolicy>) -> Self {
        Self { bridge, policy }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Empty,
    Owned,
    Attached,
    Closing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetachError {
    Timeout,
    NotInstalled,
    ForeignLoggerInstalled,
}

#[derive(Debug)]
struct LogAttachment {
    state: Arc<Mutex<AttachmentState>>,
}

#[derive(Debug)]
struct AttachmentState {
    slot: SlotState,
    entered_calls: usize,
}

#[derive(Debug, Clone)]
struct LogControl {
    state: Weak<Mutex<AttachmentState>>,
}

impl LogAttachment {
    fn attach(state: SlotState) -> Result<Self, DetachError> {
        match state {
            SlotState::Empty => Ok(Self {
                state: Arc::new(Mutex::new(AttachmentState {
                    slot: SlotState::Attached,
                    entered_calls: 0,
                })),
            }),
            SlotState::Owned | SlotState::Attached | SlotState::Closing => {
                Err(DetachError::ForeignLoggerInstalled)
            }
        }
    }

    fn control(&self) -> LogControl {
        LogControl {
            state: Arc::downgrade(&self.state),
        }
    }

    fn slot(&self) -> SlotState {
        self.state.lock().expect("fixture state lock").slot
    }

    fn set_entered_calls(&mut self, entered_calls: usize) {
        self.state.lock().expect("fixture state lock").entered_calls = entered_calls;
    }

    fn detach(&mut self, timeout: Duration) -> Result<(), DetachError> {
        let mut state = self.state.lock().expect("fixture state lock");
        if state.slot != SlotState::Attached {
            return Err(DetachError::NotInstalled);
        }
        state.slot = SlotState::Closing;
        if state.entered_calls != 0 && timeout.is_zero() {
            state.slot = SlotState::Attached;
            return Err(DetachError::Timeout);
        }
        state.entered_calls = 0;
        state.slot = SlotState::Empty;
        Ok(())
    }
}

impl LogControl {
    fn submit(&self) -> Result<(), DetachError> {
        let state = self.state.upgrade().ok_or(DetachError::NotInstalled)?;
        (state.lock().expect("fixture state lock").slot == SlotState::Attached)
            .then_some(())
            .ok_or(DetachError::NotInstalled)
    }
}

struct DenyPolicy;

impl BridgeEventPolicy for DenyPolicy {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        BridgeEventDecision::Reject(PolicyRejection::Denied)
    }
}

fn assert_open_policy(_: Arc<dyn BridgeEventPolicy>) {}

fn contract_event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("static schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("contract").expect("static service name"),
        target: TargetCategory::new("contract").expect("static target category"),
        action: ActionName::new("attachment").expect("static action name"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Map::default(),
    }
}

#[test]
fn detach_retry_after_timeout() {
    let mut attachment = LogAttachment::attach(SlotState::Empty).expect("empty slot attaches");
    attachment.set_entered_calls(1);
    assert_eq!(attachment.detach(Duration::ZERO), Err(DetachError::Timeout));
    assert_eq!(
        attachment.slot(),
        SlotState::Attached,
        "timeout retains attachment"
    );
    attachment
        .detach(Duration::from_millis(1))
        .expect("retry detaches");
    assert_eq!(attachment.slot(), SlotState::Empty);
}

#[test]
fn stale_control_not_installed() {
    let mut attachment = LogAttachment::attach(SlotState::Empty).expect("empty slot attaches");
    let control = attachment.control();
    control
        .submit()
        .expect("saved control is live before detach");
    attachment.detach(Duration::from_millis(1)).expect("detach");
    assert_eq!(control.submit(), Err(DetachError::NotInstalled));
}

#[test]
fn foreign_logger_rejected() {
    for state in [SlotState::Owned, SlotState::Attached, SlotState::Closing] {
        assert_eq!(
            LogAttachment::attach(state).map(|_| ()),
            Err(DetachError::ForeignLoggerInstalled)
        );
    }
}

#[test]
fn attachment_has_no_owner_authority() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/log_control_not_owner.rs");
}

#[test]
fn attachment_options_accept_open_policy() {
    let options = AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("contract").expect("static action name"),
            parse_bracket_action: true,
        },
        Arc::new(DenyPolicy),
    );
    assert_open_policy(Arc::clone(&options.policy));
    assert!(options.bridge.parse_bracket_action);
    assert_eq!(
        options.policy.decide(&contract_event()),
        BridgeEventDecision::Reject(PolicyRejection::Denied)
    );
    let _ = BridgeEventDecision::Admit;
    let _ = PolicyRejection::PayloadTooLarge;
    let _ = PolicyRejection::Invalid;
}
