//! Converts parsed arguments to native SDK inputs and sends them with the
//! shared synchronous client.

use crate::cli::{Cli, Command, Kind, LogArgs, LogSeverity, MetricArgs, MetricKind, SpanArgs};
use crate::constants::{MAX_ROOT_CERTIFICATE_BYTES, SCOPE_NAME, STDIN_SOURCE};
use crate::error_codes::{EXIT_EXPORT, EXIT_VALIDATION};
use sc_observability_otlp::api::logs::{AnyValue, LogRecord as _, Severity};
use sc_observability_otlp::api::trace::{
    SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceState,
};
use sc_observability_otlp::api::{InstrumentationScope, KeyValue, Value};
use sc_observability_otlp::constants::MAX_INPUT_BYTES;
use sc_observability_otlp::error_codes::sync as codes;
use sc_observability_otlp::sdk::Resource;
use sc_observability_otlp::sdk::trace::{
    IdGenerator, RandomIdGenerator, SpanData, SpanEvents, SpanLinks,
};
use sc_observability_otlp::sync::{
    Client, SyncError, check_input_limits, resolve_endpoint, span_times,
};
use serde_json::Value as Json;
use std::{fs::File, io::Read, path::Path, time::SystemTime};

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
    let endpoint = resolve_endpoint(cli.endpoint.as_deref())?;
    let mut client = Client::new(&endpoint)?;
    for header in &cli.headers {
        let (name, value) = header
            .split_once('=')
            .filter(|(name, _)| !name.is_empty())
            .ok_or_else(|| {
                SyncError::validation(codes::INVALID_CONFIG, "header must use NAME=VALUE")
            })?;
        client = client.with_header(name, value)?;
    }
    if let Some(timeout) = cli.timeout {
        client = client.with_timeout(timeout)?;
    }
    if let Some(path) = &cli.root_certificate {
        let pem = read_root_certificate(path)?;
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
    let severity = match args.severity {
        LogSeverity::Trace => Severity::Trace,
        LogSeverity::Debug => Severity::Debug,
        LogSeverity::Info => Severity::Info,
        LogSeverity::Warn => Severity::Warn,
        LogSeverity::Error => Severity::Error,
        LogSeverity::Fatal => Severity::Fatal,
    };
    client.send_log(resource, scope, |record| {
        record.set_timestamp(SystemTime::now());
        record.set_severity_number(severity);
        record.set_severity_text(severity.name());
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
    let id_generator = RandomIdGenerator::default();
    let (start_time, end_time) = span_times(args.start_time_unix_nano, args.end_time_unix_nano)?;
    let status = match (&args.error, args.ok) {
        (Some(description), _) => Status::error(description.clone()),
        (None, true) => Status::Ok,
        (None, false) => Status::Unset,
    };
    Ok(SpanData {
        span_context: SpanContext::new(
            args.trace_id.unwrap_or_else(|| id_generator.new_trace_id()),
            args.span_id.unwrap_or_else(|| id_generator.new_span_id()),
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
        start_time,
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
    check_attribute_input_limits(text.len(), 0)?;
    let object: serde_json::Map<String, Json> = serde_json::from_str(&text)
        .map_err(|error| invalid(format!("attributes must be one JSON object: {error}")))?;
    check_attribute_input_limits(text.len(), object.len())?;
    Ok(object.into_iter().collect())
}

/// Applies the shared input bounds to CLI attribute input.
fn check_attribute_input_limits(input_bytes: usize, attributes: usize) -> Result<(), SyncError> {
    check_input_limits(input_bytes, attributes)
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
    check_attribute_input_limits(bytes.len(), 0)?;
    String::from_utf8(bytes).map_err(|_| invalid(format!("{name} is not UTF-8")))
}

/// Reads a custom certificate only from regular files and within its PEM limit.
fn read_root_certificate(path: &Path) -> Result<Vec<u8>, SyncError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        SyncError::validation(
            codes::INVALID_CONFIG,
            format!("cannot read root certificate {}: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(SyncError::validation(
            codes::INVALID_CONFIG,
            format!("root certificate {} must be a regular file", path.display()),
        ));
    }
    let file = File::open(path).map_err(|error| {
        SyncError::validation(
            codes::INVALID_CONFIG,
            format!("cannot read root certificate {}: {error}", path.display()),
        )
    })?;
    read_capped_bytes(
        file,
        &format!("root certificate {}", path.display()),
        MAX_ROOT_CERTIFICATE_BYTES,
        codes::INVALID_CONFIG,
    )
}

/// Reads at most one byte beyond `limit`, rejecting excess without buffering it.
fn read_capped_bytes(
    reader: impl Read,
    name: &str,
    limit: usize,
    read_error_code: &'static str,
) -> Result<Vec<u8>, SyncError> {
    let cap = u64::try_from(limit).map_or(u64::MAX, |limit| limit.saturating_add(1));
    let mut bytes = Vec::new();
    reader.take(cap).read_to_end(&mut bytes).map_err(|error| {
        SyncError::validation(read_error_code, format!("cannot read {name}: {error}"))
    })?;
    if bytes.len() > limit {
        return Err(SyncError::validation(
            codes::INPUT_LIMIT_EXCEEDED,
            format!("{name} exceeds the {limit}-byte limit"),
        ));
    }
    Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_codes::{EXIT_EXPORT, EXIT_VALIDATION};
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
                r#"{"s":"text","b":true,"i":-3,"f":1.5,"big":18446744073709551615,"duplicate":"first","duplicate":"last"}"#,
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
        assert_eq!(find("duplicate"), Some(Value::from("last")));
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
    fn sdk_generated_ids_and_parent_context_have_the_expected_flags() {
        use clap::Parser;

        let parse = |args: &[&str]| {
            let cli = Cli::try_parse_from(std::iter::once("sc-otel").chain(args.iter().copied()))
                .expect("valid span arguments");
            let Command::Span(args) = cli.command else {
                panic!("expected span arguments");
            };
            span(&args, InstrumentationScope::builder("test").build()).expect("span data")
        };
        let first = parse(&["span", "--name", "first"]);
        let second = parse(&["span", "--name", "second"]);
        assert!(first.span_context.is_valid());
        assert_ne!(
            first.span_context.trace_id(),
            second.span_context.trace_id()
        );
        assert_ne!(first.span_context.span_id(), second.span_context.span_id());
        assert_eq!(first.span_context.trace_flags(), TraceFlags::SAMPLED);
        assert!(!first.parent_span_is_remote);

        let child = parse(&[
            "span",
            "--name",
            "child",
            "--trace-id",
            "4bf92f3577b34da6a3ce929d0e0e4736",
            "--parent-span-id",
            "00f067aa0ba902b7",
        ]);
        assert!(child.parent_span_is_remote);
        assert_eq!(child.span_context.trace_flags(), TraceFlags::SAMPLED);
    }

    #[test]
    fn root_certificate_file_is_byte_capped() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("roots.pem");
        std::fs::write(&path, vec![b'x'; MAX_ROOT_CERTIFICATE_BYTES + 1])
            .expect("write oversized root certificate");

        let error = read_root_certificate(&path).expect_err("oversized PEM");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED);
    }

    #[cfg(unix)]
    #[test]
    fn root_certificate_rejects_writerless_fifo_without_blocking() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("roots.pem");
        let status = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("create FIFO");
        assert!(status.success(), "mkfifo exited with {status}");

        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(read_root_certificate(&path));
        });
        let result = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("reader blocked on writer-less FIFO");
        let error = result.expect_err("FIFO is not a regular certificate file");
        assert_eq!(code(&error), codes::INVALID_CONFIG);
    }
}
