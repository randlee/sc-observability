// D13 compiles the attachment contract as a private fixture until D2 binds the
// production facade. Include that fixture so these failures stay attached to
// the same frozen surface rather than duplicating a look-alike API here.
#[path = "../attachment_contracts.rs"]
mod attachment_contracts;

use std::time::Duration;

use attachment_contracts::{FixtureLogAttachment, FixtureLogControl};

fn attachment_cannot_control_level_or_shutdown(attachment: FixtureLogAttachment) {
    let _ = attachment.elevate_level();
    let _ = attachment.reset_level();
    let _ = attachment.shutdown(Duration::ZERO);
}

fn control_cannot_elevate_level(control: FixtureLogControl) {
    let _ = control.elevate_level();
}

fn main() {}
