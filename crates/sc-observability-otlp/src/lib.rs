//! OTLP-backed telemetry with test-only integration to `sc-observability` and
//! `sc-observe`.
//!
//! This crate owns telemetry configuration, span assembly, exporter contracts,
//! and the lifecycle/runtime behavior for OTLP-bound signals. It attaches to
//! routing through ordinary projector registration and keeps OpenTelemetry
//! transport concerns out of the lower crates. Facade tests use the core
//! logging and observation crates; production OTLP code does not use them.
#![expect(
    clippy::missing_errors_doc,
    reason = "telemetry-facade error behavior is documented centrally in workspace docs, and repeating it on every wrapper method adds low-signal boilerplate"
)]

#[cfg(feature = "durable-store")]
pub mod durable;

mod assembly;
mod config;
#[cfg(test)]
mod contract_tests;
mod contracts;
mod export_records;
mod exporter_factory;
#[cfg(test)]
mod facade_tests;
mod failure;
mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
mod projectors;
mod runtime;
#[cfg(any(feature = "otlp-sdk", feature = "sync-http", feature = "log-sink"))]
mod severity;
#[cfg(all(test, feature = "otlp-sdk", feature = "sync-http"))]
mod severity_tests;
mod testing;

#[cfg(feature = "v1")]
pub mod v1;

#[cfg(feature = "otlp-sdk")]
#[path = "sdk/mod.rs"]
mod sdk_backend;
#[cfg(feature = "sync-http")]
mod sync_http;

pub mod constants;
pub mod error_codes;

#[cfg(feature = "log-sink")]
mod log_sink;
#[cfg(feature = "log-sink")]
mod native;
#[cfg(feature = "synchronous-client")]
pub mod sync;

#[cfg(feature = "log-sink")]
#[doc(inline)]
pub use log_sink::OtelLogSink;
#[cfg(feature = "log-sink")]
#[doc(inline)]
pub use native::{api, sdk};

#[doc(inline)]
pub use assembly::SpanAssemblyLoss;
#[doc(inline)]
pub use config::{
    ExporterBackend, LogsConfig, MetricsConfig, ResourceAttributes, SyncHttpRetryPolicy,
    TracesConfig,
};
#[doc(inline)]
pub use projectors::V2TelemetryProjectors;
#[doc(inline)]
pub use runtime::RuntimeTelemetry;
#[cfg(feature = "v1")]
use sc_observability_types::telemetry_health_provider_sealed;
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryHealthReport, TelemetryHealthState,
};
#[cfg(feature = "v1")]
#[allow(
    deprecated,
    reason = "the root keeps the released v1 paths while v1 items direct consumers to v2"
)]
#[doc(inline)]
pub use v1::{
    AuthHeader, CompleteSpan, OtelConfig, OtlpEndpoint, OtlpProtocol, SpanAssembler, Telemetry,
    TelemetryConfig, TelemetryConfigBuilder, TelemetryError, TelemetryProjectors,
};

/// Opt-in canonical OTLP facade for the compatible 1.x transition.
///
/// This namespace re-exports the existing telemetry implementation and does
/// not introduce a second backend, configuration authority, or lifecycle.
pub mod v2 {
    #[doc(inline)]
    pub use crate::RuntimeTelemetry as Telemetry;
    #[doc(inline)]
    pub use crate::V2TelemetryProjectors as TelemetryProjectors;
    #[doc(inline)]
    pub use crate::config::{
        AuthHeader, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
        ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
    };
    #[doc(inline)]
    pub use crate::{ExporterBackend, SyncHttpRetryPolicy};
    #[doc(inline)]
    pub use sc_observability_types::v2::{
        ConfigFailure, EventError, FlushError, InitError, ShutdownError, TelemetryError,
    };
}
