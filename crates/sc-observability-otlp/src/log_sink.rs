//! Native OpenTelemetry sink for the core logger (ADR-023).
//!
//! [`OtelLogSink`] maps each already-filtered, already-redacted
//! [`LogEvent`] to a native SDK log record and emits it through a logger from
//! a caller-owned [`SdkLoggerProvider`](sdk::logs::SdkLoggerProvider). The core
//! logger keeps level filtering, redaction and file fan-out; registering this
//! sink beside the built-in file sink sends the same events to both.

use std::time::SystemTime;

use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity};
use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry_sdk::logs::SdkLogger;
use sc_observability::v2::LogSink;
use sc_observability_types::v2::LogSinkError;
use sc_observability_types::{Level, LogEvent, SinkHealth, SinkHealthState, SinkName};
use serde_json::Value;

use crate::constants::{OTEL_LOG_SINK_NAME, SDK_DIAGNOSTIC_TARGET_PREFIX};
use crate::{api, sdk};

#[cfg(test)]
mod tests;

/// Core-logger sink that emits events through a native OpenTelemetry logger.
///
/// # Provider ownership
///
/// The caller builds, keeps and shuts down the
/// [`SdkLoggerProvider`](sdk::logs::SdkLoggerProvider). Configure it with the
/// official nonblocking `BatchLogProcessor`: [`LogSink::write`] then only
/// enqueues a record and returns, whatever the network does. A simple
/// processor would export on the writer thread and is not supported. The sink
/// never shuts the provider down and the provider remains usable after the
/// sink is dropped. Admission to the batch processor is not a delivery
/// guarantee: the SDK drops records when its queue is full.
///
/// # Mapping
///
/// The event timestamp, severity, message, target and valid trace context
/// map to the native record fields; the action becomes the `event.name`
/// attribute; the event service becomes `sc.observability.log.service`,
/// distinct from the resource's `service.name`. Optional identifiers,
/// outcome, diagnostic, state transition and process identity become
/// `sc.observability.log.*` attributes and event fields are added under their
/// own names. Events whose target starts with `opentelemetry` are SDK
/// diagnostics routed back through a bridge; the sink drops them so they never
/// recurse into the SDK.
#[derive(Debug)]
pub struct OtelLogSink {
    logger: SdkLogger,
}

impl OtelLogSink {
    /// Creates a sink that emits through `provider`'s logger for `scope`.
    #[must_use]
    pub fn new(
        provider: &sdk::logs::SdkLoggerProvider,
        scope: api::InstrumentationScope,
    ) -> Self {
        Self {
            logger: provider.logger_with_scope(scope),
        }
    }
}

impl LogSink for OtelLogSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        if event.target.as_str().starts_with(SDK_DIAGNOSTIC_TARGET_PREFIX) {
            return Ok(());
        }
        let mut record = self.logger.create_log_record();
        map_event(event, &mut record);
        self.logger.emit(record);
        Ok(())
    }

    /// Returns at once: export timing belongs to the caller's batch processor
    /// and provider, so the logger's flush never waits on the network.
    fn flush(&self) -> Result<(), LogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new(OTEL_LOG_SINK_NAME).expect("constant sink name is valid"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

/// Serializes one log event value; the shared event types always serialize.
macro_rules! json_text {
    ($value:expr) => {
        serde_json::to_string($value).expect("log event values serialize to JSON")
    };
}

fn map_event(event: &LogEvent, record: &mut sdk::logs::SdkLogRecord) {
    record.set_timestamp(SystemTime::from(event.timestamp.into_inner()));
    record.set_observed_timestamp(SystemTime::now());
    let (severity, text) = severity(event.level);
    record.set_severity_number(severity);
    record.set_severity_text(text);
    if let Some(message) = &event.message {
        record.set_body(AnyValue::from(message.clone()));
    }
    record.set_target(event.target.as_str().to_owned());
    record.add_attribute("event.name", event.action.as_str().to_owned());
    record.add_attribute("sc.observability.log.service", event.service.as_str().to_owned());
    record.add_attribute("sc.observability.log.version", json_text!(&event.version));
    record.add_attribute("sc.observability.log.identity", json_text!(&event.identity));
    let optional = [
        ("sc.observability.log.request_id", event.request_id.as_ref().map(|value| json_text!(value))),
        ("sc.observability.log.correlation_id", event.correlation_id.as_ref().map(|value| json_text!(value))),
        ("sc.observability.log.outcome", event.outcome.as_ref().map(|value| json_text!(value))),
        ("sc.observability.log.diagnostic", event.diagnostic.as_ref().map(|value| json_text!(value))),
        (
            "sc.observability.log.state_transition",
            event.state_transition.as_ref().map(|value| json_text!(value)),
        ),
    ];
    for (key, value) in optional {
        if let Some(value) = value {
            record.add_attribute(key, value);
        }
    }
    for (key, value) in &event.fields {
        if let Some(value) = any_value(value) {
            record.add_attribute(key.clone(), value);
        }
    }
    if let Some(trace) = &event.trace {
        let ids = (
            TraceId::from_hex(trace.trace_id.as_str()),
            SpanId::from_hex(trace.span_id.as_str()),
        );
        if let (Ok(trace_id), Ok(span_id)) = ids
            && trace_id != TraceId::INVALID
            && span_id != SpanId::INVALID
        {
            record.set_trace_context(trace_id, span_id, None);
        }
        if let Some(parent) = &trace.parent_span_id {
            record.add_attribute(
                "sc.observability.log.parent_span_id",
                parent.as_str().to_owned(),
            );
        }
    }
}

const fn severity(level: Level) -> (Severity, &'static str) {
    let (_, text) = crate::severity::fields(level);
    let severity = match level {
        Level::Trace => Severity::Trace,
        Level::Debug => Severity::Debug,
        Level::Info => Severity::Info,
        Level::Warn => Severity::Warn,
        Level::Error => Severity::Error,
    };
    (severity, text)
}


fn any_value(value: &Value) -> Option<AnyValue> {
    match value {
        Value::Null => None,
        Value::Bool(value) => Some(AnyValue::Boolean(*value)),
        Value::Number(number) => number
            .as_i64()
            .map(AnyValue::Int)
            .or_else(|| number.as_f64().map(AnyValue::Double)),
        Value::String(value) => Some(AnyValue::from(value.clone())),
        Value::Array(_) | Value::Object(_) => Some(AnyValue::from(value.to_string())),
    }
}
