//! Compile-only public-signature proof for the opt-in observation facade.

use std::path::PathBuf;

use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
#[allow(deprecated)]
use sc_observability_types::{
    FlushError as LegacyFlushError, InitError as LegacyInitError, ServiceName,
    ShutdownError as LegacyShutdownError, ToolName,
};
use sc_observe::v2::{
    FlushError, InitError, Observability, ObservabilityBuilder, ObservabilityConfig, ShutdownError,
};
use sc_observe::{
    Observability as LegacyObservability, ObservabilityBuilder as LegacyBuilder,
    ObservabilityConfig as LegacyConfig,
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
}

#[allow(deprecated)]
#[test]
fn released_root_observe_exports_keep_their_error_identities() {
    let _: fn(ToolName, PathBuf) -> Result<LegacyConfig, LegacyInitError> =
        LegacyConfig::default_for;
    let _: fn(&LegacyConfig) -> Result<ServiceName, LegacyInitError> = LegacyConfig::service_name;
    let _: fn(LegacyConfig) -> Result<LegacyObservability, LegacyInitError> =
        LegacyObservability::new;
    let _: fn(&LegacyObservability) -> Result<(), LegacyFlushError> = LegacyObservability::flush;
    let _: fn(&LegacyObservability) -> Result<(), LegacyShutdownError> =
        LegacyObservability::shutdown;
    let _: fn(LegacyBuilder) -> Result<LegacyObservability, LegacyInitError> = LegacyBuilder::build;
}

#[test]
fn released_typed_helpers_keep_their_public_signatures() {
    let _: fn(ToolName, PathBuf) -> Result<LegacyConfig, InitFailure> =
        LegacyConfig::default_for_typed;
    let _: fn(&LegacyConfig) -> Result<ServiceName, InitFailure> = LegacyConfig::service_name_typed;
    let _: fn(LegacyConfig) -> Result<LegacyObservability, InitFailure> =
        LegacyObservability::new_typed;
    let _: fn(&LegacyObservability) -> Result<(), FlushFailure> = LegacyObservability::flush_typed;
    let _: fn(&LegacyObservability) -> Result<(), ShutdownFailure> =
        LegacyObservability::shutdown_typed;
    let _: fn(LegacyBuilder) -> Result<LegacyObservability, InitFailure> =
        LegacyBuilder::build_typed;
}

#[allow(deprecated)]
#[test]
fn released_and_canonical_observe_contracts_are_send_sync() {
    requires_send_sync::<LegacyConfig>();
    requires_send_sync::<LegacyBuilder>();
    requires_send_sync::<LegacyObservability>();
    requires_send_sync::<ObservabilityConfig>();
    requires_send_sync::<ObservabilityBuilder>();
    requires_send_sync::<Observability>();

    requires_send_sync::<LegacyInitError>();
    requires_send_sync::<LegacyFlushError>();
    requires_send_sync::<LegacyShutdownError>();
    requires_send_sync::<InitError>();
    requires_send_sync::<FlushError>();
    requires_send_sync::<ShutdownError>();
    requires_send_sync::<InitFailure>();
    requires_send_sync::<FlushFailure>();
    requires_send_sync::<ShutdownFailure>();
}
