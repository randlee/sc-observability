//! Lifecycle contract tests use only crate-private exporter seams and a
//! manual poller; no executor or secondary runtime is needed.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use crate::config::{OtelConfig, validated_transport_bounds};
use crate::contracts::{
    ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, MetricExporter, TraceExporter,
};
use crate::lifecycle::{LifecycleCore, LifecycleState, SignalKind};
use crate::{CompleteSpan, LogEvent, MetricRecord};
use sc_observability_types::v2::ExportError;

struct NoopLogs;
struct NoopTraces;
struct NoopMetrics;

impl LogExporter for NoopLogs {
    fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl TraceExporter for NoopTraces {
    fn export_spans(&self, _batch: &[CompleteSpan]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl MetricExporter for NoopMetrics {
    fn export_metrics(&self, _batch: &[MetricRecord]) -> Result<(), ExportError> {
        Ok(())
    }
}

struct PendingUntilReleased {
    released: Arc<AtomicBool>,
}

impl Future for PendingUntilReleased {
    type Output = Result<(), ExportError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.released.load(Ordering::Acquire) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

struct RecordingLifecycle {
    flushes: Arc<AtomicUsize>,
    shutdowns: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    terminal: Option<fn() -> ExportError>,
}

impl ExporterLifecycle for RecordingLifecycle {
    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        self.flushes.fetch_add(1, Ordering::AcqRel);
        if let Some(error) = self.terminal {
            return Box::pin(async move { Err(error()) });
        }
        Box::pin(PendingUntilReleased {
            released: Arc::clone(&self.released),
        })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.shutdowns.fetch_add(1, Ordering::AcqRel);
        if let Some(error) = self.terminal {
            return Box::pin(async move { Err(error()) });
        }
        Box::pin(PendingUntilReleased {
            released: Arc::clone(&self.released),
        })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }
}

fn fixture(
    terminal: Option<fn() -> ExportError>,
    transport: OtelConfig,
) -> (
    LifecycleCore,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicBool>,
) {
    let flushes = Arc::new(AtomicUsize::new(0));
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicBool::new(false));
    let lifecycle = Arc::new(RecordingLifecycle {
        flushes: Arc::clone(&flushes),
        shutdowns: Arc::clone(&shutdowns),
        released: Arc::clone(&released),
        terminal,
    });
    let exporters = ExporterSet {
        logs: Arc::new(NoopLogs),
        traces: Arc::new(NoopTraces),
        metrics: Arc::new(NoopMetrics),
        lifecycle,
    };
    let bounds = validated_transport_bounds(&transport).expect("test transport bounds");
    let core = LifecycleCore::new(exporters, &bounds).expect("test lifecycle core");
    (core, flushes, shutdowns, released)
}

fn default_fixture() -> (
    LifecycleCore,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicBool>,
) {
    fixture(None, OtelConfig::default())
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    Pin::new(future).poll(&mut context)
}

#[test]
fn ordered_flush_waits_for_prior_admission_and_preserves_payload() {
    let (core, flushes, _, released) = default_fixture();
    let admitted = core
        .admit(SignalKind::Metrics, "histogram-payload", 17)
        .expect("admit");
    assert_eq!(admitted.get(), &"histogram-payload");
    let mut flush = core.flush_async();
    assert!(poll_once(&mut flush).is_pending());
    assert_eq!(flushes.load(Ordering::Acquire), 0);
    let payload = admitted.complete(Ok(()));
    assert_eq!(payload, "histogram-payload");
    released.store(true, Ordering::Release);
    assert!(poll_once(&mut flush).is_ready());
    assert_eq!(flushes.load(Ordering::Acquire), 1);
}

#[test]
fn admission_is_fail_open_and_drop_accounting_is_exact_once() {
    let transport = OtelConfig {
        queue_capacity: Some(1),
        queue_byte_capacity: Some(4),
        ..OtelConfig::default()
    };
    let (core, _, _, released) = fixture(None, transport);
    let admitted = core
        .admit(SignalKind::Logs, (), 4)
        .expect("first admission");
    let Err(rejected) = core.admit(SignalKind::Logs, (), 1) else {
        panic!("queue must be full")
    };
    assert_eq!(rejected.code(), crate::error_codes::OTLP_QUEUE_FULL);
    assert_eq!(core.health().dropped_by_signal, [1, 0, 0]);
    drop(admitted);
    assert_eq!(core.health().dropped_by_signal, [2, 0, 0]);
    released.store(true, Ordering::Release);
    let mut shutdown = core.shutdown_async();
    assert!(poll_once(&mut shutdown).is_ready());
    let Err(closed) = core.admit(SignalKind::Logs, (), 1) else {
        panic!("closed admission")
    };
    assert!(matches!(
        closed,
        sc_observability_types::v2::TelemetryError::Shutdown
    ));
    assert_eq!(core.health().phase, LifecycleState::Shutdown);
    assert_eq!(core.health().dropped_by_signal, [3, 0, 0]);
}

#[test]
fn repeated_shutdown_uses_one_backend_operation() {
    let (core, _, shutdowns, released) = default_fixture();
    let mut first = core.shutdown_async();
    assert!(poll_once(&mut first).is_pending());
    assert_eq!(shutdowns.load(Ordering::Acquire), 1);
    released.store(true, Ordering::Release);
    assert!(poll_once(&mut first).is_ready());
    let mut second = core.shutdown_async();
    assert!(poll_once(&mut second).is_ready());
    assert_eq!(shutdowns.load(Ordering::Acquire), 1);
}

#[test]
fn cancelled_waiter_can_be_replaced_without_duplicate_shutdown() {
    let (core, _, shutdowns, released) = default_fixture();
    let mut first = core.shutdown_async();
    assert!(poll_once(&mut first).is_pending());
    drop(first);
    let mut replacement = core.shutdown_async();
    released.store(true, Ordering::Release);
    assert!(poll_once(&mut replacement).is_ready());
    assert_eq!(shutdowns.load(Ordering::Acquire), 1);
}

#[test]
fn lifecycle_deadline_and_runtime_termination_are_typed() {
    let transport = OtelConfig {
        timeout_ms: 1.into(),
        lifecycle_flush_timeout_ms: Some(2.into()),
        lifecycle_shutdown_timeout_ms: Some(2.into()),
        ..OtelConfig::default()
    };
    let (core, _, _, _) = fixture(None, transport);
    let mut flush = core.flush_async();
    assert!(poll_once(&mut flush).is_pending());
    thread::sleep(Duration::from_millis(5));
    let Poll::Ready(result) = poll_once(&mut flush) else {
        panic!("deadline result remained pending")
    };
    assert_eq!(
        result.expect_err("must timeout").diagnostic().code,
        crate::error_codes::OTLP_LIFECYCLE_TIMEOUT
    );
    assert!(core.health().degraded);

    let (core, _, _, _) = fixture(Some(runtime_terminated), OtelConfig::default());
    let mut flush = core.flush_async();
    let Poll::Ready(result) = poll_once(&mut flush) else {
        panic!("runtime result remained pending")
    };
    let result = result.expect_err("runtime failed");
    assert_eq!(
        result.diagnostic().code,
        crate::error_codes::OTLP_RUNTIME_TERMINATED
    );
}

fn runtime_terminated() -> ExportError {
    ExportError::RuntimeTerminated {
        context: Box::new(sc_observability_types::ErrorContext::new(
            crate::error_codes::OTLP_RUNTIME_TERMINATED,
            "test runtime terminated",
            sc_observability_types::Remediation::recoverable(
                "keep the runtime alive",
                std::iter::empty::<String>(),
            ),
        )),
    }
}
