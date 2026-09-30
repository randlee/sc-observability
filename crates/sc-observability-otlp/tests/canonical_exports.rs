//! Compile-only public-signature proof for the opt-in OTLP facade.

use sc_observability_otlp::v2::{
    InitError, OtelConfig, Telemetry, TelemetryConfig, TelemetryConfigBuilder,
};
use sc_observability_types::Observable;
use sc_observability_types::ServiceName;
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use std::sync::Arc;

fn requires_send_sync<T: Send + Sync>() {}

#[allow(dead_code)]
fn canonical_projector_signature<T: Observable>(
    telemetry: Arc<Telemetry>,
) -> sc_observability_otlp::v2::TelemetryProjectors<T> {
    sc_observability_otlp::v2::TelemetryProjectors::new(telemetry)
}

#[test]
#[allow(
    deprecated,
    reason = "the fixture verifies the retained compatible public signature alongside the canonical typed signature"
)]
fn canonical_otlp_exports_have_real_public_signatures() {
    let _: fn(ServiceName) -> TelemetryConfigBuilder = TelemetryConfigBuilder::new;
    let _: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitError> =
        TelemetryConfigBuilder::build;
    let _: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitFailure> =
        TelemetryConfigBuilder::build_typed;
    let _: fn(TelemetryConfig) -> Result<Telemetry, InitFailure> = Telemetry::new;
    let _: fn(&Telemetry) -> Result<(), FlushFailure> = Telemetry::flush;
    let _: fn(&Telemetry) -> Result<(), ShutdownFailure> = Telemetry::shutdown;
    let _: fn(TelemetryConfigBuilder, OtelConfig) -> TelemetryConfigBuilder =
        TelemetryConfigBuilder::with_transport;

    requires_send_sync::<Telemetry>();
}
