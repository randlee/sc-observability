//! Compile-only consumer proof for the opt-in observation facade.

use sc_observe::v2::{Observability, ObservabilityBuilder, ObservabilityConfig};

#[test]
fn canonical_observe_exports_are_public() {
    let _ = core::any::TypeId::of::<Observability>();
    let _ = core::any::TypeId::of::<ObservabilityBuilder>();
    let _ = core::any::TypeId::of::<ObservabilityConfig>();
}
