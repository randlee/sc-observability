//! Native signal construction shared by the CLI and Python frontends.
//!
//! Frontends extract their arguments and apply their own option rules; these
//! types then build the same native log record, metric instrument and span.

use std::time::SystemTime;

use opentelemetry::logs::LogRecord as _;
use opentelemetry::trace::{SpanId, TraceId};

use super::{SyncError, parent_span_is_remote, span_times};
use crate::{api, sdk};

/// One log record a frontend sends: severity, body, optional trace context
/// and attributes.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Native severity; its name is also the record's severity text.
    pub severity: api::logs::Severity,
    /// Record body.
    pub body: String,
    /// Trace and span the record belongs to.
    pub trace_context: Option<(TraceId, SpanId)>,
    /// Record attributes in input order.
    pub attributes: Vec<(String, api::logs::AnyValue)>,
}

impl LogEntry {
    /// Fills a native record from this entry, timestamped now.
    ///
    /// Pass it to [`Client::send_log`](super::Client::send_log):
    /// `client.send_log(&resource, scope, |record| { entry.fill(record); Ok(()) })`.
    pub fn fill(self, record: &mut sdk::logs::SdkLogRecord) {
        record.set_timestamp(SystemTime::now());
        record.set_severity_number(self.severity);
        record.set_severity_text(self.severity.name());
        record.set_body(api::logs::AnyValue::from(self.body));
        if let Some((trace_id, span_id)) = self.trace_context {
            record.set_trace_context(trace_id, span_id, None);
        }
        for (key, value) in self.attributes {
            record.add_attribute(key, value);
        }
    }
}

/// Native `f64` instrument kind of one measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// Monotonic sum ([`Counter`](api::metrics::Counter)).
    Counter,
    /// Non-monotonic sum ([`UpDownCounter`](api::metrics::UpDownCounter)).
    UpDownCounter,
    /// Last value ([`Gauge`](api::metrics::Gauge)).
    Gauge,
    /// Distribution ([`Histogram`](api::metrics::Histogram)).
    Histogram,
}

/// One metric measurement a frontend sends on a fresh instrument.
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Instrument name.
    pub name: String,
    /// Instrument kind.
    pub kind: MetricKind,
    /// Measured value.
    pub value: f64,
    /// Instrument unit.
    pub unit: Option<String>,
    /// Instrument description.
    pub description: Option<String>,
    /// Measurement attributes.
    pub attributes: Vec<api::KeyValue>,
}

impl Measurement {
    /// Builds the instrument on `meter` and records the value once.
    ///
    /// Pass it to [`Client::send_metrics`](super::Client::send_metrics):
    /// `client.send_metrics(&resource, scope, |meter| { measurement.record(meter); Ok(()) })`.
    pub fn record(self, meter: &api::metrics::Meter) {
        let Self {
            name,
            kind,
            value,
            unit,
            description,
            attributes,
        } = self;
        macro_rules! instrument {
            ($builder:expr) => {{
                let mut builder = $builder;
                if let Some(unit) = unit {
                    builder = builder.with_unit(unit);
                }
                if let Some(description) = description {
                    builder = builder.with_description(description);
                }
                builder.build()
            }};
        }
        match kind {
            MetricKind::Counter => instrument!(meter.f64_counter(name)).add(value, &attributes),
            MetricKind::UpDownCounter => {
                instrument!(meter.f64_up_down_counter(name)).add(value, &attributes);
            }
            MetricKind::Gauge => instrument!(meter.f64_gauge(name)).record(value, &attributes),
            MetricKind::Histogram => {
                instrument!(meter.f64_histogram(name)).record(value, &attributes);
            }
        }
    }
}

/// Returns the span status for a frontend's error description and ok flag.
///
/// An error description wins over `ok`. A frontend that treats both together as
/// invalid input rejects that combination before calling this.
#[must_use]
pub fn span_status(error: Option<String>, ok: bool) -> api::trace::Status {
    match (error, ok) {
        (Some(description), _) => api::trace::Status::error(description),
        (None, true) => api::trace::Status::Ok,
        (None, false) => api::trace::Status::Unset,
    }
}

/// One completed span a frontend sends.
#[derive(Debug, Clone)]
pub struct CompletedSpan {
    /// Span name.
    pub name: String,
    /// Trace ID; a random one when absent.
    pub trace_id: Option<TraceId>,
    /// Span ID; a random one when absent.
    pub span_id: Option<SpanId>,
    /// Parent span ID, recorded as a local parent.
    pub parent_span_id: Option<SpanId>,
    /// Span kind.
    pub kind: api::trace::SpanKind,
    /// Start time in Unix nanoseconds; the end time when absent.
    pub start_time_unix_nano: Option<u64>,
    /// End time in Unix nanoseconds; now when absent.
    pub end_time_unix_nano: Option<u64>,
    /// Span status, usually from [`span_status`].
    pub status: api::trace::Status,
    /// Span attributes.
    pub attributes: Vec<api::KeyValue>,
}

impl CompletedSpan {
    /// Builds the sampled native span for [`Client::send_span`](super::Client::send_span).
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] with
    /// [`error_codes::sync::INVALID_RECORD`](crate::error_codes::sync::INVALID_RECORD)
    /// when the start time is after the end time.
    pub fn into_span_data(
        self,
        scope: api::InstrumentationScope,
    ) -> Result<sdk::trace::SpanData, SyncError> {
        use sdk::trace::IdGenerator as _;

        let (start_time, end_time) =
            span_times(self.start_time_unix_nano, self.end_time_unix_nano)?;
        let ids = sdk::trace::RandomIdGenerator::default();
        Ok(sdk::trace::SpanData {
            span_context: api::trace::SpanContext::new(
                self.trace_id.unwrap_or_else(|| ids.new_trace_id()),
                self.span_id.unwrap_or_else(|| ids.new_span_id()),
                api::trace::TraceFlags::SAMPLED,
                false,
                api::trace::TraceState::NONE,
            ),
            parent_span_id: self.parent_span_id.unwrap_or(SpanId::INVALID),
            parent_span_is_remote: parent_span_is_remote(),
            span_kind: self.kind,
            name: self.name.into(),
            start_time,
            end_time,
            attributes: self.attributes,
            dropped_attributes_count: 0,
            events: sdk::trace::SpanEvents::default(),
            links: sdk::trace::SpanLinks::default(),
            status: self.status,
            instrumentation_scope: scope,
        })
    }
}
