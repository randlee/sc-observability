//! Shared recording fixtures for crate-private OTLP contract and lifecycle tests.

#![cfg(test)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sc_observability_types::v2::ExportError;
use sc_observability_types::{ErrorContext, Remediation};

use crate::contracts::{
    ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, MetricExporter, TraceExporter,
};
use crate::error_codes;

/// Lifecycle operations recorded by [`RecordingLifecycle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleCall {
    BlockingPreflight,
    FlushAsync,
    ShutdownAsync,
    FlushBlocking,
    FlushBlockingWithTimeout(Duration),
    ShutdownBlocking,
    ShutdownBlockingWithTimeout(Duration),
}

/// A successful lifecycle double that records every operation in order.
#[derive(Default)]
pub(crate) struct RecordingLifecycle {
    pub(crate) calls: Mutex<Vec<LifecycleCall>>,
}

impl RecordingLifecycle {
    fn record(&self, call: LifecycleCall) {
        self.calls.lock().expect("calls poisoned").push(call);
    }
}

impl ExporterLifecycle for RecordingLifecycle {
    fn is_shutdown(&self) -> bool {
        let calls = self.calls.lock().expect("calls poisoned");
        calls.contains(&LifecycleCall::ShutdownBlocking)
            || calls
                .iter()
                .any(|call| matches!(call, LifecycleCall::ShutdownBlockingWithTimeout(_)))
            || calls.contains(&LifecycleCall::ShutdownAsync)
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        self.record(LifecycleCall::BlockingPreflight);
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        self.record(LifecycleCall::FlushAsync);
        Box::pin(async { Ok(()) })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.record(LifecycleCall::ShutdownAsync);
        Box::pin(async { Ok(()) })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        self.record(LifecycleCall::FlushBlocking);
        Ok(())
    }

    fn flush_blocking_with_timeout(&self, timeout: Duration) -> Result<(), ExportError> {
        self.record(LifecycleCall::FlushBlockingWithTimeout(timeout));
        Ok(())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.record(LifecycleCall::ShutdownBlocking);
        Ok(())
    }

    fn shutdown_blocking_with_timeout(&self, timeout: Duration) -> Result<(), ExportError> {
        self.record(LifecycleCall::ShutdownBlockingWithTimeout(timeout));
        Ok(())
    }
}

/// A log exporter double that records complete batches and can be made to fail.
pub(crate) struct RecordingLogExporter<T> {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) batches: Mutex<Vec<Vec<T>>>,
    pub(crate) fail: AtomicBool,
}

impl<T> Default for RecordingLogExporter<T> {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
            fail: AtomicBool::new(false),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> LogExporter<T> for RecordingLogExporter<T> {
    fn export_logs(&self, batch: &[T]) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(batch.len());
        self.batches
            .lock()
            .expect("batches poisoned")
            .push(batch.to_vec());
        export_result(&self.fail, "log")
    }
}

/// A trace exporter double that records complete batches and can be made to fail.
pub(crate) struct RecordingTraceExporter<T> {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) batches: Mutex<Vec<Vec<T>>>,
    pub(crate) fail: AtomicBool,
}

impl<T> Default for RecordingTraceExporter<T> {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
            fail: AtomicBool::new(false),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> TraceExporter<T> for RecordingTraceExporter<T> {
    fn export_spans(&self, batch: &[T]) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(batch.len());
        self.batches
            .lock()
            .expect("batches poisoned")
            .push(batch.to_vec());
        export_result(&self.fail, "trace")
    }
}

/// A metric exporter double that records complete batches and can be made to fail.
pub(crate) struct RecordingMetricExporter<T> {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) batches: Mutex<Vec<Vec<T>>>,
    pub(crate) fail: AtomicBool,
}

impl<T> Default for RecordingMetricExporter<T> {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
            fail: AtomicBool::new(false),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> MetricExporter<T> for RecordingMetricExporter<T> {
    fn export_metrics(&self, batch: &[T]) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(batch.len());
        self.batches
            .lock()
            .expect("batches poisoned")
            .push(batch.to_vec());
        export_result(&self.fail, "metric")
    }
}

fn export_result(fail: &AtomicBool, signal: &str) -> Result<(), ExportError> {
    if fail.load(Ordering::SeqCst) {
        Err(ExportError::Transport {
            context: Box::new(ErrorContext::new(
                error_codes::OTLP_EXPORT_TERMINAL,
                format!("{signal} export failed"),
                Remediation::not_recoverable("test exporter failure"),
            )),
        })
    } else {
        Ok(())
    }
}

/// The injected exporter set and handles used to inspect every recorded call.
pub(crate) struct RecordingExporterSet<L, S, M> {
    pub(crate) exporters: ExporterSet<L, S, M>,
    pub(crate) logs: Arc<RecordingLogExporter<L>>,
    pub(crate) traces: Arc<RecordingTraceExporter<S>>,
    pub(crate) metrics: Arc<RecordingMetricExporter<M>>,
    pub(crate) lifecycle: Arc<RecordingLifecycle>,
}

/// Builds one shared recording fixture for the three signal families.
pub(crate) fn recording_exporter_set<L, S, M>() -> RecordingExporterSet<L, S, M>
where
    L: Clone + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
    M: Clone + Send + Sync + 'static,
{
    let logs = Arc::new(RecordingLogExporter::default());
    let traces = Arc::new(RecordingTraceExporter::default());
    let metrics = Arc::new(RecordingMetricExporter::default());
    let lifecycle = Arc::new(RecordingLifecycle::default());
    RecordingExporterSet {
        exporters: ExporterSet {
            logs: logs.clone(),
            traces: traces.clone(),
            metrics: metrics.clone(),
            lifecycle: lifecycle.clone(),
        },
        logs,
        traces,
        metrics,
        lifecycle,
    }
}
