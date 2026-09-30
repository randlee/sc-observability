//! Compile-only consumer proof for the opt-in core facade.

use sc_observability::v2::{Logger, LoggerBuilder, LoggerConfig};

#[test]
fn canonical_core_exports_are_public() {
    let _ = core::any::TypeId::of::<Logger>();
    let _ = core::any::TypeId::of::<LoggerBuilder>();
    let _ = core::any::TypeId::of::<LoggerConfig>();
}
