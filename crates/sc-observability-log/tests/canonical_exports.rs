//! Compile-only public-signature proof for the opt-in bridge facade.

use std::time::Duration;

use sc_observability_log::v2::{BridgeEvent, BridgeOptions, FlushError, LogControl, LogGuard, ShutdownError};

#[test]
fn canonical_log_exports_have_real_public_signatures() {
    let _control: fn(&LogGuard) -> LogControl = LogGuard::control;
    let _guard_flush: fn(&LogGuard, Duration) -> Result<(), FlushError> = LogGuard::flush;
    let _guard_shutdown: fn(LogGuard, Duration) -> Result<(), ShutdownError> = LogGuard::shutdown;
    let _control_flush: fn(&LogControl, Duration) -> Result<(), FlushError> = LogControl::flush;

    fn requires_clone<T: Clone>() {}
    requires_clone::<BridgeEvent>();
    requires_clone::<BridgeOptions>();
    requires_clone::<LogControl>();
}
