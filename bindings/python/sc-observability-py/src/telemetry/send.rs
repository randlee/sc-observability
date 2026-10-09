//! Extracts Python arguments while holding the GIL, then builds, uses and
//! drops one synchronous client per call with the GIL released.
use crate::{internal_failure, result_json, unavailable_failure};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyFloat, PyInt, PyString};
use sc_observability_dto::{Failure, boundary_diagnostic};
use sc_observability_otlp::api::logs::{AnyValue, Severity};
use sc_observability_otlp::api::trace::SpanKind;
use sc_observability_otlp::api::{InstrumentationScope, KeyValue, Value};
use sc_observability_otlp::error_codes::{TELEMETRY_EXPORT_FAILED, sync as codes};
use sc_observability_otlp::sdk::Resource;
use sc_observability_otlp::sdk::error::OTelSdkError;
use sc_observability_otlp::sdk::trace::SpanData;
use sc_observability_otlp::sync::{
    Client, CompletedSpan, InputByteCounter, LogEntry, Measurement, MetricKind, SyncError,
    check_input_limits, parse_span_id, parse_trace_id, read_root_certificate, resolve_endpoint,
    span_status, unsigned_attribute,
};
use std::{panic::AssertUnwindSafe, path::PathBuf, time::Duration};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

const SCOPE_NAME: &str = "sc_observability";

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(super) struct Config {
    endpoint: Option<String>,
    headers: Vec<(String, String)>,
    timeout_s: Option<f64>,
    root_certificate: Option<PathBuf>,
    service_name: Option<String>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(super) struct LogFields<'py> {
    body: String,
    severity: String,
    trace_id: Option<String>,
    span_id: Option<String>,
    attributes: Vec<(String, Bound<'py, PyAny>)>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(super) struct SpanFields<'py> {
    name: String,
    trace_id: Option<String>,
    span_id: Option<String>,
    parent_span_id: Option<String>,
    kind: String,
    start_time_unix_nano: Option<Bound<'py, PyInt>>,
    end_time_unix_nano: Option<Bound<'py, PyInt>>,
    ok: bool,
    error: Option<String>,
    attributes: Vec<(String, Bound<'py, PyAny>)>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(super) struct MetricFields<'py> {
    name: String,
    kind: String,
    value: f64,
    unit: Option<String>,
    description: Option<String>,
    attributes: Vec<(String, Bound<'py, PyAny>)>,
}

/// One attribute value; nested and null values are rejected, not flattened.
#[derive(Debug, PartialEq)]
enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Scalar {
    fn native<T: From<bool> + From<i64> + From<f64> + From<String>>(self) -> T {
        match self {
            Self::Bool(value) => T::from(value),
            Self::Int(value) => T::from(value),
            Self::Float(value) => T::from(value),
            Self::Str(value) => T::from(value),
        }
    }
}

impl From<i64> for Scalar {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<String> for Scalar {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

enum Signal {
    Log(LogEntry),
    Span(Box<SpanData>),
    Metric(Measurement),
}

/// Exports one log record.
#[pyfunction]
pub(super) fn send_log(py: Python<'_>, config: Config, fields: LogFields<'_>) -> String {
    run(py, config, "send_log", || log(fields).map(Signal::Log))
}

/// Exports one completed span.
#[pyfunction]
pub(super) fn send_span(py: Python<'_>, config: Config, fields: SpanFields<'_>) -> String {
    run(py, config, "send_span", || {
        span(fields).map(|span| Signal::Span(Box::new(span)))
    })
}

/// Exports one metric measurement.
#[pyfunction]
pub(super) fn send_metric(py: Python<'_>, config: Config, fields: MetricFields<'_>) -> String {
    run(py, config, "send_metric", || {
        metric(fields).map(Signal::Metric)
    })
}

/// Returns the ADR-014 `Result[None]` wire envelope.
fn run(
    py: Python<'_>,
    config: Config,
    operation: &str,
    prepare: impl FnOnce() -> Result<Signal, SyncError>,
) -> String {
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let signal = prepare()?;
        py.detach(move || export(config, signal))
    }));
    result_json(match outcome {
        Ok(result) => result.map_err(|error| failure(operation, &error)),
        Err(_) => Err(internal_failure("native telemetry call panicked")),
    })
}

/// Projects a client failure, keeping its registry code. `field` names the
/// rejected native argument. Display text only: the client redacts header
/// values and URL userinfo there.
fn failure(operation: &str, error: &SyncError) -> Failure {
    match error {
        // Running inside an entered Tokio runtime is a property of the calling
        // thread, not of any input field.
        SyncError::Validation {
            code: codes::RUNTIME_ENTERED,
            ..
        } => unavailable_failure(codes::RUNTIME_ENTERED, error.to_string()),
        SyncError::Validation { code, .. } => Failure::Validation {
            diagnostic: Box::new(boundary_diagnostic(code, error.to_string())),
            field: match *code {
                codes::INVALID_CONFIG => "config",
                _ => "fields",
            }
            .into(),
        },
        SyncError::Export(OTelSdkError::Timeout(_)) => Failure::Timeout {
            diagnostic: Box::new(boundary_diagnostic(
                TELEMETRY_EXPORT_FAILED.as_str(),
                error.to_string(),
            )),
            operation: operation.into(),
        },
        SyncError::Export(_) => {
            unavailable_failure(TELEMETRY_EXPORT_FAILED.as_str(), error.to_string())
        }
    }
}

fn export(config: Config, signal: Signal) -> Result<(), SyncError> {
    let mut client = client(&config)?;
    let mut resource = Resource::builder();
    if let Some(service) = config.service_name {
        InputByteCounter::new().add_text(&service)?;
        resource = resource.with_service_name(service);
    }
    let resource = resource.build();
    match signal {
        Signal::Log(entry) => client.send_log(&resource, scope(), |record| {
            entry.fill(record);
            Ok(())
        }),
        Signal::Span(span) => client.send_span(&resource, *span),
        Signal::Metric(measurement) => client.send_metrics(&resource, scope(), |meter| {
            measurement.record(meter);
            Ok(())
        }),
    }
}

fn client(config: &Config) -> Result<Client, SyncError> {
    let endpoint = resolve_endpoint(config.endpoint.as_deref())?;
    let mut client = Client::new(&endpoint)?;
    for (name, value) in &config.headers {
        client = client.with_header(name, value)?;
    }
    if let Some(seconds) = config.timeout_s {
        let timeout = Duration::try_from_secs_f64(seconds).map_err(|error| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("timeout_s {seconds}: {error}"),
            )
        })?;
        client = client.with_timeout(timeout)?;
    }
    if let Some(path) = &config.root_certificate {
        let pem = read_root_certificate(path)?;
        client = client.with_root_certificate_pem(&pem)?;
    }
    Ok(client)
}

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder(SCOPE_NAME)
        .with_version(env!("CARGO_PKG_VERSION"))
        .build()
}

fn log(fields: LogFields<'_>) -> Result<LogEntry, SyncError> {
    let severity = match fields.severity.as_str() {
        "trace" => Severity::Trace,
        "debug" => Severity::Debug,
        "info" => Severity::Info,
        "warn" => Severity::Warn,
        "error" => Severity::Error,
        "fatal" => Severity::Fatal,
        other => return Err(invalid(format!("unknown severity {other:?}"))),
    };
    let trace = match (fields.trace_id.as_deref(), fields.span_id.as_deref()) {
        (Some(trace_id), Some(span_id)) => Some((
            parse_trace_id("trace_id", trace_id)?,
            parse_span_id("span_id", span_id)?,
        )),
        (None, None) => None,
        _ => {
            return Err(invalid(
                "trace_id and span_id must be given together".into(),
            ));
        }
    };
    let attributes = attributes(fields.attributes, &[&fields.body])?
        .into_iter()
        .map(|(key, value)| (key, value.native::<AnyValue>()))
        .collect();
    Ok(LogEntry {
        severity,
        body: fields.body,
        trace_context: trace,
        attributes,
    })
}

fn span(fields: SpanFields<'_>) -> Result<SpanData, SyncError> {
    if fields.parent_span_id.is_some() && fields.trace_id.is_none() {
        return Err(invalid("parent_span_id requires trace_id".into()));
    }
    if fields.error.is_some() && fields.ok {
        return Err(invalid("ok and error are mutually exclusive".into()));
    }
    let kind = match fields.kind.as_str() {
        "internal" => SpanKind::Internal,
        "server" => SpanKind::Server,
        "client" => SpanKind::Client,
        "producer" => SpanKind::Producer,
        "consumer" => SpanKind::Consumer,
        other => return Err(invalid(format!("unknown span kind {other:?}"))),
    };
    let trace_id = fields
        .trace_id
        .as_deref()
        .map(|value| parse_trace_id("trace_id", value))
        .transpose()?;
    let span_id = fields
        .span_id
        .as_deref()
        .map(|value| parse_span_id("span_id", value))
        .transpose()?;
    let parent_span_id = fields
        .parent_span_id
        .as_deref()
        .map(|value| parse_span_id("parent_span_id", value))
        .transpose()?;
    let end_time_unix_nano = fields
        .end_time_unix_nano
        .map(|nanos| unix_nanos("end_time_unix_nano", &nanos))
        .transpose()?;
    let start_time_unix_nano = fields
        .start_time_unix_nano
        .map(|nanos| unix_nanos("start_time_unix_nano", &nanos))
        .transpose()?;
    let attributes = attributes(
        fields.attributes,
        &[&fields.name, fields.error.as_deref().unwrap_or_default()],
    )?;
    CompletedSpan {
        name: fields.name,
        trace_id,
        span_id,
        parent_span_id,
        kind,
        start_time_unix_nano,
        end_time_unix_nano,
        status: span_status(fields.error, fields.ok),
        attributes: key_values(attributes),
    }
    .into_span_data(scope())
}

fn metric(fields: MetricFields<'_>) -> Result<Measurement, SyncError> {
    let kind = match fields.kind.as_str() {
        "counter" => MetricKind::Counter,
        "up_down_counter" => MetricKind::UpDownCounter,
        "gauge" => MetricKind::Gauge,
        "histogram" => MetricKind::Histogram,
        other => return Err(invalid(format!("unknown metric kind {other:?}"))),
    };
    let attributes = attributes(
        fields.attributes,
        &[
            &fields.name,
            fields.unit.as_deref().unwrap_or_default(),
            fields.description.as_deref().unwrap_or_default(),
        ],
    )?;
    Ok(Measurement {
        name: fields.name,
        kind,
        value: fields.value,
        unit: fields.unit,
        description: fields.description,
        attributes: key_values(attributes),
    })
}

/// Converts attributes, counting every `signal_text` field plus every key and
/// string value against the shared per-call input limits.
fn attributes(
    attributes: Vec<(String, Bound<'_, PyAny>)>,
    signal_text: &[&str],
) -> Result<Vec<(String, Scalar)>, SyncError> {
    check_input_limits(0, attributes.len())?;
    let mut input = InputByteCounter::new();
    for text in signal_text {
        input.add_text(text)?;
    }
    let mut converted = Vec::with_capacity(attributes.len());
    for (key, value) in attributes {
        let value = scalar(&key, &value)?;
        input.add_text(&key)?;
        if let Scalar::Str(text) = &value {
            input.add_text(text)?;
        }
        converted.push((key, value));
    }
    Ok(converted)
}

fn scalar(key: &str, value: &Bound<'_, PyAny>) -> Result<Scalar, SyncError> {
    let unsupported = || {
        invalid(format!(
            "attribute {key}: only str, bool, int and float values are supported"
        ))
    };
    if value.is_instance_of::<PyBool>() {
        value.extract().map(Scalar::Bool).map_err(|_| unsupported())
    } else if value.is_instance_of::<PyInt>() {
        value
            .extract::<i64>()
            .map(Scalar::from)
            .or_else(|_| value.extract::<u64>().map(unsigned_attribute::<Scalar>))
            .map_err(|_| {
                invalid(format!(
                    "attribute {key}: integer outside the i64/u64 range"
                ))
            })
    } else if value.is_instance_of::<PyFloat>() {
        value
            .extract()
            .map(Scalar::Float)
            .map_err(|_| unsupported())
    } else if value.is_instance_of::<PyString>() {
        value.extract().map(Scalar::Str).map_err(|_| unsupported())
    } else {
        Err(unsupported())
    }
}

fn key_values(attributes: Vec<(String, Scalar)>) -> Vec<KeyValue> {
    attributes
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, value.native::<Value>()))
        .collect()
}

fn invalid(message: String) -> SyncError {
    SyncError::validation(codes::INVALID_RECORD, message)
}

fn unix_nanos(field: &str, nanos: &Bound<'_, PyInt>) -> Result<u64, SyncError> {
    nanos.extract().map_err(|_| {
        invalid(format!(
            "{field}: integer outside the unsigned 64-bit range"
        ))
    })
}
