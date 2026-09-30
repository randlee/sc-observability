//! Compile-only public-signature proof for the opt-in observation facade.

use std::{path::PathBuf, sync::Arc};

#[allow(deprecated)]
use sc_observability_types::{
    FlushError as LegacyFlushError, InitError as LegacyInitError, ServiceName,
    ShutdownError as LegacyShutdownError, ToolName,
};
use sc_observability_types::{
    ObservabilityHealthProvider, ObservabilityHealthReport, Observation, ObservationError,
    ProjectionRegistration, SubscriberRegistration,
    typed::{FlushFailure, InitFailure, ShutdownFailure},
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

#[test]
fn released_root_observe_exports_keep_their_error_identities() {
    // Each method is bound on its own, with no type annotation, so the
    // expectation can only be fulfilled by that method's own deprecation.
    // The signature is then pinned separately; that statement names the
    // deprecated released error types and allows only that use.
    #[expect(
        deprecated,
        reason = "the released default_for deprecation must remain externally observable"
    )]
    let default_for = LegacyConfig::default_for;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(ToolName, PathBuf) -> Result<LegacyConfig, LegacyInitError> = default_for;
    #[expect(
        deprecated,
        reason = "the released service_name deprecation must remain externally observable"
    )]
    let service_name = LegacyConfig::service_name;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(&LegacyConfig) -> Result<ServiceName, LegacyInitError> = service_name;
    #[expect(
        deprecated,
        reason = "the released Observability::new deprecation must remain externally observable"
    )]
    let new = LegacyObservability::new;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(LegacyConfig) -> Result<LegacyObservability, LegacyInitError> = new;
    #[expect(
        deprecated,
        reason = "the released Observability::flush deprecation must remain externally observable"
    )]
    let flush = LegacyObservability::flush;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(&LegacyObservability) -> Result<(), LegacyFlushError> = flush;
    #[expect(
        deprecated,
        reason = "the released Observability::shutdown deprecation must remain externally observable"
    )]
    let shutdown = LegacyObservability::shutdown;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(&LegacyObservability) -> Result<(), LegacyShutdownError> = shutdown;
    #[expect(
        deprecated,
        reason = "the released ObservabilityBuilder::build deprecation must remain externally observable"
    )]
    let build = LegacyBuilder::build;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released error type"
    )]
    let _: fn(LegacyBuilder) -> Result<LegacyObservability, LegacyInitError> = build;
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
fn released_and_canonical_observe_routing_exports_keep_their_public_signatures() {
    let _: fn(LegacyConfig) -> LegacyBuilder = LegacyObservability::builder;
    let _: fn(&LegacyObservability, Observation<String>) -> Result<(), ObservationError> =
        LegacyObservability::emit::<String>;
    let _: fn(&LegacyObservability) -> ObservabilityHealthReport = LegacyObservability::health;
    let _: fn(LegacyBuilder, Arc<dyn ObservabilityHealthProvider>) -> LegacyBuilder =
        LegacyBuilder::with_observability_health_provider;
    let _: fn(LegacyBuilder, SubscriberRegistration<String>) -> LegacyBuilder =
        LegacyBuilder::register_subscriber::<String>;
    let _: fn(LegacyBuilder, ProjectionRegistration<String>) -> LegacyBuilder =
        LegacyBuilder::register_projection::<String>;

    let _: fn(ObservabilityConfig) -> ObservabilityBuilder = Observability::builder;
    let _: fn(&Observability, Observation<String>) -> Result<(), ObservationError> =
        Observability::emit::<String>;
    let _: fn(&Observability) -> ObservabilityHealthReport = Observability::health;
    let _: fn(ObservabilityBuilder, Arc<dyn ObservabilityHealthProvider>) -> ObservabilityBuilder =
        ObservabilityBuilder::with_observability_health_provider;
    let _: fn(ObservabilityBuilder, SubscriberRegistration<String>) -> ObservabilityBuilder =
        ObservabilityBuilder::register_subscriber::<String>;
    let _: fn(ObservabilityBuilder, ProjectionRegistration<String>) -> ObservabilityBuilder =
        ObservabilityBuilder::register_projection::<String>;
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
