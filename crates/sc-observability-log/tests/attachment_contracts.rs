//! Wave-one private fixtures for the non-owning attachment contract.
//!
//! The fixture models attachment state without installing the process-global
//! bridge.  D2 binds these decisions to the production facade in wave two.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use sc_observability_log::BridgeOptions;
use sc_observability_types::{
    ActionName, ErrorCode, ErrorContext, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION,
    ProcessIdentity, Remediation, SchemaVersion, ServiceName, TargetCategory, Timestamp,
};
use serde_json::Map;

trait FixtureBridgeEventPolicy: Send + Sync {
    fn decide(&self, event: &LogEvent) -> FixtureBridgeEventDecision;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureBridgeEventDecision {
    Admit,
    Reject(FixturePolicyRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixturePolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

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

fn foreign_logger() -> FixtureDetachError {
    FixtureDetachError::ForeignLoggerInstalled(detach_error(
        "SC_LOG_FOREIGN_LOGGER_INSTALLED",
        "foreign logger is installed",
        Remediation::not_recoverable("detach the existing logger"),
    ))
}

#[derive(Debug)]
pub struct FixtureLogAttachment {
    state: Arc<Mutex<FixtureAttachmentState>>,
}

#[derive(Debug)]
struct FixtureAttachmentState {
    slot: FixtureSlotState,
    entered_calls: usize,
}

#[derive(Debug, Clone)]
pub struct FixtureLogControl {
    state: Weak<Mutex<FixtureAttachmentState>>,
}

impl FixtureLogAttachment {
    fn attach(state: FixtureSlotState) -> Result<Self, FixtureDetachError> {
        match state {
            FixtureSlotState::Empty => Ok(Self {
                state: Arc::new(Mutex::new(FixtureAttachmentState {
                    slot: FixtureSlotState::Attached,
                    entered_calls: 0,
                })),
            }),
            FixtureSlotState::Owned | FixtureSlotState::Attached | FixtureSlotState::Closing => {
                Err(foreign_logger())
            }
        }
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
        (state.lock().expect("fixture state lock").slot == FixtureSlotState::Attached)
            .then_some(())
            .ok_or_else(not_installed)
    }
}

struct FixtureDenyPolicy;

impl FixtureBridgeEventPolicy for FixtureDenyPolicy {
    fn decide(&self, _: &LogEvent) -> FixtureBridgeEventDecision {
        FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Denied)
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

fn assert_detach_error(error: &FixtureDetachError, code: &str) {
    assert_eq!(error.context().diagnostic().code.as_str(), code);
    assert!(matches!(
        error.context().diagnostic().remediation,
        Remediation::Recoverable { .. } | Remediation::NotRecoverable { .. }
    ));
}

#[test]
fn detach_retry_after_timeout() {
    let mut attachment =
        FixtureLogAttachment::attach(FixtureSlotState::Empty).expect("empty slot attaches");
    let control = attachment.control();
    attachment.set_entered_calls(1);
    assert_detach_error(
        &attachment.detach(Duration::ZERO).unwrap_err(),
        "SC_LOG_DETACH_TIMEOUT",
    );
    assert_eq!(
        attachment.slot(),
        FixtureSlotState::Closing,
        "timeout retains the attachment with admission closed"
    );
    assert_detach_error(
        &control.submit().unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
    );
    attachment
        .detach(Duration::from_millis(1))
        .expect("retry detaches");
    assert_eq!(attachment.slot(), FixtureSlotState::Empty);
    assert_detach_error(
        &attachment.detach(Duration::from_millis(1)).unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
    );
}

#[test]
fn stale_control_not_installed() {
    let mut attachment =
        FixtureLogAttachment::attach(FixtureSlotState::Empty).expect("empty slot attaches");
    let control = attachment.control();
    control
        .submit()
        .expect("saved control is live before detach");
    attachment.detach(Duration::from_millis(1)).expect("detach");
    assert_detach_error(
        &control.submit().unwrap_err(),
        "SC_LOG_DETACH_NOT_INSTALLED",
    );
}

#[test]
fn foreign_logger_rejected() {
    for state in [
        FixtureSlotState::Owned,
        FixtureSlotState::Attached,
        FixtureSlotState::Closing,
    ] {
        assert_detach_error(
            &FixtureLogAttachment::attach(state).unwrap_err(),
            "SC_LOG_FOREIGN_LOGGER_INSTALLED",
        );
    }
}

#[test]
fn attachment_options_accept_open_policy() {
    let options = FixtureAttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("contract").expect("static action name"),
            parse_bracket_action: true,
        },
        Arc::new(FixtureDenyPolicy),
    );
    assert_open_policy(Arc::clone(&options.policy));
    assert!(options.bridge.parse_bracket_action);
    assert_eq!(
        options.policy.decide(&contract_event()),
        FixtureBridgeEventDecision::Reject(FixturePolicyRejection::Denied)
    );
    let _ = FixtureBridgeEventDecision::Admit;
    let _ = FixturePolicyRejection::PayloadTooLarge;
    let _ = FixturePolicyRejection::Invalid;
}
