// R-A4-005: a `LogControl` has no runtime-level mutation authority.
use sc_observability_log::{LevelChangeSource, LevelFilter, LogControl};

fn mutate_through_control(control: LogControl) {
    let _ = control.elevate_level(LevelFilter::Info, LevelChangeSource::Application);
    let _ = control.reset_level(LevelChangeSource::Application);
}

fn main() {}
