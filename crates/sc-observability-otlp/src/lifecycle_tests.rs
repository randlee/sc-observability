//! Lifecycle contract tests use only crate-private exporter seams and a
//! manual poller; no executor or secondary runtime is needed.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use crate::config::{OtelConfig, validated_transport_bounds};
use crate::contracts::{
    CompleteSpan, ExportRecord, ExporterLifecycle, ExporterSet, InstrumentationScope,
    LifecycleFuture, Resource,
};
use crate::lifecycle::{LifecycleCore, LifecycleState, SignalKind};
use crate::testing::{RecordingLogExporter, RecordingMetricExporter, RecordingTraceExporter};
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, Attributes, ExportError, FiniteF64, HistogramPoint,
    MetricRecord, MetricValue, SpanEvent, SpanKind, SpanRecord, SpanStatus, TraceContext,
    TraceFlags,
};
use sc_observability_types::{
    ActionName, DurationMs, MetricName, ServiceName, SpanId, Timestamp, TraceId,
};

struct PendingUntilReleased {
    released: Arc<AtomicBool>,
    entered: Option<Arc<AtomicBool>>,
    block_until_released: bool,
}

impl Future for PendingUntilReleased {
    type Output = Result<(), ExportError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.block_until_released {
            if let Some(entered) = &self.entered {
                entered.store(true, Ordering::Release);
            }
            while !self.released.load(Ordering::Acquire) {
                thread::yield_now();
            }
        }
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
    block_next_flush: AtomicBool,
    flush_entered: Option<Arc<AtomicBool>>,
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
            entered: self.flush_entered.clone(),
            block_until_released: self.block_next_flush.swap(false, Ordering::AcqRel),
        })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.shutdowns.fetch_add(1, Ordering::AcqRel);
        if let Some(error) = self.terminal {
            return Box::pin(async move { Err(error()) });
        }
        Box::pin(PendingUntilReleased {
            released: Arc::clone(&self.released),
            entered: None,
            block_until_released: false,
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
    transport: &OtelConfig,
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
        block_next_flush: AtomicBool::new(false),
        flush_entered: None,
    });
    let exporters = ExporterSet {
        logs: Arc::new(RecordingLogExporter::default()),
        traces: Arc::new(RecordingTraceExporter::default()),
        metrics: Arc::new(RecordingMetricExporter::default()),
        lifecycle,
    };
    let bounds = validated_transport_bounds(transport).expect("test transport bounds");
    let core = LifecycleCore::new(exporters, &bounds).expect("test lifecycle core");
    (core, flushes, shutdowns, released)
}

fn default_fixture() -> (
    LifecycleCore,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicBool>,
) {
    fixture(None, &OtelConfig::default())
}

fn preserved_records() -> (ExportRecord<CompleteSpan>, ExportRecord<MetricRecord>) {
    let resource = Resource {
        attributes: Attributes::from([("host.id".into(), AttributeValue::UInt(u64::MAX))]),
        schema_url: Some("https://example.test/resource".into()),
    };
    let scope = InstrumentationScope {
        name: "lifecycle-consumer".into(),
        version: Some("2.0".into()),
        schema_url: Some("https://example.test/scope".into()),
        attributes: Attributes::from([("scope.enabled".into(), AttributeValue::Bool(true))]),
    };
    let metric = ExportRecord {
        resource: resource.clone(),
        scope: scope.clone(),
        record: MetricRecord::try_new(
            Timestamp::UNIX_EPOCH,
            ServiceName::new("test").expect("valid service"),
            MetricName::new("latency").expect("valid metric"),
            MetricValue::Histogram {
                point: HistogramPoint::try_new(
                    vec![FiniteF64::new(1.0).unwrap()],
                    vec![1, 2],
                    3,
                    FiniteF64::new(5.0).expect("finite histogram sum"),
                )
                .expect("valid histogram"),
                temporality: AggregationTemporality::Cumulative,
                start_time: Timestamp::UNIX_EPOCH,
            },
        )
        .expect("valid metric"),
    };
    let trace = TraceContext::new(
        TraceId::new("1234567890abcdef1234567890abcdef").expect("valid trace"),
        SpanId::new("1234567890abcdef").expect("valid span"),
        TraceFlags::new(0x81),
    );
    let link = sc_observability_types::v2::SpanLink::new(
        TraceId::new("abcdefabcdefabcdefabcdefabcdefab").expect("valid linked trace"),
        SpanId::new("abcdefabcdefabcd").expect("valid linked span"),
        TraceFlags::new(0x41),
        Attributes::from([("link.kind".into(), AttributeValue::String("parent".into()))]),
    );
    let span = ExportRecord {
        resource,
        scope,
        record: CompleteSpan {
            record: SpanRecord::new(
                Timestamp::UNIX_EPOCH,
                ServiceName::new("test").expect("valid service"),
                ActionName::new("request").expect("valid action"),
                trace.clone(),
                Attributes::from([("span.kind".into(), AttributeValue::String("server".into()))]),
            )
            .with_kind(SpanKind::Server)
            .with_links(vec![link])
            .end(SpanStatus::Ok, DurationMs::from(7)),
            events: vec![SpanEvent {
                timestamp: Timestamp::UNIX_EPOCH,
                trace,
                name: ActionName::new("event").expect("valid event"),
                attributes: Attributes::from([(
                    "event.kind".into(),
                    AttributeValue::String("checkpoint".into()),
                )]),
                diagnostic: None,
            }],
        },
    };
    (span, metric)
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

struct CountingWake {
    wakes: AtomicUsize,
}

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::AcqRel);
    }
}

fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    Pin::new(future).poll(&mut context)
}

fn poll_with_waker<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
    let mut context = Context::from_waker(waker);
    Pin::new(future).poll(&mut context)
}

#[test]
fn concurrent_waiters_do_not_start_duplicate_backend_operation() {
    let flushes = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(AtomicBool::new(false));
    let second_polled = Arc::new(AtomicBool::new(false));
    let lifecycle = Arc::new(RecordingLifecycle {
        flushes: Arc::clone(&flushes),
        shutdowns: Arc::new(AtomicUsize::new(0)),
        released: Arc::clone(&released),
        terminal: None,
        block_next_flush: AtomicBool::new(true),
        flush_entered: Some(Arc::clone(&entered)),
    });
    let exporters = ExporterSet {
        logs: Arc::new(RecordingLogExporter::default()),
        traces: Arc::new(RecordingTraceExporter::default()),
        metrics: Arc::new(RecordingMetricExporter::default()),
        lifecycle,
    };
    let bounds = validated_transport_bounds(&OtelConfig::default()).expect("test bounds");
    let core = LifecycleCore::new(exporters, &bounds).expect("test lifecycle core");
    core.admit(SignalKind::Logs, (), 1)
        .unwrap()
        .complete(Err(runtime_terminated()));
    let mut first = core.flush_async();
    let mut second = core.flush_async();

    let first_thread = thread::spawn(move || poll_once(&mut first));
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !entered.load(Ordering::Acquire) {
        assert!(
            std::time::Instant::now() < deadline,
            "first backend poll did not start"
        );
        thread::yield_now();
    }
    let second_polled_for_thread = Arc::clone(&second_polled);
    let second_thread = thread::spawn(move || {
        let result = poll_once(&mut second);
        second_polled_for_thread.store(true, Ordering::Release);
        (second, result)
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !second_polled.load(Ordering::Acquire) {
        assert!(
            std::time::Instant::now() < deadline,
            "second waiter did not poll"
        );
        thread::yield_now();
    }
    let flushes_before_release = flushes.load(Ordering::Acquire);
    released.store(true, Ordering::Release);
    assert!(matches!(
        first_thread.join().expect("first waiter thread"),
        Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
    ));
    let (mut second, second_result) = second_thread.join().expect("second waiter thread");
    assert!(second_result.is_pending());
    assert_eq!(flushes_before_release, 1);
    assert!(matches!(
        poll_once(&mut second),
        Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
    ));
}

#[test]
fn ordered_flush_waits_for_prior_admission_and_preserves_payload() {
    let (core, flushes, _, released) = default_fixture();
    let (span_payload, metric_payload) = preserved_records();
    let admitted_span = core
        .admit(SignalKind::Traces, span_payload.clone(), 256)
        .expect("admit span");
    let admitted_metric = core
        .admit(SignalKind::Metrics, metric_payload.clone(), 128)
        .expect("admit metric");
    assert_eq!(admitted_span.get(), &span_payload);
    assert_eq!(admitted_metric.get(), &metric_payload);
    assert_eq!(admitted_span.get().resource, span_payload.resource);
    assert_eq!(admitted_span.get().scope, span_payload.scope);
    assert_eq!(admitted_span.get().record.record.trace().flags.bits(), 0x81);
    assert_eq!(admitted_span.get().record.record.links().len(), 1);
    assert_eq!(
        admitted_span.get().record.events[0].trace.flags.bits(),
        0x81
    );
    let MetricValue::Histogram { point, .. } = admitted_metric.get().record.value() else {
        panic!("metric payload lost its histogram variant")
    };
    assert_eq!(point.explicit_bounds(), &[FiniteF64::new(1.0).unwrap()]);
    assert_eq!(point.bucket_counts(), &[1, 2]);
    assert_eq!(point.count(), 3);
    assert!((point.sum().get() - 5.0).abs() < f64::EPSILON);
    let mut flush = core.flush_async();
    assert!(poll_once(&mut flush).is_pending());
    assert_eq!(flushes.load(Ordering::Acquire), 0);
    assert_eq!(admitted_span.complete(Ok(())), span_payload);
    assert_eq!(admitted_metric.complete(Ok(())), metric_payload);
    released.store(true, Ordering::Release);
    assert!(poll_once(&mut flush).is_ready());
    assert_eq!(flushes.load(Ordering::Acquire), 1);
}

#[test]
fn ordered_flush_returns_admitted_export_failure_and_keeps_success_control() {
    let (core, _, _, released) = default_fixture();
    let admitted = core
        .admit(SignalKind::Logs, (), 1)
        .expect("admit log before transport failure");
    admitted.complete(Err(runtime_terminated()));
    released.store(true, Ordering::Release);

    let mut flush = core.flush_async();
    let Poll::Ready(result) = poll_once(&mut flush) else {
        panic!("flush must complete after the admitted export finishes")
    };
    assert_eq!(
        result
            .expect_err("admitted transport failure must reach flush")
            .diagnostic()
            .code,
        crate::error_codes::OTLP_RUNTIME_TERMINATED
    );
    assert!(core.health().degraded);

    let (control, _, _, released) = default_fixture();
    let admitted = control
        .admit(SignalKind::Logs, (), 1)
        .expect("admit success-control log");
    admitted.complete(Ok(()));
    released.store(true, Ordering::Release);
    let mut flush = control.flush_async();
    assert!(matches!(poll_once(&mut flush), Poll::Ready(Ok(()))));
}

#[test]
fn consumed_failure_does_not_poison_later_successful_windows() {
    let (core, flushes, shutdowns, released) = default_fixture();
    core.admit(SignalKind::Logs, (), 1)
        .unwrap()
        .complete(Err(runtime_terminated()));
    let mut first = core.flush_async();
    let mut shared = core.flush_async();
    assert!(poll_once(&mut first).is_pending());
    released.store(true, Ordering::Release);
    for waiter in [&mut first, &mut shared] {
        assert!(matches!(
            poll_once(waiter),
            Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
        ));
    }
    core.admit(SignalKind::Logs, (), 1)
        .unwrap()
        .complete(Ok(()));
    assert!(matches!(
        poll_once(&mut core.flush_async()),
        Poll::Ready(Ok(()))
    ));
    assert!(matches!(
        poll_once(&mut core.shutdown_async()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(flushes.load(Ordering::Acquire), 2);
    assert_eq!(shutdowns.load(Ordering::Acquire), 1);
    assert!(
        core.health().degraded,
        "health retains historical degradation"
    );
    assert_eq!(core.health().dropped_by_signal, [1, 0, 0]);
}

#[test]
fn failures_on_both_sides_of_cutoff_survive_either_completion_order() {
    for later_completes_first in [false, true] {
        let (core, _, _, released) = default_fixture();
        let earlier = core.admit(SignalKind::Logs, (), 1).unwrap();
        let mut first = core.flush_async();
        let later = core.admit(SignalKind::Traces, (), 1).unwrap();
        if later_completes_first {
            later.complete(Err(admission_timeout()));
            earlier.complete(Err(runtime_terminated()));
        } else {
            earlier.complete(Err(runtime_terminated()));
            later.complete(Err(admission_timeout()));
        }
        released.store(true, Ordering::Release);
        assert!(matches!(
            poll_once(&mut first),
            Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
        ));
        assert!(matches!(
            poll_once(&mut core.flush_async()),
            Poll::Ready(Err(ExportError::LifecycleTimeout { .. }))
        ));
        assert!(matches!(
            poll_once(&mut core.flush_async()),
            Poll::Ready(Ok(()))
        ));
        assert!(matches!(
            poll_once(&mut core.shutdown_async()),
            Poll::Ready(Ok(()))
        ));
    }
}

#[test]
fn pending_backend_retains_window_failure_after_waiter_cancellation() {
    let (core, flushes, _, released) = default_fixture();
    let earlier = core.admit(SignalKind::Logs, (), 1).unwrap();
    let mut first = core.flush_async();
    earlier.complete(Err(runtime_terminated()));
    assert!(
        poll_once(&mut first).is_pending(),
        "backend is still pending"
    );
    core.admit(SignalKind::Traces, (), 1)
        .unwrap()
        .complete(Err(admission_timeout()));
    drop(first);
    let mut replacement = core.flush_async();
    released.store(true, Ordering::Release);
    assert!(matches!(
        poll_once(&mut replacement),
        Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
    ));
    assert_eq!(flushes.load(Ordering::Acquire), 1);
    assert!(matches!(
        poll_once(&mut core.flush_async()),
        Poll::Ready(Err(ExportError::LifecycleTimeout { .. }))
    ));
}

#[test]
fn overlapping_shutdown_preserves_flush_failure_for_all_waiters() {
    // Exercise failure captured before shutdown as well as completion while
    // both operations are active. Shutdown must inherit its flush precondition.
    for complete_before_shutdown in [false, true] {
        let (core, flushes, shutdowns, released) = default_fixture();
        let admission = core.admit(SignalKind::Logs, (), 1).unwrap();
        let mut flush = core.flush_async();
        let admission = if complete_before_shutdown {
            admission.complete(Err(runtime_terminated()));
            None
        } else {
            Some(admission)
        };
        let mut shutdown = core.shutdown_async();
        let mut shared_shutdown = core.shutdown_async();
        if let Some(admission) = admission {
            admission.complete(Err(runtime_terminated()));
        }
        assert!(poll_once(&mut shutdown).is_pending());
        released.store(true, Ordering::Release);
        for waiter in [&mut shutdown, &mut shared_shutdown, &mut flush] {
            assert!(matches!(
                poll_once(waiter),
                Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
            ));
        }
        assert!(matches!(
            poll_once(&mut core.shutdown_async()),
            Poll::Ready(Ok(()))
        ));
        assert_eq!(flushes.load(Ordering::Acquire), 1);
        assert_eq!(shutdowns.load(Ordering::Acquire), 1);
    }
}

#[test]
fn empty_flush_excludes_first_later_admission() {
    let (core, flushes, _, released) = default_fixture();
    let mut empty = core.flush_async();
    let later = core.admit(SignalKind::Logs, (), 1).unwrap();
    released.store(true, Ordering::Release);
    assert!(matches!(poll_once(&mut empty), Poll::Ready(Ok(()))));
    assert_eq!(flushes.load(Ordering::Acquire), 1);
    later.complete(Err(runtime_terminated()));
    assert!(matches!(
        poll_once(&mut core.flush_async()),
        Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
    ));
}

#[test]
fn admission_failure_after_timed_out_window_reaches_next_barrier() {
    let transport = OtelConfig {
        timeout_ms: Some(1.into()),
        lifecycle_flush_timeout_ms: Some(2.into()),
        ..OtelConfig::default()
    };
    let (core, _, _, released) = fixture(None, &transport);
    let admission = core.admit(SignalKind::Logs, (), 1).unwrap();
    let mut expired = core.flush_async();
    thread::sleep(Duration::from_millis(5));
    assert!(matches!(
        poll_once(&mut expired),
        Poll::Ready(Err(ExportError::LifecycleTimeout { .. }))
    ));
    admission.complete(Err(runtime_terminated()));
    released.store(true, Ordering::Release);
    assert!(matches!(
        poll_once(&mut core.flush_async()),
        Poll::Ready(Err(ExportError::RuntimeTerminated { .. }))
    ));
    assert!(matches!(
        poll_once(&mut core.flush_async()),
        Poll::Ready(Ok(()))
    ));
}

fn admission_timeout() -> ExportError {
    ExportError::LifecycleTimeout {
        context: Box::new(sc_observability_types::ErrorContext::new(
            crate::error_codes::OTLP_LIFECYCLE_TIMEOUT,
            "test admission timed out",
            sc_observability_types::Remediation::recoverable(
                "retry the export",
                std::iter::empty::<String>(),
            ),
        )),
    }
}

#[test]
fn ordered_barrier_wakes_after_its_last_admission_finishes() {
    let (core, _, _, released) = default_fixture();
    let admitted = core
        .admit(SignalKind::Logs, (), 1)
        .expect("admit log record");
    let (completion_attempt_tx, completion_attempt_rx) = mpsc::channel();
    let completion_started = Arc::new(Barrier::new(2));
    let completion_started_for_thread = Arc::clone(&completion_started);
    let (completion_done_tx, completion_done_rx) = mpsc::channel();
    let completion_thread = thread::spawn(move || {
        completion_attempt_rx
            .recv()
            .expect("registration hook signals completion attempt");
        completion_started_for_thread.wait();
        admitted.complete(Ok(()));
        completion_done_tx
            .send(())
            .expect("test observes completed admission");
    });
    core.set_barrier_registration_hook(move || {
        completion_attempt_tx
            .send(())
            .expect("start completion while the admission mutex is held");
        completion_started.wait();
    });
    let mut flush = core.flush_async();
    let wake_counter = Arc::new(CountingWake {
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake_counter));

    assert!(poll_with_waker(&mut flush, &waker).is_pending());

    // `poll_with_waker` releases the admission mutex before it returns, so
    // completion may legitimately wake immediately. The hook/barrier above
    // already proves completion attempted while that mutex was held; assert
    // the one required wake after completion instead of observing a racy
    // pre-completion count here.
    completion_done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("completion proceeds after waker registration");
    completion_thread.join().expect("completion thread");
    assert_eq!(wake_counter.wakes.load(Ordering::Acquire), 1);

    released.store(true, Ordering::Release);
    assert!(poll_with_waker(&mut flush, &waker).is_ready());
}

#[test]
fn admission_is_fail_open_and_drop_accounting_is_exact_once() {
    let transport = OtelConfig {
        queue_capacity: Some(1),
        queue_byte_capacity: Some(4),
        ..OtelConfig::default()
    };
    let (core, _, _, released) = fixture(None, &transport);
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
        sc_observability_types::v2::TelemetryError::Shutdown { .. }
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
fn failed_shutdown_is_idempotent_after_terminal_completion() {
    let (core, _, shutdowns, _) = fixture(Some(runtime_terminated), &OtelConfig::default());
    let mut first = core.shutdown_async();
    let Poll::Ready(result) = poll_once(&mut first) else {
        panic!("shutdown failure remained pending")
    };
    assert_eq!(
        result.expect_err("shutdown must fail").diagnostic().code,
        crate::error_codes::OTLP_RUNTIME_TERMINATED
    );

    let mut second = core.shutdown_async();
    assert!(matches!(poll_once(&mut second), Poll::Ready(Ok(()))));
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
        timeout_ms: Some(1.into()),
        lifecycle_flush_timeout_ms: Some(2.into()),
        lifecycle_shutdown_timeout_ms: Some(2.into()),
        ..OtelConfig::default()
    };
    let (core, _, _, _) = fixture(None, &transport);
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

    let (core, _, _, _) = fixture(Some(runtime_terminated), &OtelConfig::default());
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
