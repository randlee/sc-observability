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
                    vec![1.0],
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

fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
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
    assert!(first_thread.join().expect("first waiter thread").is_ready());
    let (mut second, second_result) = second_thread.join().expect("second waiter thread");
    assert!(second_result.is_pending());
    assert_eq!(flushes_before_release, 1);
    assert!(poll_once(&mut second).is_ready());
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
    assert_eq!(point.explicit_bounds(), &[1.0]);
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
        timeout_ms: 1.into(),
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
