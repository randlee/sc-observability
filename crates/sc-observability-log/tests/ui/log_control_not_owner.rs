#![allow(deprecated)] // The compile-fail assertion pins the retained v1 path.

// R-A4-005: a `LogControl` cannot shut the logger down or be turned into a `LogGuard`.
use std::time::Duration;

use sc_observability_log::{LogControl, LogGuard};

fn shut_down_through_control(control: LogControl) {
    let _ = control.shutdown(Duration::from_secs(1));
}

fn control_into_owner(control: LogControl) -> LogGuard {
    control.into()
}

fn main() {}
