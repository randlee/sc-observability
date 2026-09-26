//! Crate-private OTLP exporter contracts.

use std::sync::Arc;

use crate::{CompleteSpan, LogEvent, MetricRecord};
use sc_observability_types::typed::ExportFailure;

/// Object-safe lifecycle operations shared by exporter backends.
#[allow(
    dead_code,
    reason = "D.21 stages this private contract before D.6 supplies its lifecycle implementation"
)]
pub(crate) trait ExporterLifecycle: Send + Sync {
    /// Flushes all work admitted before the backend's barrier.
    fn flush_blocking(&self) -> Result<(), ExportFailure>;

    /// Shuts the backend down after its ordered barrier.
    fn shutdown_blocking(&self) -> Result<(), ExportFailure>;
}

/// Object-safe exporter for projected log records.
pub(crate) trait LogExporter: Send + Sync {
    /// Exports one batch of log events.
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportFailure>;
}

/// Object-safe exporter for completed spans.
pub(crate) trait TraceExporter: Send + Sync {
    /// Exports one batch of completed spans.
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportFailure>;
}

/// Object-safe exporter for projected metrics.
pub(crate) trait MetricExporter: Send + Sync {
    /// Exports one batch of metric records.
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportFailure>;
}

/// Backend-neutral set of private exporter capabilities.
#[allow(
    dead_code,
    reason = "D.21 stages the set before D.7 and D.8 supply backend constructors"
)]
pub(crate) struct ExporterSet {
    pub(crate) logs: Arc<dyn LogExporter>,
    pub(crate) traces: Arc<dyn TraceExporter>,
    pub(crate) metrics: Arc<dyn MetricExporter>,
    pub(crate) lifecycle: Arc<dyn ExporterLifecycle>,
}
