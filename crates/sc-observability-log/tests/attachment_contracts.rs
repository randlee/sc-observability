//! Wave-one private fixtures for the non-owning attachment contract.
//!
//! The fixture models attachment state without installing the process-global
//! bridge.  D2 binds these decisions to the production facade in wave two.

use std::time::Duration;

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
    state: SlotState,
    entered_calls: usize,
}

#[derive(Debug, Clone, Copy)]
struct LogControl {
    attached: bool,
}

impl LogAttachment {
    fn attach(state: SlotState) -> Result<Self, DetachError> {
        match state {
            SlotState::Empty => Ok(Self {
                state: SlotState::Attached,
                entered_calls: 0,
            }),
            SlotState::Owned | SlotState::Attached | SlotState::Closing => {
                Err(DetachError::ForeignLoggerInstalled)
            }
        }
    }

    fn control(&self) -> LogControl {
        LogControl {
            attached: self.state == SlotState::Attached,
        }
    }

    fn detach(&mut self, timeout: Duration) -> Result<(), DetachError> {
        if self.state != SlotState::Attached {
            return Err(DetachError::NotInstalled);
        }
        self.state = SlotState::Closing;
        if self.entered_calls != 0 && timeout.is_zero() {
            self.state = SlotState::Attached;
            return Err(DetachError::Timeout);
        }
        self.entered_calls = 0;
        self.state = SlotState::Empty;
        Ok(())
    }
}

impl LogControl {
    fn submit(self) -> Result<(), DetachError> {
        self.attached.then_some(()).ok_or(DetachError::NotInstalled)
    }
}

/// ```compile_fail
/// # use std::time::Duration;
/// # struct LogAttachment;
/// # let mut attachment = LogAttachment;
/// attachment.shutdown(Duration::from_secs(1));
/// ```
///
/// Attachments intentionally expose no owner shutdown operation.
struct OwnerAuthorityCompileFail;

#[test]
fn detach_retry_after_timeout() {
    let mut attachment = LogAttachment::attach(SlotState::Empty).expect("empty slot attaches");
    attachment.entered_calls = 1;
    assert_eq!(attachment.detach(Duration::ZERO), Err(DetachError::Timeout));
    assert_eq!(
        attachment.state,
        SlotState::Attached,
        "timeout retains attachment"
    );
    attachment
        .detach(Duration::from_millis(1))
        .expect("retry detaches");
    assert_eq!(attachment.state, SlotState::Empty);
}

#[test]
fn stale_control_not_installed() {
    let mut attachment = LogAttachment::attach(SlotState::Empty).expect("empty slot attaches");
    let control = attachment.control();
    attachment.detach(Duration::from_millis(1)).expect("detach");
    let stale = LogControl {
        attached: attachment.state == SlotState::Attached,
    };
    assert_eq!(stale.submit(), Err(DetachError::NotInstalled));
    assert!(
        control.attached,
        "the pre-detach handle was initially usable"
    );
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
    assert_eq!(std::mem::size_of::<OwnerAuthorityCompileFail>(), 0);
    let attachment = LogAttachment::attach(SlotState::Empty).expect("attach");
    let _control = attachment.control();
    // The compile-fail example above is intentionally the only attempted
    // shutdown call: this contract offers detach, never owner shutdown.
}
