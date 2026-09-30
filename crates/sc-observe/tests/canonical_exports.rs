//! Compile-only public-signature proof for the opt-in observation facade.

use std::path::PathBuf;

use sc_observe::v2::{FlushError, InitError, Observability, ObservabilityBuilder, ObservabilityConfig, ShutdownError};
use sc_observability_types::{ServiceName, ToolName};

#[test]
fn canonical_observe_exports_have_real_public_signatures() {
    let _defaults: fn(ToolName, PathBuf) -> Result<ObservabilityConfig, InitError> = ObservabilityConfig::default_for;
    let _service_name: fn(&ObservabilityConfig) -> Result<ServiceName, InitError> = ObservabilityConfig::service_name;
    let _builder: fn(ObservabilityConfig) -> ObservabilityBuilder = Observability::builder;
    let _build: fn(ObservabilityBuilder) -> Result<Observability, InitError> = ObservabilityBuilder::build;
    let _flush: fn(&Observability) -> Result<(), FlushError> = Observability::flush;
    let _shutdown: fn(&Observability) -> Result<(), ShutdownError> = Observability::shutdown;

    fn requires_send_sync<T: Send + Sync>() {}
    requires_send_sync::<Observability>();
}
