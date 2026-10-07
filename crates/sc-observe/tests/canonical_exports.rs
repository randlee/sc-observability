#![cfg(feature = "v1")]
#![allow(
    deprecated,
    reason = "this compile-only contract fixture intentionally names the released v1 facade"
)]

//! Compile-only public-signature proof for the opt-in observation facade.

use std::{path::PathBuf, sync::Arc};

use sc_observability_types::v2::{
    FlushError as LegacyFlushError, ProjectionRegistration as CanonicalProjectionRegistration,
    ShutdownError as LegacyShutdownError,
    SubscriberRegistration as CanonicalSubscriberRegistration,
};
#[allow(deprecated)]
use sc_observability_types::{InitError as LegacyInitError, ServiceName, ToolName};
#[allow(deprecated)]
use sc_observability_types::{
    ObservabilityHealthProvider, ObservabilityHealthReport, Observation, ObservationError,
    ProjectionRegistration as LegacyProjectionRegistration,
    SubscriberRegistration as LegacySubscriberRegistration,
    typed::{FlushFailure, InitFailure, ShutdownFailure},
};
use sc_observe::v2::{
    FlushError, InitError, Observability, ObservabilityBuilder, ObservabilityConfig, ShutdownError,
};
#[allow(deprecated)]
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
#[allow(deprecated)]
fn released_typed_helpers_keep_their_public_signatures() {
    #[expect(
        deprecated,
        reason = "the released default_for_typed deprecation must remain externally observable"
    )]
    let default_for_typed = LegacyConfig::default_for_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(ToolName, PathBuf) -> Result<LegacyConfig, InitFailure> = default_for_typed;
    #[expect(
        deprecated,
        reason = "the released service_name_typed deprecation must remain externally observable"
    )]
    let service_name_typed = LegacyConfig::service_name_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(&LegacyConfig) -> Result<ServiceName, InitFailure> = service_name_typed;
    #[expect(
        deprecated,
        reason = "the released new_typed deprecation must remain externally observable"
    )]
    let new_typed = LegacyObservability::new_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(LegacyConfig) -> Result<LegacyObservability, InitFailure> = new_typed;
    #[expect(
        deprecated,
        reason = "the released flush_typed deprecation must remain externally observable"
    )]
    let flush_typed = LegacyObservability::flush_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(&LegacyObservability) -> Result<(), FlushFailure> = flush_typed;
    #[expect(
        deprecated,
        reason = "the released shutdown_typed deprecation must remain externally observable"
    )]
    let shutdown_typed = LegacyObservability::shutdown_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(&LegacyObservability) -> Result<(), ShutdownFailure> = shutdown_typed;
    #[expect(
        deprecated,
        reason = "the released build_typed deprecation must remain externally observable"
    )]
    let build_typed = LegacyBuilder::build_typed;
    #[allow(deprecated, reason = "the pin names the deprecated released helper")]
    let _: fn(LegacyBuilder) -> Result<LegacyObservability, InitFailure> = build_typed;
}

#[test]
fn released_and_canonical_observe_routing_exports_keep_their_public_signatures() {
    #[allow(
        deprecated,
        reason = "the pin names the released root configuration type"
    )]
    let _: fn(LegacyConfig) -> LegacyBuilder = LegacyObservability::builder;
    let _: fn(&LegacyObservability, Observation<String>) -> Result<(), ObservationError> =
        LegacyObservability::emit::<String>;
    let _: fn(&LegacyObservability) -> ObservabilityHealthReport = LegacyObservability::health;
    let _: fn(LegacyBuilder, Arc<dyn ObservabilityHealthProvider>) -> LegacyBuilder =
        LegacyBuilder::with_observability_health_provider;
    #[expect(
        deprecated,
        reason = "the released register_subscriber deprecation must remain externally observable"
    )]
    let register_subscriber = LegacyBuilder::register_subscriber::<String>;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released registration type"
    )]
    let _: fn(LegacyBuilder, LegacySubscriberRegistration<String>) -> LegacyBuilder =
        register_subscriber;
    #[expect(
        deprecated,
        reason = "the released register_projection deprecation must remain externally observable"
    )]
    let register_projection = LegacyBuilder::register_projection::<String>;
    #[allow(
        deprecated,
        reason = "the pin names the deprecated released registration type"
    )]
    let _: fn(LegacyBuilder, LegacyProjectionRegistration<String>) -> LegacyBuilder =
        register_projection;

    let _: fn(&Observability, Observation<String>) -> Result<(), ObservationError> =
        Observability::emit::<String>;
    let _: fn(&Observability) -> ObservabilityHealthReport = Observability::health;
    let _: fn(ObservabilityBuilder, Arc<dyn ObservabilityHealthProvider>) -> ObservabilityBuilder =
        ObservabilityBuilder::with_observability_health_provider;
    let _: fn(
        ObservabilityBuilder,
        CanonicalSubscriberRegistration<String>,
    ) -> ObservabilityBuilder = ObservabilityBuilder::register_subscriber::<String>;
    let _: fn(
        ObservabilityBuilder,
        CanonicalProjectionRegistration<String>,
    ) -> ObservabilityBuilder = ObservabilityBuilder::register_projection::<String>;
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
