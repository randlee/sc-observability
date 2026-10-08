//! Converts parsed arguments to native SDK inputs and sends them with the
//! shared synchronous client.

use crate::cli::{Cli, Command, Kind, LogArgs, LogSeverity, MetricArgs, MetricKind, SpanArgs};
use crate::constants::{
    DEFAULT_ENDPOINT, ENDPOINT_ENV, EXIT_EXPORT, EXIT_VALIDATION, SCOPE_NAME, STDIN_SOURCE,
};
use sc_observability_otlp::api::logs::{AnyValue, LogRecord as _, Severity};
use sc_observability_otlp::api::trace::{
    SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
};
use sc_observability_otlp::api::{InstrumentationScope, KeyValue, Value};
use sc_observability_otlp::constants::MAX_INPUT_BYTES;
use sc_observability_otlp::error_codes::sync as codes;
use sc_observability_otlp::sdk::Resource;
use sc_observability_otlp::sdk::trace::{SpanData, SpanEvents, SpanLinks};
use sc_observability_otlp::sync::{Client, SyncError, check_input_limits};
use serde_json::Value as Json;
use std::{
    hash::{BuildHasher, RandomState},
    io::Read,
    time::{Duration, SystemTime},
};

/// Runs the parsed command; the error decides the process exit.
pub(crate) fn run(cli: &Cli) -> Result<(), SyncError> {
    let mut client = client(cli)?;
    let mut resource = Resource::builder();
    if let Some(service) = &cli.service {
        resource = resource.with_service_name(service.clone());
    }
    let resource = resource.build();
    let scope = InstrumentationScope::builder(SCOPE_NAME)
        .with_version(env!("CARGO_PKG_VERSION"))
        .build();
    match &cli.command {
        Command::Log(args) => send_log(&mut client, &resource, scope, args),
        Command::Span(args) => client.send_span(&resource, span(args, scope)?),
        Command::Metric(args) => send_metric(&mut client, &resource, scope, args),
    }
}

/// Validation failures export nothing; export failures come from the exporter.
pub(crate) const fn exit_code(error: &SyncError) -> u8 {
    match error {
        SyncError::Validation { .. } => EXIT_VALIDATION,
        SyncError::Export(_) => EXIT_EXPORT,
    }
}

fn client(cli: &Cli) -> Result<Client, SyncError> {
    let endpoint = match &cli.endpoint {
        Some(endpoint) => endpoint.clone(),
        None => std::env::var(ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned()),
    };
    let mut client = Client::new(&endpoint)?;
    for (name, value) in &cli.headers {
        client = client.with_header(name, value)?;
    }
    if let Some(timeout) = cli.timeout {
        client = client.with_timeout(timeout)?;
    }
    if let Some(path) = &cli.root_certificate {
        let pem = std::fs::read(path).map_err(|error| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("cannot read root certificate {}: {error}", path.display()),
            )
        })?;
        client = client.with_root_certificate_pem(&pem)?;
    }
    Ok(client)
}

fn send_log(
    client: &mut Client,
    resource: &Resource,
    scope: InstrumentationScope,
    args: &LogArgs,
) -> Result<(), SyncError> {
    let attributes = attributes(args.attributes.attributes.as_deref())?;
    let (severity, text) = match args.severity {
        LogSeverity::Trace => (Severity::Trace, "TRACE"),
        LogSeverity::Debug => (Severity::Debug, "DEBUG"),
        LogSeverity::Info => (Severity::Info, "INFO"),
        LogSeverity::Warn => (Severity::Warn, "WARN"),
        LogSeverity::Error => (Severity::Error, "ERROR"),
        LogSeverity::Fatal => (Severity::Fatal, "FATAL"),
    };
    client.send_log(resource, scope, |record| {
        record.set_timestamp(SystemTime::now());
        record.set_severity_number(severity);
        record.set_severity_text(text);
        record.set_body(AnyValue::from(args.body.clone()));
        if let (Some(trace_id), Some(span_id)) = (args.trace_id, args.span_id) {
            record.set_trace_context(trace_id, span_id, None);
        }
        for (key, value) in attributes {
            record.add_attribute(key.clone(), scalar::<AnyValue>(&key, value)?);
        }
        Ok(())
    })
}

fn span(args: &SpanArgs, scope: InstrumentationScope) -> Result<SpanData, SyncError> {
    let end_time = args
        .end_time_unix_nano
        .map_or_else(SystemTime::now, unix_nanos);
    let status = match (&args.error, args.ok) {
        (Some(description), _) => Status::error(description.clone()),
        (None, true) => Status::Ok,
        (None, false) => Status::Unset,
    };
    Ok(SpanData {
        span_context: SpanContext::new(
            args.trace_id.unwrap_or_else(|| {
                TraceId::from(u128::from(random()) << 64 | u128::from(random()))
            }),
            args.span_id.unwrap_or_else(|| SpanId::from(random())),
            TraceFlags::SAMPLED,
            false,
            TraceState::NONE,
        ),
        parent_span_id: args.parent_span_id.unwrap_or(SpanId::INVALID),
        parent_span_is_remote: args.parent_span_id.is_some(),
        span_kind: match args.kind {
            Kind::Internal => SpanKind::Internal,
            Kind::Server => SpanKind::Server,
            Kind::Client => SpanKind::Client,
            Kind::Producer => SpanKind::Producer,
            Kind::Consumer => SpanKind::Consumer,
        },
        name: args.name.clone().into(),
        start_time: args.start_time_unix_nano.map_or(end_time, unix_nanos),
        end_time,
        attributes: key_values(attributes(args.attributes.attributes.as_deref())?)?,
        dropped_attributes_count: 0,
        events: SpanEvents::default(),
        links: SpanLinks::default(),
        status,
        instrumentation_scope: scope,
    })
}

fn send_metric(
    client: &mut Client,
    resource: &Resource,
    scope: InstrumentationScope,
    args: &MetricArgs,
) -> Result<(), SyncError> {
    let attributes = key_values(attributes(args.attributes.attributes.as_deref())?)?;
    client.send_metrics(resource, scope, |meter| {
        macro_rules! instrument {
            ($builder:expr) => {{
                let mut builder = $builder;
                if let Some(unit) = &args.unit {
                    builder = builder.with_unit(unit.clone());
                }
                if let Some(description) = &args.description {
                    builder = builder.with_description(description.clone());
                }
                builder.build()
            }};
        }
        let name = args.name.clone();
        match args.kind {
            MetricKind::Counter => {
                instrument!(meter.f64_counter(name)).add(args.value, &attributes);
            }
            MetricKind::UpDownCounter => {
                instrument!(meter.f64_up_down_counter(name)).add(args.value, &attributes);
            }
            MetricKind::Gauge => {
                instrument!(meter.f64_gauge(name)).record(args.value, &attributes);
            }
            MetricKind::Histogram => {
                instrument!(meter.f64_histogram(name)).record(args.value, &attributes);
            }
        }
        Ok(())
    })
}

/// Reads `--attributes` as one capped JSON object.
fn attributes(source: Option<&str>) -> Result<Vec<(String, Json)>, SyncError> {
    let Some(source) = source else {
        return Ok(Vec::new());
    };
    let text = if source == STDIN_SOURCE {
        read_capped(std::io::stdin().lock(), "standard input")?
    } else if let Some(path) = source.strip_prefix('@') {
        let file = std::fs::File::open(path)
            .map_err(|error| invalid(format!("cannot read attributes file {path}: {error}")))?;
        read_capped(file, path)?
    } else {
        source.to_owned()
    };
    check_input_limits(text.len(), 0)?;
    let object: serde_json::Map<String, Json> = serde_json::from_str(&text)
        .map_err(|error| invalid(format!("attributes must be one JSON object: {error}")))?;
    check_input_limits(text.len(), object.len())?;
    Ok(object.into_iter().collect())
}

/// Reads at most one byte past the input limit, so oversized input is
/// rejected without being buffered.
fn read_capped(reader: impl Read, name: &str) -> Result<String, SyncError> {
    let cap = u64::try_from(MAX_INPUT_BYTES).map_or(u64::MAX, |limit| limit.saturating_add(1));
    let mut bytes = Vec::new();
    reader
        .take(cap)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(format!("cannot read {name}: {error}")))?;
    check_input_limits(bytes.len(), 0)?;
    String::from_utf8(bytes).map_err(|_| invalid(format!("{name} is not UTF-8")))
}

fn key_values(attributes: Vec<(String, Json)>) -> Result<Vec<KeyValue>, SyncError> {
    attributes
        .into_iter()
        .map(|(key, value)| {
            let value = scalar::<Value>(&key, value)?;
            Ok(KeyValue::new(key, value))
        })
        .collect()
}

/// Converts one JSON attribute value; nested and null values are rejected
/// rather than flattened.
fn scalar<T>(key: &str, value: Json) -> Result<T, SyncError>
where
    T: From<bool> + From<i64> + From<f64> + From<String>,
{
    match value {
        Json::Bool(value) => Ok(T::from(value)),
        Json::Number(number) => number
            .as_i64()
            .map(T::from)
            .or_else(|| number.as_f64().map(T::from))
            .ok_or_else(|| invalid(format!("attribute {key}: unsupported number {number}"))),
        Json::String(value) => Ok(T::from(value)),
        Json::Null | Json::Array(_) | Json::Object(_) => Err(invalid(format!(
            "attribute {key}: only string, boolean and number values are supported"
        ))),
    }
}

fn invalid(message: String) -> SyncError {
    SyncError::validation(codes::INVALID_RECORD, message)
}

fn unix_nanos(nanos: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos)
}

/// A non-zero id from the standard library's randomly keyed hasher; each
/// `RandomState` carries distinct keys.
fn random() -> u64 {
    RandomState::new().hash_one(SystemTime::now()).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{EXIT_EXPORT, EXIT_VALIDATION};
    use sc_observability_otlp::constants::MAX_BATCH_RECORDS;
    use sc_observability_otlp::sdk::error::OTelSdkError;

    fn code(error: &SyncError) -> &'static str {
        match error {
            SyncError::Validation { code, .. } => code,
            SyncError::Export(_) => "export",
        }
    }

    #[test]
    fn validation_and_export_failures_have_distinct_nonzero_exits() {
        let validation = SyncError::validation(codes::INVALID_RECORD, "bad");
        let export = SyncError::Export(OTelSdkError::InternalFailure("down".into()));
        assert_eq!(exit_code(&validation), EXIT_VALIDATION);
        assert_eq!(exit_code(&export), EXIT_EXPORT);
    }

    #[test]
    fn attributes_map_json_scalars_to_native_values() {
        let parsed = key_values(
            attributes(Some(
                r#"{"s":"text","b":true,"i":-3,"f":1.5,"big":18446744073709551615}"#,
            ))
            .expect("valid object"),
        )
        .expect("scalars");
        let find = |key: &str| {
            parsed
                .iter()
                .find(|kv| kv.key.as_str() == key)
                .map(|kv| kv.value.clone())
        };
        assert_eq!(find("s"), Some(Value::from("text")));
        assert_eq!(find("b"), Some(Value::Bool(true)));
        assert_eq!(find("i"), Some(Value::I64(-3)));
        assert_eq!(find("f"), Some(Value::F64(1.5)));
        assert_eq!(find("big"), Some(Value::F64(18_446_744_073_709_551_615.0)));
        assert_eq!(
            scalar::<AnyValue>("k", Json::from("v")).expect("string"),
            AnyValue::from("v".to_owned())
        );
    }

    #[test]
    fn unsupported_attributes_are_validation_errors() {
        for source in [
            "[1]",
            "not json",
            r#"{"a":null}"#,
            r#"{"a":[1]}"#,
            r#"{"a":{"b":1}}"#,
        ] {
            let error = attributes(Some(source))
                .and_then(key_values)
                .expect_err(source);
            assert_eq!(code(&error), codes::INVALID_RECORD, "{source}");
        }
        let error =
            attributes(Some("@/nonexistent/sc-otel-attributes.json")).expect_err("missing file");
        assert_eq!(code(&error), codes::INVALID_RECORD);
    }

    #[test]
    fn structured_input_is_byte_and_record_capped() {
        let at_limit = std::io::repeat(b' ').take(MAX_INPUT_BYTES as u64);
        assert_eq!(
            read_capped(at_limit, "input").expect("at limit").len(),
            MAX_INPUT_BYTES
        );
        let above = std::io::repeat(b' ').take(10 * MAX_INPUT_BYTES as u64);
        let error = read_capped(above, "input").expect_err("above limit");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED);
        let inline = format!("{{\"a\":\"{}\"}}", "x".repeat(MAX_INPUT_BYTES));
        let error = attributes(Some(&inline)).expect_err("inline above limit");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED);
        let many = (0..=MAX_BATCH_RECORDS)
            .map(|index| format!("\"k{index}\":1"))
            .collect::<Vec<_>>()
            .join(",");
        let error = attributes(Some(&format!("{{{many}}}"))).expect_err("too many attributes");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED);
    }

    #[test]
    fn random_ids_are_valid_and_distinct() {
        let ids = (0..64)
            .map(|_| random())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), 64);
        assert!(!ids.contains(&0));
    }
}
