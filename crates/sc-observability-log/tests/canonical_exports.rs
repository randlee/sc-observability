//! Compile-only public-signature proof for the opt-in bridge facade.

use std::time::Duration;

use sc_observability_log::v2::{
    BridgeEvent, BridgeOptions, FlushError, LogControl, LogGuard, ShutdownError,
};

fn requires_clone<T: Clone>() {}

#[test]
fn canonical_log_exports_have_real_public_signatures() {
    let _: fn(&LogGuard) -> LogControl = LogGuard::control;
    let _: fn(&LogGuard, Duration) -> Result<(), FlushError> = LogGuard::flush;
    let _: fn(LogGuard, Duration) -> Result<(), ShutdownError> = LogGuard::shutdown;
    let _: fn(&LogControl, Duration) -> Result<(), FlushError> = LogControl::flush;

    requires_clone::<BridgeEvent>();
    requires_clone::<BridgeOptions>();
    requires_clone::<LogControl>();
}
