//! Compile-only public-signature proof for the opt-in OTLP facade.

use sc_observability_otlp::v2::{InitError, OtelConfig, Telemetry, TelemetryConfig, TelemetryConfigBuilder};
use sc_observability_types::ServiceName;
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};

#[test]
#[allow(
    deprecated,
    reason = "the fixture verifies the retained compatible public signature alongside the canonical typed signature"
)]
fn canonical_otlp_exports_have_real_public_signatures() {
    let _config: fn(ServiceName) -> TelemetryConfigBuilder = TelemetryConfigBuilder::new;
    let _legacy_build: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitError> = TelemetryConfigBuilder::build;
    let _typed_build: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitFailure> = TelemetryConfigBuilder::build_typed;
    let _new: fn(TelemetryConfig) -> Result<Telemetry, InitFailure> = Telemetry::new;
    let _flush: fn(&Telemetry) -> Result<(), FlushFailure> = Telemetry::flush;
    let _shutdown: fn(&Telemetry) -> Result<(), ShutdownFailure> = Telemetry::shutdown;
    let _transport: fn(TelemetryConfigBuilder, OtelConfig) -> TelemetryConfigBuilder = TelemetryConfigBuilder::with_transport;

    fn requires_send_sync<T: Send + Sync>() {}
    requires_send_sync::<Telemetry>();
}
