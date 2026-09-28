//! Crate-private OTLP exporter contracts.

pub(crate) mod credits;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sc_observability_types::LogEvent;
use sc_observability_types::v2::{
    Attributes, ExportError, MetricRecord, SpanEnded, SpanEvent, SpanRecord, TraceFlags,
};

/// Resource identity and schema retained separately from signal attributes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Resource {
    pub(crate) attributes: Attributes,
    pub(crate) schema_url: Option<String>,
}

/// Instrumentation scope identity and attributes shared by each exported record.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InstrumentationScope {
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) schema_url: Option<String>,
    pub(crate) attributes: Attributes,
}

/// Neutral signal plus its lossless resource and instrumentation context.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExportRecord<T> {
    pub(crate) resource: Resource,
    pub(crate) scope: InstrumentationScope,
    pub(crate) record: T,
}

/// Existing neutral log payload enriched with v2 flags and typed attributes.
/// `trace_flags` describes `event.trace`; it never invents a trace when absent.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LogRecord {
    pub(crate) event: LogEvent,
    pub(crate) trace_flags: TraceFlags,
    pub(crate) attributes: Attributes,
}

/// Completed v2 span and its ordered events; started spans cannot be exported.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompleteSpan {
    pub(crate) record: SpanRecord<SpanEnded>,
    pub(crate) events: Vec<SpanEvent>,
}

/// Object-safe asynchronous lifecycle result used by both backend adapters.
pub(crate) type LifecycleFuture =
    Pin<Box<dyn Future<Output = Result<(), ExportError>> + Send + 'static>>;

/// Object-safe lifecycle operations shared by exporter backends.
#[expect(
    dead_code,
    reason = "D.21 stages this private contract before D.6 supplies its lifecycle implementation"
)]
pub(crate) trait ExporterLifecycle: Send + Sync {
    /// Performs backend checks that are safe only outside an async lifecycle.
    fn blocking_preflight(&self) -> Result<(), ExportError>;

    /// Flushes all work admitted before the backend's barrier.
    fn flush_async(&self) -> LifecycleFuture;

    /// Shuts the backend down after its ordered barrier.
    fn shutdown_async(&self) -> LifecycleFuture;

    /// Flushes all work admitted before the backend's barrier.
    fn flush_blocking(&self) -> Result<(), ExportError>;

    /// Shuts the backend down after its ordered barrier.
    fn shutdown_blocking(&self) -> Result<(), ExportError>;
}

/// Object-safe exporter for projected log records.
pub(crate) trait LogExporter<T = ExportRecord<LogRecord>>: Send + Sync {
    /// Exports one batch of log events.
    fn export_logs(&self, batch: &[T]) -> Result<(), ExportError>;
}

/// Object-safe exporter for completed spans.
pub(crate) trait TraceExporter<T = ExportRecord<CompleteSpan>>: Send + Sync {
    /// Exports one batch of completed spans.
    fn export_spans(&self, batch: &[T]) -> Result<(), ExportError>;
}

/// Object-safe exporter for projected metrics.
pub(crate) trait MetricExporter<T = ExportRecord<MetricRecord>>: Send + Sync {
    /// Exports one batch of metric records.
    fn export_metrics(&self, batch: &[T]) -> Result<(), ExportError>;
}

/// Backend-neutral set of private exporter capabilities.
pub(crate) struct ExporterSet<
    L = ExportRecord<LogRecord>,
    S = ExportRecord<CompleteSpan>,
    M = ExportRecord<MetricRecord>,
> {
    pub(crate) logs: Arc<dyn LogExporter<L>>,
    pub(crate) traces: Arc<dyn TraceExporter<S>>,
    pub(crate) metrics: Arc<dyn MetricExporter<M>>,
    pub(crate) lifecycle: Arc<dyn ExporterLifecycle>,
}
