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
mod compat;
mod config;
#[cfg(test)]
mod contract_tests;
mod contracts;
mod exporter_factory;
#[cfg(test)]
#[allow(
    deprecated,
    reason = "telemetry compatibility tests exercise retained lifecycle and error wrappers"
)]
mod facade_tests;
mod failure;
mod legacy_projection;
mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
mod projectors;
mod runtime;
mod testing;

#[cfg(feature = "otlp-sdk")]
mod sdk;
#[cfg(feature = "sync-http")]
mod sync_http;

pub mod constants;
pub mod error_codes;

use sc_observability_types::telemetry_health_provider_sealed;
// `facade_tests` reaches the runtime's collaborators through `super::*`.
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryError, TelemetryHealthReport,
    TelemetryHealthState,
};
#[cfg(test)]
use {
    crate::assembly::V2SpanAssembler,
    crate::config::{TelemetryConfig as RuntimeTelemetryConfig, validated_transport_bounds},
    crate::contracts::{ExportRecord, ExporterSet, LogExporter, LogRecord},
    crate::exporter_factory::exporter_factory,
    crate::failure::{export_failure_from_canonical_event, shutdown_flush_failure},
    crate::legacy_projection::trace_context,
    sc_observability_types::typed::FlushFailure,
    sc_observability_types::v2::TelemetryError as CanonicalTelemetryError,
    sc_observability_types::v2::{
        ConfigFailure, EventError as CanonicalEventError, ExportError,
        MetricRecord as CanonicalMetricRecord,
    },
    sc_observability_types::{ErrorContext, MetricRecord, Remediation, SpanSignal},
    serde_json::Value,
    std::sync::Arc,
    std::sync::atomic::Ordering,
};

#[doc(inline)]
pub use assembly::{CompleteSpan, SpanAssembler, SpanAssemblyLoss};
#[doc(inline)]
pub use compat::{
    AuthHeader, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry, TelemetryConfig,
    TelemetryConfigBuilder, TelemetryProjectors,
};
#[doc(inline)]
pub use config::{
    ExporterBackend, LogsConfig, MetricsConfig, ResourceAttributes, SyncHttpRetryPolicy,
    TracesConfig,
};
#[doc(inline)]
pub use projectors::V2TelemetryProjectors;
#[doc(inline)]
pub use runtime::RuntimeTelemetry;

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
#[cfg(feature = "sdk-test-support")]
#[doc(inline)]
pub use sdk::SdkFixture;
