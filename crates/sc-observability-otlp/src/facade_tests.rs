//! Canonical facade ingress coverage kept independent of the retired 1.x facade.

use std::sync::Arc;

use crate::assembly::V2SpanAssembler;
use crate::config::{
    LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, TelemetryConfig, TelemetryConfigBuilder,
    TracesConfig,
};
use crate::constants::{MAX_OTLP_EVENTS_PER_SPAN, MAX_OTLP_LIVE_SPANS};
use crate::contracts::{self, ExportRecord, LogRecord};
use crate::testing::{RecordingLogExporter, RecordingMetricExporter, RecordingTraceExporter};
use crate::{ExporterHealthState, RuntimeTelemetry, error_codes};
use sc_observability_types::v2::{self, MetricRecord as CanonicalMetricRecord};
use sc_observability_types::{
    ActionName, DurationMs, MetricName, ServiceName, SpanId, Timestamp, TraceId,
};

type Traces = RecordingTraceExporter<ExportRecord<contracts::CompleteSpan>>;
type Metrics = RecordingMetricExporter<ExportRecord<CanonicalMetricRecord>>;

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
    let traces = Arc::new(Traces::default());
    let metrics = Arc::new(Metrics::default());
    let telemetry = RuntimeTelemetry::new_with_exporters_typed(
        telemetry_config(),
        Arc::new(RecordingLogExporter::<ExportRecord<LogRecord>>::default()),
        traces.clone(),
        metrics.clone(),
    )
    .expect("recording telemetry");
    (telemetry, traces, metrics)
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
    telemetry.shutdown_typed().expect("shutdown");

    let health = telemetry.health();
    assert_eq!(health.dropped_exports_total, 1, "{health:?}");
    assert!(exported_spans(&traces).is_empty());
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

    let one_second: Timestamp =
        serde_json::from_str("\"1970-01-01T00:00:01Z\"").expect("valid timestamp");
    let histogram = v2::MetricRecord::try_new(
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
    .expect("valid histogram metric");
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
