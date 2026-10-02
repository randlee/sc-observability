//! Projection of the legacy facade signal model onto neutral OTLP records.
//!
//! Kept separate from the facade so transport-compatibility conversion cannot
//! obscure lifecycle ownership and public telemetry behavior.

use std::collections::BTreeMap;

use crate::contracts::{ExportRecord, LogRecord};
use sc_observability_types::otlp::{OtlpInstrumentationScope, OtlpLogRecord, OtlpResource};
use sc_observability_types::v2::{ConfigFailure, ExportError};
use sc_observability_types::{
    ErrorContext, LogEvent, MetricRecord, Remediation, ServiceName, SpanSignal,
};

pub(super) fn resource(service: &ServiceName) -> OtlpResource {
    OtlpResource {
        attributes: BTreeMap::from_iter([(
            "service.name".to_owned(),
            sc_observability_types::v2::AttributeValue::String(service.as_str().to_owned()),
        )]),
        schema_url: None,
    }
}
pub(super) fn log_record(event: &LogEvent) -> ExportRecord<LogRecord> {
    ExportRecord {
        resource: resource(&event.service),
        scope: OtlpInstrumentationScope::default(),
        record: OtlpLogRecord {
            event: event.clone(),
            trace_flags: sc_observability_types::v2::TraceFlags::default(),
            attributes: BTreeMap::new(),
        },
    }
}
/// Converts a released span signal to the canonical model at admission.
///
/// Released spans carry no kind, links or trace flags, so the canonical
/// defaults apply; every released field is preserved.
pub(super) fn span_signal(
    signal: &SpanSignal,
) -> Result<sc_observability_types::v2::SpanSignal, ExportError> {
    use sc_observability_types::v2::SpanSignal as Canonical;
    Ok(match signal {
        SpanSignal::Started(record) => Canonical::Started(started_record(record)),
        SpanSignal::Event(event) => Canonical::Event(span_event(event)),
        SpanSignal::Ended(record) => {
            let duration = record
                .duration_ms()
                .ok_or_else(|| transport_error("completed span has no duration"))?;
            Canonical::Ended(started_record(record).end(span_status(record.status()), duration))
        }
    })
}

fn started_record<S>(
    record: &sc_observability_types::SpanRecord<S>,
) -> sc_observability_types::v2::SpanRecord<sc_observability_types::v2::SpanStarted> {
    let started = sc_observability_types::v2::SpanRecord::new(
        record.timestamp(),
        record.service().clone(),
        record.name().clone(),
        trace_context(record.trace()),
        attributes(record.attributes()),
    );
    match record.diagnostic().cloned() {
        Some(diagnostic) => started.with_diagnostic(diagnostic),
        None => started,
    }
}

fn span_status(
    status: sc_observability_types::SpanStatus,
) -> sc_observability_types::v2::SpanStatus {
    match status {
        sc_observability_types::SpanStatus::Ok => sc_observability_types::v2::SpanStatus::Ok,
        sc_observability_types::SpanStatus::Error => sc_observability_types::v2::SpanStatus::Error,
        sc_observability_types::SpanStatus::Unset => sc_observability_types::v2::SpanStatus::Unset,
    }
}

fn span_event(event: &sc_observability_types::SpanEvent) -> sc_observability_types::v2::SpanEvent {
    sc_observability_types::v2::SpanEvent {
        timestamp: event.timestamp,
        trace: trace_context(&event.trace),
        name: event.name.clone(),
        attributes: attributes(&event.attributes),
        diagnostic: event.diagnostic.clone(),
    }
}

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
pub(super) fn metric_record(
    metric: &MetricRecord,
) -> Result<ExportRecord<sc_observability_types::v2::MetricRecord>, ExportError> {
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
    Ok(ExportRecord {
        resource: resource(&metric.service),
        scope: OtlpInstrumentationScope::default(),
        record,
    })
}
fn attributes(
    values: &serde_json::Map<String, serde_json::Value>,
) -> sc_observability_types::v2::Attributes {
    values
        .iter()
        .map(|(key, value)| (key.clone(), attribute(value)))
        .collect()
}
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
fn transport_error(message: &str) -> ExportError {
    ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            sc_observability_types::error_codes::otlp::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("correct the signal before exporting"),
        )),
    }
}
#[cfg_attr(
    not(any(feature = "otlp-sdk", feature = "sync-http")),
    expect(
        dead_code,
        reason = "only the compiled exporter backends construct transport failures"
    )
)]
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
