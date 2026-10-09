//! Converts parsed arguments to native SDK inputs and sends them with the
//! shared synchronous client.

use crate::cli::{Cli, Command, Kind, LogArgs, LogSeverity, MetricArgs, MetricKind, SpanArgs};
use crate::constants::{SCOPE_NAME, STDIN_SOURCE};
use crate::error_codes::{EXIT_EXPORT, EXIT_VALIDATION};
use sc_observability_otlp::api::logs::{AnyValue, Severity};
use sc_observability_otlp::api::trace::SpanKind;
use sc_observability_otlp::api::{InstrumentationScope, KeyValue, Value};
use sc_observability_otlp::constants::MAX_INPUT_BYTES;
use sc_observability_otlp::error_codes::sync as codes;
use sc_observability_otlp::sdk::Resource;
use sc_observability_otlp::sdk::trace::SpanData;
use sc_observability_otlp::sync::{
    Client, CompletedSpan, InputByteCounter, LogEntry, Measurement, MetricKind as SharedMetricKind,
    SyncError, check_input_limits, read_bounded_regular_file, read_root_certificate,
    resolve_endpoint, span_status, unsigned_attribute,
};
use serde_json::Value as Json;
use serde_json::value::RawValue;
use std::{fmt, io::Read, path::Path};

/// Runs the parsed command; the error decides the process exit.
pub(crate) fn run(cli: Cli) -> Result<(), SyncError> {
    let mut client = client(&cli)?;
    let mut resource = Resource::builder();
    if let Some(service) = cli.service {
        InputByteCounter::new().add_text(&service)?;
        resource = resource.with_service_name(service);
    }
    let resource = resource.build();
    let scope = InstrumentationScope::builder(SCOPE_NAME)
        .with_version(env!("CARGO_PKG_VERSION"))
        .build();
    match cli.command {
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
    args: LogArgs,
) -> Result<(), SyncError> {
    let attributes = attributes(args.attributes.attributes.as_deref(), &[&args.body])?
        .into_iter()
        .map(|(key, value)| {
            let value = scalar::<AnyValue>(&key, value)?;
            Ok((key, value))
        })
        .collect::<Result<_, SyncError>>()?;
    let entry = LogEntry {
        severity: match args.severity {
            LogSeverity::Trace => Severity::Trace,
            LogSeverity::Debug => Severity::Debug,
            LogSeverity::Info => Severity::Info,
            LogSeverity::Warn => Severity::Warn,
            LogSeverity::Error => Severity::Error,
            LogSeverity::Fatal => Severity::Fatal,
        },
        body: args.body,
        trace_context: args.trace_id.zip(args.span_id),
        attributes,
    };
    client.send_log(resource, scope, |record| {
        entry.fill(record);
        Ok(())
    })
}

fn span(args: SpanArgs, scope: InstrumentationScope) -> Result<SpanData, SyncError> {
    let attributes = key_values(attributes(
        args.attributes.attributes.as_deref(),
        &[&args.name, args.error.as_deref().unwrap_or_default()],
    )?)?;
    CompletedSpan {
        name: args.name,
        trace_id: args.trace_id,
        span_id: args.span_id,
        parent_span_id: args.parent_span_id,
        kind: match args.kind {
            Kind::Internal => SpanKind::Internal,
            Kind::Server => SpanKind::Server,
            Kind::Client => SpanKind::Client,
            Kind::Producer => SpanKind::Producer,
            Kind::Consumer => SpanKind::Consumer,
        },
        start_time_unix_nano: args.start_time_unix_nano,
        end_time_unix_nano: args.end_time_unix_nano,
        // clap rejects --ok together with --error, so span_status never sees both.
        status: span_status(args.error, args.ok),
        attributes,
    }
    .into_span_data(scope)
}

fn send_metric(
    client: &mut Client,
    resource: &Resource,
    scope: InstrumentationScope,
    args: MetricArgs,
) -> Result<(), SyncError> {
    let attributes = key_values(attributes(
        args.attributes.attributes.as_deref(),
        &[
            &args.name,
            args.unit.as_deref().unwrap_or_default(),
            args.description.as_deref().unwrap_or_default(),
        ],
    )?)?;
    let measurement = Measurement {
        name: args.name,
        kind: match args.kind {
            MetricKind::Counter => SharedMetricKind::Counter,
            MetricKind::UpDownCounter => SharedMetricKind::UpDownCounter,
            MetricKind::Gauge => SharedMetricKind::Gauge,
            MetricKind::Histogram => SharedMetricKind::Histogram,
        },
        value: args.value,
        unit: args.unit,
        description: args.description,
        attributes,
    };
    client.send_metrics(resource, scope, |meter| {
        measurement.record(meter);
        Ok(())
    })
}

/// Reads `--attributes` as one capped JSON object and counts it with the signal's
/// text fields.
fn attributes(
    source: Option<&str>,
    signal_text: &[&str],
) -> Result<Vec<(String, Json)>, SyncError> {
    let mut input = InputByteCounter::new();
    for text in signal_text {
        input.add_text(text)?;
    }
    let Some(source) = source else {
        return Ok(Vec::new());
    };
    let text = if source == STDIN_SOURCE {
        read_capped(std::io::stdin().lock(), "standard input")?
    } else if let Some(path) = source.strip_prefix('@') {
        let bytes =
            read_bounded_regular_file(Path::new(path), "attributes file", codes::INVALID_RECORD)?;
        String::from_utf8(bytes)
            .map_err(|_| invalid(format!("attributes file {path} is not UTF-8")))?
    } else {
        source.to_owned()
    };
    // Raw tokens keep an out-of-range integer literal distinguishable from a
    // float: parsed `Number`s turn it into a lossy `f64`. Entries keep input
    // order and a repeated key stays, as in the Python frontend.
    let ObjectEntries(object) = serde_json::from_str(&text)
        .map_err(|error| invalid(format!("attributes must be one JSON object: {error}")))?;
    check_input_limits(0, object.len())?;
    let mut attributes = Vec::with_capacity(object.len());
    for (key, raw) in object {
        input.add_text(&key)?;
        let token = raw.get().trim();
        if integer_out_of_range(token) {
            return Err(invalid(format!(
                "attribute {key}: integer outside the i64/u64 range"
            )));
        }
        let value: Json = serde_json::from_str(token)
            .map_err(|error| invalid(format!("attributes must be one JSON object: {error}")))?;
        if let Json::String(value) = &value {
            input.add_text(value)?;
        }
        attributes.push((key, value));
    }
    Ok(attributes)
}

/// One JSON object as its entries in input order, repeated keys included.
struct ObjectEntries(Vec<(String, Box<RawValue>)>);

impl<'de> serde::Deserialize<'de> for ObjectEntries {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ObjectEntries;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(ObjectEntries(entries))
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

/// Whether `token` is an integer literal that fits neither `i64` nor `u64`.
fn integer_out_of_range(token: &str) -> bool {
    let digits = token.strip_prefix('-').unwrap_or(token);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && token.parse::<i64>().is_err()
        && token.parse::<u64>().is_err()
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
            .or_else(|| number.as_u64().map(unsigned_attribute::<T>))
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
    use sc_observability_otlp::api::trace::TraceFlags;

    fn client_with_root_certificate(path: &std::path::Path) -> Result<Client, SyncError> {
        use clap::Parser;

        let path = path.to_str().expect("UTF-8 temporary certificate path");
        let cli = Cli::try_parse_from([
            "sc-otel",
            "--root-certificate",
            path,
            "log",
            "--body",
            "test",
        ])
        .expect("valid CLI arguments");
        client(&cli)
    }
    use crate::error_codes::{EXIT_EXPORT, EXIT_VALIDATION};
    use sc_observability_otlp::constants::MAX_BATCH_RECORDS;
    use sc_observability_otlp::sdk::error::OTelSdkError;

    fn code(error: &SyncError) -> String {
        match error {
            SyncError::Validation { code, .. } => code.to_string(),
            SyncError::Export(_) => "export".to_owned(),
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
            ), &[])
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
        assert_eq!(find("big"), Some(Value::from("18446744073709551615")));
        assert_eq!(find("duplicate"), Some(Value::from("first")));
        assert_eq!(
            scalar::<AnyValue>("k", Json::from("v")).expect("string"),
            AnyValue::from("v".to_owned())
        );
    }

    #[test]
    fn attributes_keep_input_order_and_repeated_keys() {
        let parsed = attributes(Some(r#"{"b":1,"a":2,"b":3}"#), &[]).expect("valid object");
        let keys: Vec<(&str, &Json)> = parsed.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(
            keys,
            [
                ("b", &Json::from(1)),
                ("a", &Json::from(2)),
                ("b", &Json::from(3))
            ]
        );
    }

    #[test]
    fn integer_literals_outside_the_64_bit_ranges_are_rejected() {
        for source in [
            r#"{"k":18446744073709551616}"#,
            r#"{"k":-9223372036854775809}"#,
        ] {
            let error = attributes(Some(source), &[]).expect_err(source);
            assert_eq!(code(&error), codes::INVALID_RECORD.as_str(), "{source}");
            assert!(
                error
                    .to_string()
                    .contains("attribute k: integer outside the i64/u64 range"),
                "{source}: {error}"
            );
        }
        let parsed = key_values(
            attributes(
                Some(r#"{"f":1.8446744073709552e19,"max":18446744073709551615,"min":-9223372036854775808}"#),
                &[],
            )
            .expect("in-range and float values"),
        )
        .expect("scalars");
        let find = |key: &str| {
            parsed
                .iter()
                .find(|kv| kv.key.as_str() == key)
                .map(|kv| kv.value.clone())
        };
        assert_eq!(find("f"), Some(Value::F64(1.844_674_407_370_955_2e19)));
        assert_eq!(find("max"), Some(Value::from("18446744073709551615")));
        assert_eq!(find("min"), Some(Value::I64(i64::MIN)));
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
            let error = attributes(Some(source), &[])
                .and_then(key_values)
                .expect_err(source);
            assert_eq!(code(&error), codes::INVALID_RECORD.as_str(), "{source}");
        }
        let error = attributes(Some("@/nonexistent/sc-otel-attributes.json"), &[])
            .expect_err("missing file");
        assert_eq!(code(&error), codes::INVALID_RECORD.as_str());
    }

    #[test]
    fn structured_input_is_byte_and_record_capped() {
        use clap::Parser;

        let at_limit = std::io::repeat(b' ').take(MAX_INPUT_BYTES as u64);
        assert_eq!(
            read_capped(at_limit, "input").expect("at limit").len(),
            MAX_INPUT_BYTES
        );
        let above = std::io::repeat(b' ').take(10 * MAX_INPUT_BYTES as u64);
        let error = read_capped(above, "input").expect_err("above limit");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let error =
            attributes(None, &[&"x".repeat(MAX_INPUT_BYTES + 1)]).expect_err("oversized log body");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let cli = Cli::try_parse_from([
            "sc-otel",
            "span",
            "--name",
            &"x".repeat(MAX_INPUT_BYTES + 1),
        ])
        .expect("span arguments parse");
        let Command::Span(args) = cli.command else {
            panic!("expected span command");
        };
        let error = span(args, InstrumentationScope::builder("test").build())
            .expect_err("oversized span name");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let cli = Cli::try_parse_from([
            "sc-otel",
            "span",
            "--name",
            "s",
            "--error",
            &"x".repeat(MAX_INPUT_BYTES + 1),
        ])
        .expect("span arguments parse");
        let Command::Span(args) = cli.command else {
            panic!("expected span command");
        };
        let error = span(args, InstrumentationScope::builder("test").build())
            .expect_err("oversized span error text");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let half = "x".repeat(MAX_INPUT_BYTES / 2 + 1);
        let error =
            attributes(None, &[&half, &half, &half]).expect_err("metric text fields add up");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let cli = Cli::try_parse_from([
            "sc-otel",
            "--service",
            &"x".repeat(MAX_INPUT_BYTES + 1),
            "log",
            "--body",
            "b",
        ])
        .expect("log arguments parse");
        let error = run(cli).expect_err("oversized service name");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let inline = format!("{{\"a\":\"{}\"}}", "x".repeat(MAX_INPUT_BYTES));
        let error = attributes(Some(&inline), &[]).expect_err("inline above limit");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let key = "k".repeat(MAX_INPUT_BYTES + 1);
        let error = attributes(Some(&format!("{{\"{key}\":true}}")), &[])
            .expect_err("oversized attribute key");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
        let many = (0..=MAX_BATCH_RECORDS)
            .map(|index| format!("\"k{index}\":1"))
            .collect::<Vec<_>>()
            .join(",");
        let error = attributes(Some(&format!("{{{many}}}")), &[]).expect_err("too many attributes");
        assert_eq!(code(&error), codes::INPUT_LIMIT_EXCEEDED.as_str());
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
            span(args, InstrumentationScope::builder("test").build()).expect("span data")
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
        assert!(!child.parent_span_is_remote);
        assert_eq!(child.span_context.trace_flags(), TraceFlags::SAMPLED);
    }

    #[test]
    fn root_certificate_file_is_byte_capped() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("roots.pem");
        std::fs::write(&path, vec![b'x'; MAX_INPUT_BYTES + 1])
            .expect("write oversized root certificate");

        let error = client_with_root_certificate(&path).expect_err("oversized PEM");
        assert_eq!(code(&error), codes::INVALID_CONFIG.as_str());
    }
}
