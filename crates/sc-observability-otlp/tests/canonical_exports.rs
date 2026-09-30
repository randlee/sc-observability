//! Compile-only consumer proof for the opt-in OTLP facade.

use sc_observability_otlp::v2::{OtelConfig, Telemetry, TelemetryConfig, TelemetryConfigBuilder};

#[test]
fn canonical_otlp_exports_are_public() {
    let _ = core::any::TypeId::of::<OtelConfig>();
    let _ = core::any::TypeId::of::<Telemetry>();
    let _ = core::any::TypeId::of::<TelemetryConfig>();
    let _ = core::any::TypeId::of::<TelemetryConfigBuilder>();
}
