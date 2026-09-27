//! Wave-one private fixtures for the non-owning attachment contract.
//!
//! The fixture models attachment state without installing the process-global
//! bridge.  D2 binds these decisions to the production facade in wave two.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use sc_observability_log::{BridgeOptions, InitError};
use sc_observability_types::{
    ActionName, ErrorCode, ErrorContext, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION,
    ProcessIdentity, Remediation, SchemaVersion, ServiceName, TargetCategory, Timestamp,
};
use serde_json::Map;

trait FixtureBridgeEventPolicy: Send + Sync {
    fn decide(&self, event: &LogEvent) -> FixtureBridgeEventDecision;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
enum FixtureBridgeEventDecision {
    Admit,
    Reject(FixturePolicyRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
enum FixturePolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

#[non_exhaustive]
struct FixtureAttachmentOptions {
    bridge: BridgeOptions,
    policy: Arc<dyn FixtureBridgeEventPolicy>,
}

impl FixtureAttachmentOptions {
    fn new(bridge: BridgeOptions, policy: Arc<dyn FixtureBridgeEventPolicy>) -> Self {
        Self { bridge, policy }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureSlotState {
    Empty,
    Owned,
    Attached,
    Closing,
}

#[derive(Debug)]
enum FixtureDetachError {
    Timeout(Box<ErrorContext>),
    NotInstalled(Box<ErrorContext>),
    ForeignLoggerInstalled(Box<ErrorContext>),
}

impl FixtureDetachError {
    fn context(&self) -> &ErrorContext {
        match self {
            Self::Timeout(context)
            | Self::NotInstalled(context)
            | Self::ForeignLoggerInstalled(context) => context,
        }
    }
}

fn detach_error(
    code: &'static str,
    message: &'static str,
    remediation: Remediation,
) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        ErrorCode::new_static(code),
        message,
        remediation,
    ))
}

fn detach_timeout() -> FixtureDetachError {
    FixtureDetachError::Timeout(detach_error(
        "SC_LOG_DETACH_TIMEOUT",
        "detach timed out",
        Remediation::recoverable("retry detach", ["wait for drain"]),
    ))
}

fn not_installed() -> FixtureDetachError {
    FixtureDetachError::NotInstalled(detach_error(
        "SC_LOG_DETACH_NOT_INSTALLED",
        "attachment is not installed",
        Remediation::not_recoverable("attach before using control"),
    ))
}

fn foreign_logger_installed() -> FixtureDetachError {
    FixtureDetachError::ForeignLoggerInstalled(detach_error(
        "SC_LOG_FOREIGN_LOGGER_INSTALLED",
        "another logger owns the logging facade",
        Remediation::recoverable("remove the competing logger", ["use the logger owner"]),
    ))
}

#[derive(Debug)]
pub struct FixtureLogAttachment {
    // Models the production bridge slot shared by the attachment, the slot,
    // and saved controls: controls retain a Weak handle to observe detach.
    state: Arc<Mutex<FixtureAttachmentState>>,
}

#[derive(Debug)]
struct FixtureAttachmentState {
    slot: FixtureSlotState,
    entered_calls: usize,
    foreign_logger_installed: bool,
}

#[derive(Debug, Clone)]
struct FixtureBridgeSlot {
    state: Arc<Mutex<FixtureAttachmentState>>,
}

impl FixtureBridgeSlot {
    fn empty() -> Self {
        Self::new(FixtureSlotState::Empty, false)
    }

    fn with_state(slot: FixtureSlotState) -> Self {
        Self::new(slot, false)
    }

    fn with_foreign_logger() -> Self {
        Self::new(FixtureSlotState::Empty, true)
    }

    fn new(slot: FixtureSlotState, foreign_logger_installed: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(FixtureAttachmentState {
                slot,
                entered_calls: 0,
                foreign_logger_installed,
            })),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixtureLogControl {
    state: Weak<Mutex<FixtureAttachmentState>>,
}

impl FixtureLogAttachment {
    fn attach(slot: &FixtureBridgeSlot) -> Result<Self, InitError> {
        let state = Arc::clone(&slot.state);
        let mut slot_state = state.lock().expect("fixture state lock");
        if slot_state.foreign_logger_installed {
            return Err(InitError::ForeignLoggerInstalled);
        }
        match slot_state.slot {
            FixtureSlotState::Empty => slot_state.slot = FixtureSlotState::Attached,
            FixtureSlotState::Owned | FixtureSlotState::Attached | FixtureSlotState::Closing => {
                return Err(InitError::AlreadyInitialized);
            }
        }
        drop(slot_state);
        Ok(Self { state })
    }

    fn control(&self) -> FixtureLogControl {
        FixtureLogControl {
            state: Arc::downgrade(&self.state),
        }
    }

    fn slot(&self) -> FixtureSlotState {
        self.state.lock().expect("fixture state lock").slot
    }

    fn set_entered_calls(&mut self, entered_calls: usize) {
        self.state.lock().expect("fixture state lock").entered_calls = entered_calls;
    }

    fn detach(&mut self, timeout: Duration) -> Result<(), FixtureDetachError> {
        let mut state = self.state.lock().expect("fixture state lock");
        match state.slot {
            FixtureSlotState::Attached => state.slot = FixtureSlotState::Closing,
            // A timed-out detach retains the attachment with admission closed,
            // so the caller can retry its bounded drain from this state.
            FixtureSlotState::Closing => {}
            FixtureSlotState::Empty | FixtureSlotState::Owned => return Err(not_installed()),
        }
        if state.entered_calls != 0 && timeout.is_zero() {
            return Err(detach_timeout());
        }
        state.entered_calls = 0;
        state.slot = FixtureSlotState::Empty;
        Ok(())
    }
}

impl FixtureLogControl {
    fn submit(&self) -> Result<(), FixtureDetachError> {
        let state = self.state.upgrade().ok_or_else(not_installed)?;
        let slot = state.lock().expect("fixture state lock").slot;
        if slot == FixtureSlotState::Attached {
            Ok(())
        } else {
            Err(not_installed())
        }
    }
}

struct FixtureDenyPolicy;

impl FixtureBridgeEventPolicy for FixtureDenyPolicy {
    fn decide(&self, _: &LogEvent) -> FixtureBridgeEventDecision {
        FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Denied)
    }
}

struct FixtureAdmitPolicy;

impl FixtureBridgeEventPolicy for FixtureAdmitPolicy {
    fn decide(&self, _: &LogEvent) -> FixtureBridgeEventDecision {
        FixtureBridgeEventDecision::Admit
    }
}

struct FixturePayloadTooLargePolicy;

impl FixtureBridgeEventPolicy for FixturePayloadTooLargePolicy {
    fn decide(&self, _: &LogEvent) -> FixtureBridgeEventDecision {
        FixtureBridgeEventDecision::Reject(FixturePolicyRejection::PayloadTooLarge)
    }
}

struct FixtureInvalidPolicy;

impl FixtureBridgeEventPolicy for FixtureInvalidPolicy {
    fn decide(&self, _: &LogEvent) -> FixtureBridgeEventDecision {
        FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Invalid)
    }
}

fn assert_open_policy(_: Arc<dyn FixtureBridgeEventPolicy>) {}

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

fn assert_detach_error(error: &FixtureDetachError, code: &str, remediation: &Remediation) {
    let diagnostic = error.context().diagnostic();
    assert_eq!(diagnostic.code.as_str(), code);
    assert_eq!(&diagnostic.remediation, remediation);
}

#[test]
fn detach_retry_after_timeout() {
    let slot = FixtureBridgeSlot::empty();
    let mut attachment = FixtureLogAttachment::attach(&slot).expect("empty slot attaches");
    let control = attachment.control();
    attachment.set_entered_calls(1);
    assert_detach_error(
        &attachment.detach(Duration::ZERO).unwrap_err(),
        "SC_LOG_DETACH_TIMEOUT",
        &Remediation::recoverable("retry detach", ["wait for drain"]),
    );
    assert_eq!(
        attachment.slot(),
        FixtureSlotState::Closing,
        "timeout retains the attachment with admission closed"
    );
    assert_detach_error(
        &control.submit().unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
        &Remediation::not_recoverable("attach before using control"),
    );
    attachment
        .detach(Duration::from_millis(1))
        .expect("retry detaches");
    assert_eq!(attachment.slot(), FixtureSlotState::Empty);
    assert_detach_error(
        &attachment.detach(Duration::from_millis(1)).unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
        &Remediation::not_recoverable("attach before using control"),
    );
}

#[test]
fn stale_control_not_installed() {
    let slot = FixtureBridgeSlot::empty();
    let mut attachment = FixtureLogAttachment::attach(&slot).expect("empty slot attaches");
    let control = attachment.control();
    control
        .submit()
        .expect("saved control is live before detach");
    attachment.detach(Duration::from_millis(1)).expect("detach");
    assert_detach_error(
        &control.submit().unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
        &Remediation::not_recoverable("attach before using control"),
    );
}

#[test]
fn foreign_logger_rejected() {
    for state in [
        FixtureSlotState::Owned,
        FixtureSlotState::Attached,
        FixtureSlotState::Closing,
    ] {
        let slot = FixtureBridgeSlot::with_state(state);
        assert!(matches!(
            FixtureLogAttachment::attach(&slot),
            Err(InitError::AlreadyInitialized)
        ));
    }

    let foreign_logger_slot = FixtureBridgeSlot::with_foreign_logger();
    assert_detach_error(
        &foreign_logger_installed(),
        "SC_LOG_FOREIGN_LOGGER_INSTALLED",
        &Remediation::recoverable("remove the competing logger", ["use the logger owner"]),
    );
    assert!(matches!(
        FixtureLogAttachment::attach(&foreign_logger_slot),
        Err(InitError::ForeignLoggerInstalled)
    ));
}

#[test]
fn detached_slot_reattaches() {
    let slot = FixtureBridgeSlot::empty();
    let mut attachment = FixtureLogAttachment::attach(&slot).expect("empty slot attaches");
    attachment.detach(Duration::from_millis(1)).expect("detach");
    FixtureLogAttachment::attach(&slot).expect("detached slot reattaches");
}

#[test]
fn attachment_options_accept_open_policy() {
    let bridge = BridgeOptions {
        default_action: ActionName::new("contract").expect("static action name"),
        parse_bracket_action: true,
    };

    for (policy, expected) in [
        (
            Arc::new(FixtureAdmitPolicy) as Arc<dyn FixtureBridgeEventPolicy>,
            FixtureBridgeEventDecision::Admit,
        ),
        (
            Arc::new(FixtureDenyPolicy) as Arc<dyn FixtureBridgeEventPolicy>,
            FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Denied),
        ),
        (
            Arc::new(FixturePayloadTooLargePolicy) as Arc<dyn FixtureBridgeEventPolicy>,
            FixtureBridgeEventDecision::Reject(FixturePolicyRejection::PayloadTooLarge),
        ),
        (
            Arc::new(FixtureInvalidPolicy) as Arc<dyn FixtureBridgeEventPolicy>,
            FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Invalid),
        ),
    ] {
        let options = FixtureAttachmentOptions::new(bridge.clone(), policy);
        assert_open_policy(Arc::clone(&options.policy));
        assert!(options.bridge.parse_bracket_action);
        assert_eq!(options.policy.decide(&contract_event()), expected);
    }
}
