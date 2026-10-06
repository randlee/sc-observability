//! Private SDK fixture seam for crate-local tests.

use super::implementation::{SdkAdapterSet, build_exporter_set};
use crate::config::{validated_backend_connection, validated_telemetry_bounds};
use sc_observability_types::otlp::{OtlpCompleteSpan, OtlpLogRecord, OtlpRecord};
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::{ExportError, MetricRecord};

pub(crate) struct SdkFixture {
    adapter: SdkAdapterSet,
}

impl SdkFixture {
    pub(super) fn new(config: &crate::v2::TelemetryConfig) -> Result<Self, InitFailure> {
        let bounds = validated_telemetry_bounds(config)?;
        let connection = validated_backend_connection(&config.transport)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        let adapter = build_exporter_set(&connection, &bounds)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        Ok(Self { adapter })
    }

    pub(super) fn export_logs(
        &self,
        records: &[OtlpRecord<OtlpLogRecord>],
    ) -> Result<(), ExportError> {
        self.adapter.exporters.logs.export_logs(records)
    }

    pub(super) fn export_spans(
        &self,
        records: &[OtlpRecord<OtlpCompleteSpan>],
    ) -> Result<(), ExportError> {
        self.adapter.exporters.traces.export_spans(records)
    }

    pub(super) fn export_metrics(
        &self,
        records: &[OtlpRecord<MetricRecord>],
    ) -> Result<(), ExportError> {
        self.adapter.exporters.metrics.export_metrics(records)
    }

    pub(super) async fn flush(&self) -> Result<(), ExportError> {
        self.adapter.exporters.lifecycle.flush_async().await
    }

    pub(super) async fn shutdown(&self) -> Result<(), ExportError> {
        self.adapter.exporters.lifecycle.shutdown_async().await
    }
}
