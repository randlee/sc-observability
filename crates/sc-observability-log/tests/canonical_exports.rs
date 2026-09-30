//! Compile-only consumer proof for the opt-in bridge facade.

use sc_observability_log::v2::{BridgeEvent, BridgeOptions, LogControl, LogGuard};

#[test]
fn canonical_log_exports_are_public() {
    let _ = core::any::TypeId::of::<BridgeEvent>();
    let _ = core::any::TypeId::of::<BridgeOptions>();
    let _ = core::any::TypeId::of::<LogControl>();
    let _ = core::any::TypeId::of::<LogGuard>();
}
