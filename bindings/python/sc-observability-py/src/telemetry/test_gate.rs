//! Test-hooks-only handshake at the real flush GIL-release boundary.
use std::sync::{Condvar, Mutex};
use std::time::Duration;
#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    Armed,
    Blocked,
    Released,
    Finished,
}
// Shared mutation is required for the Python control thread/native flush handshake.
#[derive(Default)]
pub(super) struct FlushGate {
    state: Mutex<State>,
    changed: Condvar,
}
const WATCHDOG: Duration = Duration::from_secs(5);
impl FlushGate {
    pub(super) fn arm(&self) {
        *self.state.lock().unwrap() = State::Armed;
    }
    pub(super) fn block(&self) {
        let mut state = self.state.lock().unwrap();
        if *state != State::Armed {
            return;
        }
        *state = State::Blocked;
        self.changed.notify_all();
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, WATCHDOG, |s| *s == State::Blocked)
            .unwrap();
        *state = State::Finished;
        self.changed.notify_all();
    }
    pub(super) fn wait_entered(&self) -> bool {
        let state = self.state.lock().unwrap();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, WATCHDOG, |s| *s == State::Armed)
            .unwrap();
        matches!(*state, State::Blocked | State::Released | State::Finished)
    }
    pub(super) fn is_blocked(&self) -> bool {
        *self.state.lock().unwrap() == State::Blocked
    }
    pub(super) fn release(&self) {
        *self.state.lock().unwrap() = State::Released;
        self.changed.notify_all();
    }
}
