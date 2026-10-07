//! Compile-only public-signature proof for the opt-in OTLP facade.

use sc_observability_otlp::v2::{OtelConfig, Telemetry, TelemetryConfig, TelemetryConfigBuilder};
use sc_observability_types::ServiceName;
use sc_observability_types::v2::{FlushError, InitError, ShutdownError};
use std::sync::Arc;
use std::time::Duration;

fn requires_send_sync<T: Send + Sync>() {}

struct SignatureObservable;

const _: fn(Arc<Telemetry>) -> sc_observability_otlp::v2::TelemetryProjectors<SignatureObservable> =
    sc_observability_otlp::v2::TelemetryProjectors::new;

#[test]
fn canonical_otlp_exports_have_real_public_signatures() {
    let _: fn(ServiceName) -> TelemetryConfigBuilder = TelemetryConfigBuilder::new;
    let _: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitError> =
        TelemetryConfigBuilder::build_typed;
    let _: fn(TelemetryConfig) -> Result<Telemetry, InitError> = Telemetry::new;
    let _: fn(&Telemetry) -> Result<(), FlushError> = Telemetry::flush;
    let _: fn(&Telemetry, Duration) -> Result<(), FlushError> = Telemetry::flush_with_timeout;
    let _: fn(&Telemetry) -> Result<(), ShutdownError> = Telemetry::shutdown;
    let _: fn(&Telemetry, Duration) -> Result<(), ShutdownError> = Telemetry::shutdown_with_timeout;
    let _: fn(TelemetryConfigBuilder, OtelConfig) -> TelemetryConfigBuilder =
        TelemetryConfigBuilder::with_transport;

    requires_send_sync::<Telemetry>();
}
