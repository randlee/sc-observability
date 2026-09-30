//! Compile-only public-signature proof for the opt-in observation facade.

use std::path::PathBuf;

use sc_observability_types::{ServiceName, ToolName};
use sc_observe::v2::{
    FlushError, InitError, Observability, ObservabilityBuilder, ObservabilityConfig, ShutdownError,
};

fn requires_send_sync<T: Send + Sync>() {}

#[test]
fn canonical_observe_exports_have_real_public_signatures() {
    let _: fn(ToolName, PathBuf) -> Result<ObservabilityConfig, InitError> =
        ObservabilityConfig::default_for;
    let _: fn(&ObservabilityConfig) -> Result<ServiceName, InitError> =
        ObservabilityConfig::service_name;
    let _: fn(ObservabilityConfig) -> ObservabilityBuilder = Observability::builder;
    let _: fn(ObservabilityBuilder) -> Result<Observability, InitError> =
        ObservabilityBuilder::build;
    let _: fn(&Observability) -> Result<(), FlushError> = Observability::flush;
    let _: fn(&Observability) -> Result<(), ShutdownError> = Observability::shutdown;

    requires_send_sync::<Observability>();
}
