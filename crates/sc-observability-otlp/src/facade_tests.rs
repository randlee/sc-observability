//! Canonical facade ingress coverage kept independent of the retired 1.x facade.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use crate::assembly::V2SpanAssembler;
use crate::config::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, TelemetryConfig, TelemetryConfigBuilder,
    TracesConfig,
};
use crate::constants::{MAX_OTLP_EVENTS_PER_SPAN, MAX_OTLP_LIVE_SPANS};
use crate::contracts::{
    self, ExportRecord, ExporterLifecycle, LifecycleFuture, LogExporter, LogRecord, MetricExporter,
    TraceExporter,
};
use crate::testing::{
    LifecycleCall, RecordingLifecycle, RecordingLogExporter, RecordingMetricExporter,
    RecordingTraceExporter,
};
use crate::{ExporterHealthState, RuntimeTelemetry, error_codes};
use sc_observability_types::v2::{self, ExportError, MetricRecord as CanonicalMetricRecord};
use sc_observability_types::{
    ActionName, DurationMs, ErrorContext, Level, LogEvent, MetricName, ProcessIdentity,
    Remediation, SchemaVersion, ServiceName, SpanId, StateName, StateTransition, TargetCategory,
    TelemetryHealthState, Timestamp, TraceId,
};

type Logs = RecordingLogExporter<ExportRecord<LogRecord>>;
type Traces = RecordingTraceExporter<ExportRecord<contracts::CompleteSpan>>;
type Metrics = RecordingMetricExporter<ExportRecord<CanonicalMetricRecord>>;
type ShutdownAwareFixture = (
    RuntimeTelemetry,
    Receiver<()>,
    Sender<()>,
    Arc<Mutex<Vec<&'static str>>>,
);

const RACE_EMITTERS: usize = 4;
const PREFILLED_RECORD_ID: usize = 1_000_000;

struct ShutdownAwareExport {
    entered: Mutex<Option<Sender<()>>>,
    release: Mutex<Receiver<()>>,
    backend_shutdown: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl ShutdownAwareExport {
    fn export(&self) -> Result<(), ExportError> {
        self.entered
            .lock()
            .expect("entered poisoned")
            .take()
            .expect("export only runs once")
            .send(())
            .expect("test waits for export entry");
        self.release
            .lock()
            .expect("release poisoned")
            .recv()
            .expect("test releases export");
        if self.backend_shutdown.load(Ordering::SeqCst) {
            return Err(ExportError::TerminalExportFailure {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_TELEMETRY_SHUTDOWN,
                    "test exporter rejected an export after backend shutdown",
                    Remediation::not_recoverable("export before shutting down the backend"),
                )),
            });
        }
        self.events.lock().expect("events poisoned").push("export");
        Ok(())
    }
}

struct ShutdownAwareLogExporter(Arc<ShutdownAwareExport>);

impl LogExporter for ShutdownAwareLogExporter {
    fn export_logs(&self, _batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        self.0.export()
    }
}

struct ShutdownAwareTraceExporter(Arc<ShutdownAwareExport>);

impl TraceExporter for ShutdownAwareTraceExporter {
    fn export_spans(
        &self,
        _batch: &[ExportRecord<contracts::CompleteSpan>],
    ) -> Result<(), ExportError> {
        self.0.export()
    }
}

struct ShutdownAwareMetricExporter(Arc<ShutdownAwareExport>);

impl MetricExporter for ShutdownAwareMetricExporter {
    fn export_metrics(
        &self,
        _batch: &[ExportRecord<CanonicalMetricRecord>],
    ) -> Result<(), ExportError> {
        self.0.export()
    }
}

struct ShutdownAwareLifecycle {
    backend_shutdown: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl ShutdownAwareLifecycle {
    fn shutdown(&self) {
        self.backend_shutdown.store(true, Ordering::SeqCst);
        self.events
            .lock()
            .expect("events poisoned")
            .push("shutdown");
    }
}

impl ExporterLifecycle for ShutdownAwareLifecycle {
    fn is_shutdown(&self) -> bool {
        self.backend_shutdown.load(Ordering::SeqCst)
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.shutdown();
        Box::pin(async { Ok(()) })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.shutdown();
        Ok(())
    }

    fn shutdown_blocking_with_timeout(&self, _timeout: Duration) -> Result<(), ExportError> {
        self.shutdown();
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ShutdownAwareSignal {
    Logs,
    Traces,
    Metrics,
}

fn shutdown_aware_fixture(signal: ShutdownAwareSignal) -> ShutdownAwareFixture {
    let mut config = telemetry_config();
    config.logs = Some(LogsConfig { batch_size: 1 });
    config.traces = Some(TracesConfig { batch_size: 1 });
    config.metrics = Some(MetricsConfig {
        batch_size: 1,
        export_interval_ms: 60_000_u64.into(),
    });
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let backend_shutdown = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::new()));
    let blocking = Arc::new(ShutdownAwareExport {
        entered: Mutex::new(Some(entered_tx)),
        release: Mutex::new(release_rx),
        backend_shutdown: Arc::clone(&backend_shutdown),
        events: Arc::clone(&events),
    });
    let lifecycle = Arc::new(ShutdownAwareLifecycle {
        backend_shutdown,
        events: Arc::clone(&events),
    });
    let logs: Arc<dyn LogExporter> = match signal {
        ShutdownAwareSignal::Logs => Arc::new(ShutdownAwareLogExporter(Arc::clone(&blocking))),
        _ => Arc::new(RecordingLogExporter::default()),
    };
    let traces: Arc<dyn TraceExporter> = match signal {
        ShutdownAwareSignal::Traces => Arc::new(ShutdownAwareTraceExporter(Arc::clone(&blocking))),
        _ => Arc::new(RecordingTraceExporter::default()),
    };
    let metrics: Arc<dyn MetricExporter> = match signal {
        ShutdownAwareSignal::Metrics => {
            Arc::new(ShutdownAwareMetricExporter(Arc::clone(&blocking)))
        }
        _ => Arc::new(RecordingMetricExporter::default()),
    };
    let telemetry = RuntimeTelemetry::new_with_exporters_and_lifecycle_typed(
        config, logs, traces, metrics, lifecycle,
    )
    .expect("valid shutdown-aware telemetry");
    (telemetry, entered_rx, release_tx, events)
}

fn wait_until_stopping(telemetry: &RuntimeTelemetry) {
    while !telemetry
        .runtime
        .lock()
        .expect("telemetry runtime poisoned")
        .stopping
    {
        thread::yield_now();
    }
}

fn service_name() -> ServiceName {
    ServiceName::new("test-service").expect("valid service")
}

fn telemetry_config() -> TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new_typed("https://otel.example.internal")
                    .expect("valid OTLP endpoint"),
            ),
            ..OtelConfig::default()
        })
        .build_typed()
        .expect("valid telemetry config")
}

fn recording() -> (RuntimeTelemetry, Arc<Traces>, Arc<Metrics>) {
    let (telemetry, _, traces, metrics) = recording_with_config(telemetry_config());
    (telemetry, traces, metrics)
}

fn recording_with_config(
    config: TelemetryConfig,
) -> (RuntimeTelemetry, Arc<Logs>, Arc<Traces>, Arc<Metrics>) {
    let logs = Arc::new(Logs::default());
    let traces = Arc::new(Traces::default());
    let metrics = Arc::new(Metrics::default());
    let telemetry = RuntimeTelemetry::new_with_exporters_typed(
        config,
        logs.clone(),
        traces.clone(),
        metrics.clone(),
    )
    .expect("recording telemetry");
    (telemetry, logs, traces, metrics)
}

fn recording_with_lifecycle(lifecycle: Arc<RecordingLifecycle>) -> RuntimeTelemetry {
    RuntimeTelemetry::new_with_exporters_and_lifecycle_typed(
        telemetry_config(),
        Arc::new(RecordingLogExporter::<ExportRecord<LogRecord>>::default()),
        Arc::new(Traces::default()),
        Arc::new(Metrics::default()),
        lifecycle,
    )
    .expect("recording telemetry")
}

fn recording_with_logs() -> (RuntimeTelemetry, Arc<Logs>, Arc<Traces>, Arc<Metrics>) {
    recording_with_config(telemetry_config())
}

fn log_event_message(message: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("schema"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("otlp.runtime").expect("target"),
        action: ActionName::new("emit").expect("action"),
        message: Some(message.to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn log_recording() -> (
    RuntimeTelemetry,
    Arc<RecordingLogExporter<ExportRecord<LogRecord>>>,
) {
    let logs = Arc::new(RecordingLogExporter::<ExportRecord<LogRecord>>::default());
    let telemetry = telemetry_with_config(telemetry_config(), &logs);
    (telemetry, logs)
}

fn telemetry_with_config(
    config: TelemetryConfig,
    logs: &Arc<RecordingLogExporter<ExportRecord<LogRecord>>>,
) -> RuntimeTelemetry {
    RuntimeTelemetry::new_with_exporters_typed(
        config,
        logs.clone(),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("recording telemetry")
}

fn log_event_with_entity_id(entity_id: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("otlp.facade").expect("valid target"),
        action: ActionName::new("emit").expect("valid action"),
        message: Some("entity admission".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: Some(StateTransition {
            entity_kind: TargetCategory::new("agent").expect("valid entity kind"),
            entity_id: Some(entity_id.to_owned()),
            from_state: StateName::new("idle").expect("valid state"),
            to_state: StateName::new("active").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: serde_json::Map::new(),
    }
}

fn exported_logs(exporter: &RecordingLogExporter<ExportRecord<LogRecord>>) -> usize {
    exporter
        .batches
        .lock()
        .expect("batches poisoned")
        .iter()
        .map(Vec::len)
        .sum()
}

fn trace(span_id: &str) -> v2::TraceContext {
    v2::TraceContext::new(
        TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        SpanId::new(span_id).expect("valid span id"),
        v2::TraceFlags::new(0x01),
    )
}

fn started(trace: v2::TraceContext) -> v2::SpanRecord<v2::SpanStarted> {
    v2::SpanRecord::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace,
        v2::Attributes::new(),
    )
}

fn event(trace: v2::TraceContext, name: &str) -> v2::SpanSignal {
    v2::SpanSignal::Event(v2::SpanEvent {
        timestamp: Timestamp::UNIX_EPOCH,
        trace,
        name: ActionName::new(name).expect("valid event"),
        attributes: v2::Attributes::new(),
        diagnostic: None,
    })
}

fn exported_spans(traces: &Traces) -> Vec<ExportRecord<contracts::CompleteSpan>> {
    traces.batches.lock().expect("batches poisoned").concat()
}

fn log_event(id: usize) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("runtime.shutdown").expect("valid target"),
        action: ActionName::new("admission.race").expect("valid action"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::from_iter([("id".to_owned(), serde_json::Value::from(id))]),
    }
}

fn metric(id: usize) -> CanonicalMetricRecord {
    CanonicalMetricRecord::try_new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        MetricName::new(format!("runtime.shutdown.{id}")).expect("valid metric name"),
        v2::MetricValue::Gauge(v2::FiniteF64::new(1.0).expect("finite gauge")),
    )
    .expect("valid metric")
}

fn span_signals(id: usize) -> (v2::SpanSignal, v2::SpanSignal) {
    let trace = v2::TraceContext::new(
        TraceId::new(format!("{id:032x}")).expect("valid trace id"),
        SpanId::new(format!("{id:016x}")).expect("valid span id"),
        v2::TraceFlags::new(0x01),
    );
    let started = started(trace);
    let ended = started.clone().end(v2::SpanStatus::Ok, DurationMs::from(1));
    (
        v2::SpanSignal::Started(started),
        v2::SpanSignal::Ended(ended),
    )
}

fn race_until_shutdown(
    telemetry: &Arc<RuntimeTelemetry>,
    emit: impl Fn(&RuntimeTelemetry, usize) -> Result<(), v2::TelemetryError> + Send + Sync + 'static,
) -> Vec<usize> {
    let barrier = Arc::new(Barrier::new(RACE_EMITTERS + 1));
    let accepted = Arc::new(Mutex::new(Vec::new()));
    let emit = Arc::new(emit);
    let mut emitters = Vec::with_capacity(RACE_EMITTERS);

    for worker in 0..RACE_EMITTERS {
        let barrier = Arc::clone(&barrier);
        let accepted = Arc::clone(&accepted);
        let emit = Arc::clone(&emit);
        let telemetry = Arc::clone(telemetry);
        emitters.push(thread::spawn(move || {
            barrier.wait();
            for id in (worker + 1..).step_by(RACE_EMITTERS) {
                match emit(&telemetry, id) {
                    Ok(()) => accepted.lock().expect("accepted ids poisoned").push(id),
                    Err(v2::TelemetryError::Shutdown { .. }) => break,
                    Err(error) => panic!("unexpected admission error: {error:?}"),
                }
            }
        }));
    }

    barrier.wait();
    telemetry.shutdown().expect("shutdown completes");
    for emitter in emitters {
        emitter.join().expect("emitter finishes");
    }
    Arc::try_unwrap(accepted)
        .expect("emitters released accepted ids")
        .into_inner()
        .expect("accepted ids poisoned")
}

fn expected_ids(accepted: Vec<usize>) -> HashSet<usize> {
    accepted
        .into_iter()
        .chain(std::iter::once(PREFILLED_RECORD_ID))
        .collect()
}

#[test]
fn shutdown_drains_every_admitted_log_in_a_concurrent_race() {
    let (telemetry, logs, _, _) = recording_with_logs();
    let telemetry = Arc::new(telemetry);
    telemetry
        .emit_log(&log_event(PREFILLED_RECORD_ID))
        .expect("prefilled log is admitted");

    let expected = expected_ids(race_until_shutdown(&telemetry, |telemetry, id| {
        telemetry.emit_log(&log_event(id))
    }));
    let exported: HashSet<_> = logs
        .batches
        .lock()
        .expect("log batches poisoned")
        .iter()
        .flatten()
        .map(|record| {
            usize::try_from(
                record.record.event.fields["id"]
                    .as_u64()
                    .expect("recorded id is an unsigned integer"),
            )
            .expect("recorded id fits usize")
        })
        .collect();

    assert_eq!(exported, expected);
}

#[test]
fn shutdown_drains_every_completed_admitted_span_in_a_concurrent_race() {
    let (telemetry, _, traces, _) = recording_with_logs();
    let telemetry = Arc::new(telemetry);
    let (started, ended) = span_signals(PREFILLED_RECORD_ID);
    telemetry
        .emit_span(&started)
        .expect("prefilled start is admitted");
    telemetry
        .emit_span(&ended)
        .expect("prefilled end is admitted");

    let expected = expected_ids(race_until_shutdown(&telemetry, |telemetry, id| {
        let (started, ended) = span_signals(id);
        telemetry.emit_span(&started)?;
        telemetry.emit_span(&ended)
    }));
    let exported: HashSet<_> = exported_spans(&traces)
        .into_iter()
        .map(|record| {
            usize::from_str_radix(record.record.record.trace().span_id.as_str(), 16)
                .expect("recorded span id is hexadecimal")
        })
        .collect();

    assert_eq!(exported, expected);
}

#[test]
fn shutdown_drains_every_admitted_metric_in_a_concurrent_race() {
    let (telemetry, _, _, metrics) = recording_with_logs();
    let telemetry = Arc::new(telemetry);
    telemetry
        .emit_metric(&metric(PREFILLED_RECORD_ID))
        .expect("prefilled metric is admitted");

    let expected = expected_ids(race_until_shutdown(&telemetry, |telemetry, id| {
        telemetry.emit_metric(&metric(id))
    }));
    let exported: HashSet<_> = metrics
        .batches
        .lock()
        .expect("metric batches poisoned")
        .iter()
        .flatten()
        .map(|record| {
            record
                .record
                .name()
                .as_str()
                .rsplit_once('.')
                .expect("metric name includes record id")
                .1
                .parse::<usize>()
                .expect("metric record id is numeric")
        })
        .collect();

    assert_eq!(exported, expected);
}

fn histogram_metric() -> CanonicalMetricRecord {
    let one_second: Timestamp =
        serde_json::from_str("\"1970-01-01T00:00:01Z\"").expect("valid timestamp");
    v2::MetricRecord::try_new(
        one_second,
        service_name(),
        MetricName::new("tool.duration").expect("valid metric"),
        v2::MetricValue::Histogram {
            point: v2::HistogramPoint::try_new(
                vec![
                    v2::FiniteF64::new(1.0).expect("finite"),
                    v2::FiniteF64::new(10.0).expect("finite"),
                ],
                vec![1, 2, 3],
                6,
                v2::FiniteF64::new(80.5).expect("finite"),
            )
            .expect("valid histogram"),
            temporality: v2::AggregationTemporality::Delta,
            start_time: Timestamp::UNIX_EPOCH,
        },
    )
    .expect("valid histogram metric")
}

#[test]
fn canonical_assembler_uses_the_production_bounds() {
    let production = (MAX_OTLP_LIVE_SPANS, MAX_OTLP_EVENTS_PER_SPAN);
    assert_eq!(V2SpanAssembler::new().limits(), production);
    let (telemetry, _, _) = recording();
    let runtime = telemetry.runtime.lock().expect("runtime");
    assert_eq!(runtime.span_assembler.limits(), production);
    assert_eq!(V2SpanAssembler::with_limits(0, 0).limits(), (1, 1));
}

#[test]
fn canonical_span_ingress_evicts_oldest_and_reports_loss_health() {
    let (telemetry, traces, _) = recording();
    telemetry.runtime.lock().expect("runtime").span_assembler = V2SpanAssembler::with_limits(1, 1);
    let first = trace("0123456789abcdef");
    let second = trace("fedcba9876543210");

    telemetry
        .emit_span(&v2::SpanSignal::Started(started(first.clone())))
        .expect("first span");
    telemetry
        .emit_span(&event(first.clone(), "first.event"))
        .expect("first event");
    telemetry
        .emit_span(&event(first.clone(), "second.event"))
        .expect("bounded event is accounted rather than rejected");
    telemetry
        .emit_span(&v2::SpanSignal::Started(started(second.clone())))
        .expect("second span evicts the oldest");

    let health = telemetry.health();
    assert_eq!(health.dropped_exports_total, 2, "{health:?}");
    {
        let runtime = telemetry.runtime.lock().expect("runtime");
        assert_eq!(runtime.trace_status.state, ExporterHealthState::Degraded);
        assert_eq!(
            runtime
                .trace_status
                .last_error
                .as_ref()
                .and_then(|summary| summary.code.clone()),
            Some(error_codes::OTLP_INCOMPLETE_SPAN_DROPPED)
        );
    }

    let evicted_end = started(first).end(v2::SpanStatus::Ok, DurationMs::from(1));
    telemetry
        .emit_span(&v2::SpanSignal::Ended(evicted_end))
        .expect_err("an evicted span has no live start");
    assert_eq!(telemetry.health().malformed_spans_total, 1);

    let ended = started(second.clone()).end(v2::SpanStatus::Ok, DurationMs::from(2));
    telemetry
        .emit_span(&v2::SpanSignal::Ended(ended))
        .expect("surviving span completes");
    telemetry.flush().expect("flush");
    let spans = exported_spans(&traces);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].record.record.trace(), &second);
}

#[test]
fn canonical_shutdown_counts_incomplete_spans() {
    let (telemetry, traces, _) = recording();
    telemetry
        .emit_span(&v2::SpanSignal::Started(started(trace("0123456789abcdef"))))
        .expect("started");
    telemetry.shutdown().expect("shutdown");

    let health = telemetry.health();
    assert_eq!(health.dropped_exports_total, 1, "{health:?}");
    assert!(exported_spans(&traces).is_empty());
}

#[test]
fn canonical_emit_log_checks_shutdown_before_an_invalid_entity_id() {
    let (telemetry, logs) = log_recording();
    telemetry.shutdown().expect("shutdown");

    let error = telemetry
        .emit_log(&log_event_with_entity_id("entity invalid"))
        .expect_err("shutdown takes precedence over entity validation");
    assert!(
        matches!(error, v2::TelemetryError::Shutdown { .. }),
        "{error:?}"
    );
    assert_eq!(exported_logs(&logs), 0);
}

#[test]
fn canonical_emit_log_skips_entity_validation_when_logs_or_transport_are_disabled() {
    let mut logs_disabled = telemetry_config();
    logs_disabled.logs = None;
    let telemetry = telemetry_with_config(
        logs_disabled,
        &Arc::new(RecordingLogExporter::<ExportRecord<LogRecord>>::default()),
    );
    telemetry
        .emit_log(&log_event_with_entity_id("entity invalid"))
        .expect("disabled logs return before entity validation");

    let mut transport_disabled = telemetry_config();
    transport_disabled.transport.enabled = false;
    let telemetry = telemetry_with_config(
        transport_disabled,
        &Arc::new(RecordingLogExporter::<ExportRecord<LogRecord>>::default()),
    );
    telemetry
        .emit_log(&log_event_with_entity_id("entity invalid"))
        .expect("disabled transport returns before entity validation");
}

#[test]
fn canonical_emit_log_rejects_an_invalid_entity_id_before_buffering() {
    let (telemetry, logs) = log_recording();
    let error = telemetry
        .emit_log(&log_event_with_entity_id("entity invalid"))
        .expect_err("enabled telemetry rejects an invalid entity id");
    assert!(matches!(error, v2::TelemetryError::Event(_)), "{error:?}");
    telemetry.flush().expect("nothing was buffered");
    assert_eq!(exported_logs(&logs), 0);
}

#[test]
fn canonical_shutdown_is_shared_idempotent_and_uses_the_explicit_timeout() {
    let lifecycle = Arc::new(RecordingLifecycle::default());
    let telemetry = Arc::new(recording_with_lifecycle(Arc::clone(&lifecycle)));
    let shutdown = Arc::clone(&telemetry);
    std::thread::spawn(move || shutdown.shutdown_with_timeout(Duration::ZERO))
        .join()
        .expect("shutdown thread must not panic")
        .expect("shutdown completes");

    telemetry.shutdown().expect("second shutdown is idempotent");
    assert!(matches!(
        telemetry.emit_span(&v2::SpanSignal::Started(started(trace("0123456789abcdef")))),
        Err(v2::TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        telemetry.flush(),
        Err(v2::FlushError::Drain { .. })
    ));
    assert_eq!(
        *lifecycle.calls.lock().expect("calls poisoned"),
        vec![
            LifecycleCall::BlockingPreflight,
            LifecycleCall::ShutdownBlockingWithTimeout(Duration::ZERO),
        ]
    );
}

#[test]
fn canonical_emit_preserves_every_span_field_and_histogram_bucket() {
    let (telemetry, traces, metrics) = recording();
    let trace = trace("0123456789abcdef")
        .with_parent(SpanId::new("1111111111111111").expect("valid parent"));
    let link = v2::SpanLink::new(
        TraceId::new("fedcba9876543210fedcba9876543210").expect("valid linked trace"),
        SpanId::new("fedcba9876543210").expect("valid linked span"),
        v2::TraceFlags::new(0x01),
        v2::Attributes::from([("link.kind".to_owned(), v2::AttributeValue::Bool(true))]),
    );
    let started = started(trace.clone())
        .with_kind(v2::SpanKind::Client)
        .with_links(vec![link]);
    let ended = started
        .clone()
        .end(v2::SpanStatus::Error, DurationMs::from(7));
    let span_event = event(trace, "tool.call");
    telemetry
        .emit_span(&v2::SpanSignal::Started(started))
        .expect("started");
    telemetry.emit_span(&span_event).expect("event");
    telemetry
        .emit_span(&v2::SpanSignal::Ended(ended.clone()))
        .expect("ended");

    let histogram = histogram_metric();
    telemetry.emit_metric(&histogram).expect("histogram");
    telemetry.flush().expect("flush");

    let spans = exported_spans(&traces);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].record.record, ended);
    let v2::SpanSignal::Event(expected_event) = span_event else {
        unreachable!("fixture is an event");
    };
    assert_eq!(spans[0].record.events, vec![expected_event]);
    let exported_metrics = metrics.batches.lock().expect("batches").concat();
    assert_eq!(exported_metrics.len(), 1);
    assert_eq!(exported_metrics[0].record, histogram);
    assert_eq!(telemetry.health().dropped_exports_total, 0);
}

#[test]
fn log_admission_exports_each_record_when_batch_size_is_one() {
    let mut config = telemetry_config();
    config.logs = Some(LogsConfig { batch_size: 1 });
    let (telemetry, logs, _, _) = recording_with_config(config);
    for message in ["one", "two", "three"] {
        telemetry
            .emit_log(&log_event_message(message))
            .expect("log");
    }
    assert_eq!(*logs.calls.lock().expect("calls"), vec![1, 1, 1]);
}

#[test]
fn shutdown_waits_for_detached_log_batch_before_backend_shutdown() {
    let (telemetry, entered, release, events) = shutdown_aware_fixture(ShutdownAwareSignal::Logs);
    let telemetry = Arc::new(telemetry);
    let emitter = Arc::clone(&telemetry);
    let export = thread::spawn(move || emitter.emit_log(&log_event_message("detached log batch")));
    entered.recv().expect("log export entered");

    let shutdown = Arc::clone(&telemetry);
    let shutdown = thread::spawn(move || shutdown.shutdown());
    wait_until_stopping(&telemetry);
    release.send(()).expect("release log export");

    export
        .join()
        .expect("log emitter thread")
        .expect("log export accepted before shutdown");
    shutdown
        .join()
        .expect("shutdown thread")
        .expect("shutdown succeeds");
    assert_eq!(*events.lock().expect("events"), ["export", "shutdown"]);
}

#[test]
fn shutdown_waits_for_detached_completed_span_before_backend_shutdown() {
    let (telemetry, entered, release, events) = shutdown_aware_fixture(ShutdownAwareSignal::Traces);
    let telemetry = Arc::new(telemetry);
    let context = trace("0123456789abcdef");
    let span = started(context);
    let emitter = Arc::clone(&telemetry);
    let export = thread::spawn(move || {
        emitter.emit_span(&v2::SpanSignal::Started(span.clone()))?;
        emitter.emit_span(&v2::SpanSignal::Ended(
            span.end(v2::SpanStatus::Ok, DurationMs::from(1)),
        ))
    });
    entered.recv().expect("span export entered");

    let shutdown = Arc::clone(&telemetry);
    let shutdown = thread::spawn(move || shutdown.shutdown());
    wait_until_stopping(&telemetry);
    release.send(()).expect("release span export");

    export
        .join()
        .expect("span emitter thread")
        .expect("span export accepted before shutdown");
    shutdown
        .join()
        .expect("shutdown thread")
        .expect("shutdown succeeds");
    assert_eq!(*events.lock().expect("events"), ["export", "shutdown"]);
}

#[test]
fn shutdown_waits_for_detached_metric_batch_before_backend_shutdown() {
    let (telemetry, entered, release, events) =
        shutdown_aware_fixture(ShutdownAwareSignal::Metrics);
    let telemetry = Arc::new(telemetry);
    let metric = histogram_metric();
    let emitter = Arc::clone(&telemetry);
    let export = thread::spawn(move || emitter.emit_metric(&metric));
    entered.recv().expect("metric export entered");

    let shutdown = Arc::clone(&telemetry);
    let shutdown = thread::spawn(move || shutdown.shutdown());
    wait_until_stopping(&telemetry);
    release.send(()).expect("release metric export");

    export
        .join()
        .expect("metric emitter thread")
        .expect("metric export accepted before shutdown");
    shutdown
        .join()
        .expect("shutdown thread")
        .expect("shutdown succeeds");
    assert_eq!(*events.lock().expect("events"), ["export", "shutdown"]);
}

#[test]
fn shutdown_timeout_counts_detached_batch_and_degrades_health() {
    let (telemetry, entered, release, events) = shutdown_aware_fixture(ShutdownAwareSignal::Logs);
    let telemetry = Arc::new(telemetry);
    let emitter = Arc::clone(&telemetry);
    let export = thread::spawn(move || emitter.emit_log(&log_event_message("timed out batch")));
    entered.recv().expect("log export entered");

    telemetry
        .shutdown_with_timeout(Duration::ZERO)
        .expect("timeout still completes shutdown");
    let health = telemetry.health();
    assert_eq!(health.dropped_exports_total, 1);
    assert_eq!(health.state, TelemetryHealthState::Degraded);
    assert_eq!(*events.lock().expect("events"), ["shutdown"]);

    release.send(()).expect("release timed out export");
    export
        .join()
        .expect("log emitter thread")
        .expect("admission completed before shutdown");
}

#[test]
fn sustained_log_admission_and_flush_never_export_above_batch_size() {
    let batch_size = 3;
    let mut config = telemetry_config();
    config.logs = Some(LogsConfig { batch_size });
    let (telemetry, logs, _, _) = recording_with_config(config);
    for index in 0..(10 * batch_size + 3) {
        telemetry
            .emit_log(&log_event_message(&index.to_string()))
            .expect("log");
    }
    telemetry.flush().expect("flush");
    let calls = logs.calls.lock().expect("calls").clone();
    assert!(calls.iter().all(|&len| len <= batch_size));
    assert_eq!(calls.into_iter().sum::<usize>(), 10 * batch_size + 3);
}

#[test]
fn completed_spans_and_metrics_export_in_configured_chunks() {
    let batch_size = 2;
    let mut config = telemetry_config();
    config.traces = Some(TracesConfig { batch_size });
    config.metrics = Some(MetricsConfig {
        batch_size,
        export_interval_ms: 60_000_u64.into(),
    });
    let (telemetry, _, traces, metrics) = recording_with_config(config);
    for index in 1..=5 {
        let context = trace(&format!("{index:016x}"));
        let span = started(context.clone());
        telemetry
            .emit_span(&v2::SpanSignal::Started(span.clone()))
            .expect("started");
        telemetry
            .emit_span(&v2::SpanSignal::Ended(
                span.end(v2::SpanStatus::Ok, DurationMs::from(1)),
            ))
            .expect("ended");
        telemetry.emit_metric(&histogram_metric()).expect("metric");
    }
    telemetry.flush().expect("flush");
    let span_calls = traces.calls.lock().expect("span calls").clone();
    let metric_calls = metrics.calls.lock().expect("metric calls").clone();
    assert!(span_calls.iter().all(|&len| len <= batch_size));
    assert!(metric_calls.iter().all(|&len| len <= batch_size));
    assert_eq!(span_calls, vec![2, 2, 1]);
    assert_eq!(metric_calls, vec![1, 2, 2]);
    assert_eq!(span_calls.into_iter().sum::<usize>(), 5);
    assert_eq!(metric_calls.into_iter().sum::<usize>(), 5);
}
