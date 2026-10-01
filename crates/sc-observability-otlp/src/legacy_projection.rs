//! Projection of the legacy facade signal model onto neutral OTLP records.
//!
//! Kept separate from the facade so transport-compatibility conversion cannot
//! obscure lifecycle ownership and public telemetry behavior.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::assembly::CompleteSpan;
use crate::contracts::{self, LogExporter, MetricExporter, TraceExporter};
use sc_observability_types::otlp::{
    OtlpCompleteSpan, OtlpInstrumentationScope, OtlpLogRecord, OtlpRecord, OtlpResource,
};
use sc_observability_types::v2::{ConfigFailure, ExportError};
use sc_observability_types::{ErrorContext, LogEvent, MetricRecord, Remediation, ServiceName};

use super::ExporterSet;

#[allow(
    dead_code,
    reason = "the optional legacy backend selects this projection only when enabled"
)]
pub(super) fn raw_exporter_set(exporters: contracts::ExporterSet) -> ExporterSet {
    ExporterSet {
        logs: Arc::new(RawLogExporter {
            inner: exporters.logs,
        }),
        traces: Arc::new(RawTraceExporter {
            inner: exporters.traces,
        }),
        metrics: Arc::new(RawMetricExporter {
            inner: exporters.metrics,
        }),
        lifecycle: exporters.lifecycle,
    }
}

#[allow(dead_code)]
struct RawLogExporter {
    inner: Arc<dyn LogExporter>,
}
impl LogExporter<LogEvent> for RawLogExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError> {
        self.inner
            .export_logs(&batch.iter().map(log_record).collect::<Vec<_>>())
    }
}
#[allow(dead_code)]
struct RawTraceExporter {
    inner: Arc<dyn TraceExporter>,
}
impl TraceExporter<CompleteSpan> for RawTraceExporter {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError> {
        let records = batch
            .iter()
            .map(span_record)
            .collect::<Result<Vec<_>, _>>()?;
        self.inner.export_spans(&records)
    }
}
#[allow(dead_code)]
struct RawMetricExporter {
    inner: Arc<dyn MetricExporter>,
}
impl MetricExporter<MetricRecord> for RawMetricExporter {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError> {
        let records = batch
            .iter()
            .map(metric_record)
            .collect::<Result<Vec<_>, _>>()?;
        self.inner.export_metrics(&records)
    }
}

#[allow(dead_code)]
fn resource(service: &ServiceName) -> OtlpResource {
    OtlpResource {
        attributes: BTreeMap::from_iter([(
            "service.name".to_owned(),
            sc_observability_types::v2::AttributeValue::String(service.as_str().to_owned()),
        )]),
        schema_url: None,
    }
}
#[allow(dead_code)]
fn log_record(event: &LogEvent) -> OtlpRecord<OtlpLogRecord> {
    OtlpRecord {
        resource: resource(&event.service),
        scope: OtlpInstrumentationScope::default(),
        record: OtlpLogRecord {
            event: event.clone(),
            trace_flags: sc_observability_types::v2::TraceFlags::default(),
            attributes: BTreeMap::new(),
        },
    }
}
#[allow(dead_code)]
fn span_record(span: &CompleteSpan) -> Result<OtlpRecord<OtlpCompleteSpan>, ExportError> {
    let trace = trace_context(span.record.trace());
    let mut started = sc_observability_types::v2::SpanRecord::new(
        span.record.timestamp(),
        span.record.service().clone(),
        span.record.name().clone(),
        trace,
        attributes(span.record.attributes()),
    );
    if let Some(diagnostic) = span.record.diagnostic().cloned() {
        started = started.with_diagnostic(diagnostic);
    }
    let duration = span
        .record
        .duration_ms()
        .ok_or_else(|| transport_error("completed span has no duration"))?;
    let ended = started.end(
        match span.record.status() {
            sc_observability_types::SpanStatus::Ok => sc_observability_types::v2::SpanStatus::Ok,
            sc_observability_types::SpanStatus::Error => {
                sc_observability_types::v2::SpanStatus::Error
            }
            sc_observability_types::SpanStatus::Unset => {
                sc_observability_types::v2::SpanStatus::Unset
            }
        },
        duration,
    );
    Ok(OtlpRecord {
        resource: resource(span.record.service()),
        scope: OtlpInstrumentationScope::default(),
        record: OtlpCompleteSpan {
            record: ended,
            events: span
                .events
                .iter()
                .map(|event| sc_observability_types::v2::SpanEvent {
                    timestamp: event.timestamp,
                    trace: trace_context(&event.trace),
                    name: event.name.clone(),
                    attributes: attributes(&event.attributes),
                    diagnostic: event.diagnostic.clone(),
                })
                .collect(),
        },
    })
}

#[allow(dead_code)]
pub(super) fn trace_context(
    trace: &sc_observability_types::TraceContext,
) -> sc_observability_types::v2::TraceContext {
    let context = sc_observability_types::v2::TraceContext::new(
        trace.trace_id.clone(),
        trace.span_id.clone(),
        sc_observability_types::v2::TraceFlags::default(),
    );
    match trace.parent_span_id.clone() {
        Some(parent) => context.with_parent(parent),
        None => context,
    }
}
#[allow(dead_code)]
fn metric_record(
    metric: &MetricRecord,
) -> Result<OtlpRecord<sc_observability_types::v2::MetricRecord>, ExportError> {
    let value = match metric.kind {
        sc_observability_types::MetricKind::Gauge => {
            sc_observability_types::v2::MetricValue::Gauge(
                sc_observability_types::v2::FiniteF64::new(metric.value)
                    .map_err(|_| transport_error("non-finite metric value"))?,
            )
        }
        sc_observability_types::MetricKind::Counter => {
            sc_observability_types::v2::MetricValue::Sum {
                value: sc_observability_types::v2::FiniteF64::new(metric.value)
                    .map_err(|_| transport_error("non-finite metric value"))?,
                monotonic: true,
                temporality: sc_observability_types::v2::AggregationTemporality::Cumulative,
                start_time: metric.timestamp,
            }
        }
        sc_observability_types::MetricKind::Histogram => {
            return Err(transport_error(
                "legacy scalar histogram cannot be projected to the canonical histogram contract",
            ));
        }
    };
    let record = sc_observability_types::v2::MetricRecord::try_new(
        metric.timestamp,
        metric.service.clone(),
        metric.name.clone(),
        value,
    )
    .map_err(|_| transport_error("metric violates canonical interval contract"))?
    .with_unit(metric.unit.clone())
    .with_attributes(attributes(&metric.attributes));
    Ok(OtlpRecord {
        resource: resource(&metric.service),
        scope: OtlpInstrumentationScope::default(),
        record,
    })
}
#[allow(dead_code)]
fn attributes(
    values: &serde_json::Map<String, serde_json::Value>,
) -> sc_observability_types::v2::Attributes {
    values
        .iter()
        .map(|(key, value)| (key.clone(), attribute(value)))
        .collect()
}
#[allow(dead_code)]
fn attribute(value: &serde_json::Value) -> sc_observability_types::v2::AttributeValue {
    use sc_observability_types::v2::{AttributeValue, FiniteF64};
    match value {
        serde_json::Value::Null => AttributeValue::Null,
        serde_json::Value::Bool(v) => AttributeValue::Bool(*v),
        serde_json::Value::Number(v) => v
            .as_i64()
            .map(AttributeValue::Int)
            .or_else(|| v.as_u64().map(AttributeValue::UInt))
            .or_else(|| {
                v.as_f64()
                    .and_then(|n| FiniteF64::new(n).ok())
                    .map(AttributeValue::Float)
            })
            .unwrap_or(AttributeValue::Null),
        serde_json::Value::String(v) => AttributeValue::String(v.clone()),
        serde_json::Value::Array(v) => AttributeValue::Array(v.iter().map(attribute).collect()),
        serde_json::Value::Object(v) => AttributeValue::Object(attributes(v)),
    }
}
#[allow(dead_code)]
fn transport_error(message: &str) -> ExportError {
    ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            sc_observability_types::error_codes::otlp::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("correct the signal before exporting"),
        )),
    }
}
#[allow(dead_code)]
pub(super) fn transport_construction_failure(error: ExportError) -> ConfigFailure {
    ConfigFailure::TransportConstructionFailed {
        context: Box::new(
            ErrorContext::new(
                sc_observability_types::error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
                "the selected exporter transport could not be constructed",
                Remediation::recoverable(
                    "correct the transport configuration",
                    ["inspect the preserved exporter failure cause"],
                ),
            )
            .source(Box::new(error)),
        ),
    }
}
