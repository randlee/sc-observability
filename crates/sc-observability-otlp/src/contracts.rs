//! Crate-private OTLP exporter contracts.

pub(crate) mod credits;
#[cfg(any(feature = "durable-store", all(test, feature = "sync-http")))]
pub(crate) mod profiles;
#[cfg(any(feature = "durable-store", all(test, feature = "sync-http")))]
pub(crate) mod submission;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::lifecycle::LifecycleHealth;
#[cfg_attr(
    not(any(feature = "sync-http", feature = "otlp-sdk")),
    allow(
        unused_imports,
        reason = "D.21 contracts are consumed by enabled backends"
    )
)]
pub(crate) use sc_observability_types::otlp::{
    OtlpCompleteSpan as CompleteSpan, OtlpInstrumentationScope as InstrumentationScope,
    OtlpLogRecord as LogRecord, OtlpRecord as ExportRecord, OtlpResource as Resource,
};
use sc_observability_types::v2::{ExportError, MetricRecord};

/// Object-safe asynchronous lifecycle result used by both backend adapters.
pub(crate) type LifecycleFuture =
    Pin<Box<dyn Future<Output = Result<(), ExportError>> + Send + 'static>>;

/// Object-safe lifecycle operations shared by exporter backends.
pub(crate) trait ExporterLifecycle: Send + Sync {
    /// Returns whether this lifecycle has begun its terminal shutdown.
    ///
    /// A facade must use this shared state rather than maintain a second
    /// shutdown flag that can diverge from the backend's admission barrier.
    fn is_shutdown(&self) -> bool;

    /// Returns shared lifecycle accounting when this backend owns an admission core.
    fn lifecycle_health(&self) -> Option<LifecycleHealth> {
        None
    }

    /// Performs backend checks that are safe only outside an async lifecycle.
    fn blocking_preflight(&self) -> Result<(), ExportError>;

    /// Verifies that a synchronous lifecycle operation is supported.
    ///
    /// This is distinct from construction preflight: an SDK backend may be
    /// constructed on its caller runtime while requiring lifecycle completion
    /// through its asynchronous methods.
    fn blocking_lifecycle_preflight(&self) -> Result<(), ExportError> {
        self.blocking_preflight()
    }

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
