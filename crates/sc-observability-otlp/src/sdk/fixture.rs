//! Unstable external fixture access to the D.7 SDK adapter.
//!
//! This module exists solely for the repository-owned `examples/otlp-sdk`
//! consumer fixture. It is non-default, exposes no crate-private contracts,
//! and does not activate the production [`crate::Telemetry`] factory; D.18
//! retains that composition boundary.

use crate::config::{validated_backend_connection, validated_telemetry_bounds};
use crate::sdk::implementation::{SdkAdapterSet, build_exporter_set};
use crate::v2::TelemetryConfig;
use sc_observability_types::otlp::{OtlpCompleteSpan, OtlpLogRecord, OtlpMetricRecord, OtlpRecord};
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::ExportError;

/// Unstable fixture-only handle for a real caller-runtime SDK adapter.
///
/// Construct this inside the embedding application's Tokio runtime. It is
/// intentionally unavailable without the non-default `sdk-test-support`
/// feature and must not be used as a production telemetry facade.
#[expect(
    missing_debug_implementations,
    reason = "the opaque fixture intentionally retains private SDK adapter state"
)]
pub struct SdkFixture {
    adapter: SdkAdapterSet,
}

impl SdkFixture {
    /// Constructs the real SDK adapter from an already public configuration.
    ///
    /// # Errors
    ///
    /// Returns the existing typed configuration or adapter-construction
    /// failure. The fixture never creates a runtime or reads ambient OTEL
    /// settings.
    pub fn new(config: &TelemetryConfig) -> Result<Self, InitFailure> {
        let bounds = validated_telemetry_bounds(config)?;
        let connection = validated_backend_connection(&config.transport)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        let adapter = build_exporter_set(&connection, &bounds)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        Ok(Self { adapter })
    }

    /// Schedules real OTLP log projection and export through the D.6 core.
    pub fn export_logs(&self, records: &[OtlpRecord<OtlpLogRecord>]) -> Result<(), ExportError> {
        self.adapter.exporters.logs.export_logs(records)
    }

    /// Schedules real OTLP span projection and export through the D.6 core.
    pub fn export_spans(
        &self,
        records: &[OtlpRecord<OtlpCompleteSpan>],
    ) -> Result<(), ExportError> {
        self.adapter.exporters.traces.export_spans(records)
    }

    /// Schedules real OTLP metric projection and export through the D.6 core.
    pub fn export_metrics(&self, records: &[OtlpMetricRecord]) -> Result<(), ExportError> {
        self.adapter.exporters.metrics.export_metrics(records)
    }

    /// Waits for every export admitted before this call to reach a terminal outcome.
    pub async fn flush(&self) -> Result<(), ExportError> {
        self.adapter.exporters.lifecycle.flush_async().await
    }

    /// Shuts down this fixture's adapter after its ordered lifecycle barrier.
    pub async fn shutdown(&self) -> Result<(), ExportError> {
        self.adapter.exporters.lifecycle.shutdown_async().await
    }
}
