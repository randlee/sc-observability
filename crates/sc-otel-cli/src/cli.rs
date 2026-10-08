//! Command-line parsing for the `sc-otel` executable.

use clap::{Args, Parser, Subcommand, ValueEnum};
use sc_observability_otlp::api::trace::{SpanId, TraceId};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(
    name = "sc-otel",
    version,
    about = "Send OpenTelemetry logs, spans and metrics over OTLP/HTTP",
    long_about = "Send one log record, one completed span or one metric measurement to an OTLP/HTTP collector with the official OpenTelemetry exporter. Each command exports before it exits and reports the exporter's result; nothing is stored or retried after the process ends.",
    after_long_help = include_str!("manual.txt")
)]
pub(crate) struct Cli {
    /// OTLP/HTTP base URL; /v1/logs, /v1/traces or /v1/metrics is appended. Defaults to `OTEL_EXPORTER_OTLP_ENDPOINT`, then <http://localhost:4318>.
    #[arg(long, global = true, value_name = "URL")]
    pub(crate) endpoint: Option<String>,
    /// Send a request header as NAME=VALUE; repeat for more. Replaces a header of the same name from `OTEL_EXPORTER_OTLP_HEADERS`. Prefer that variable for credentials: command lines are visible to other processes.
    #[arg(long = "header", global = true, value_name = "NAME=VALUE", value_parser = parse_header)]
    pub(crate) headers: Vec<(String, String)>,
    /// Bound connecting, each request and the exporter's retries, in whole seconds greater than zero. Defaults to 3 seconds.
    #[arg(long, global = true, value_name = "SECONDS", value_parser = parse_seconds)]
    pub(crate) timeout: Option<Duration>,
    /// Trust the CA certificates in this PEM file in addition to the system roots.
    #[arg(long, global = true, value_name = "FILE")]
    pub(crate) root_certificate: Option<PathBuf>,
    /// Resource service.name. Defaults to `OTEL_SERVICE_NAME`, then the SDK default `unknown_service`. `OTEL_RESOURCE_ATTRIBUTES` adds further resource attributes.
    #[arg(long, global = true, value_name = "NAME")]
    pub(crate) service: Option<String>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Export one log record.
    #[command(
        after_long_help = "Examples:\n  sc-otel log --body 'job started'\n  sc-otel log --severity error --body 'job failed' --attributes '{\"job\":\"build\",\"attempt\":2}'\n  sc-otel log --body done --trace-id 4bf92f3577b34da6a3ce929d0e0e4736 --span-id 00f067aa0ba902b7"
    )]
    Log(LogArgs),
    /// Export one completed span.
    #[command(
        after_long_help = "Examples:\n  sc-otel span --name build --start-time-unix-nano 1700000000000000000 --end-time-unix-nano 1700000005000000000\n  sc-otel span --name deploy --kind client --error 'rollout timed out' --attributes @span.json\n\nWithout --trace-id and --span-id the span gets new random ids. Pass the ids of a parent span with --trace-id and --parent-span-id to join an existing trace. A parent supplied this way is external to this CLI process and is marked remote."
    )]
    Span(SpanArgs),
    /// Export one metric measurement.
    #[command(
        after_long_help = "Examples:\n  sc-otel metric --name jobs.completed --kind counter --value 1\n  sc-otel metric --name queue.depth --kind gauge --value 42 --attributes '{\"queue\":\"default\"}'\n  sc-otel metric --name job.duration --kind histogram --unit s --value 12.5\n\nCounter and histogram values, including negative and non-finite values, are passed to the OpenTelemetry SDK without CLI range validation. An invalid instrument name records nothing, sends no request, and fails with exit 3."
    )]
    Metric(MetricArgs),
}

#[derive(Debug, Args)]
pub(crate) struct AttributeArgs {
    /// Attributes as one JSON object of string, boolean or number values: inline JSON, @FILE, or - for standard input. At most 1 MiB and 10,000 attributes.
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub(crate) attributes: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct LogArgs {
    /// Log message body.
    #[arg(long)]
    pub(crate) body: String,
    /// Log severity.
    #[arg(long, value_enum, default_value_t = LogSeverity::Info)]
    pub(crate) severity: LogSeverity,
    /// Correlate the record with this trace (32 hex digits). Requires --span-id.
    #[arg(long, value_name = "HEX", value_parser = parse_trace_id, requires = "span_id")]
    pub(crate) trace_id: Option<TraceId>,
    /// Correlate the record with this span (16 hex digits). Requires --trace-id.
    #[arg(long, value_name = "HEX", value_parser = parse_span_id, requires = "trace_id")]
    pub(crate) span_id: Option<SpanId>,
    #[command(flatten)]
    pub(crate) attributes: AttributeArgs,
}

#[derive(Debug, Args)]
pub(crate) struct SpanArgs {
    /// Span name.
    #[arg(long)]
    pub(crate) name: String,
    /// Trace id (32 hex digits). Defaults to a new random id.
    #[arg(long, value_name = "HEX", value_parser = parse_trace_id)]
    pub(crate) trace_id: Option<TraceId>,
    /// Span id (16 hex digits). Defaults to a new random id.
    #[arg(long, value_name = "HEX", value_parser = parse_span_id)]
    pub(crate) span_id: Option<SpanId>,
    /// Remote parent span id (16 hex digits). Requires --trace-id.
    #[arg(long, value_name = "HEX", value_parser = parse_span_id, requires = "trace_id")]
    pub(crate) parent_span_id: Option<SpanId>,
    /// Span kind.
    #[arg(long, value_enum, default_value_t = Kind::Internal)]
    pub(crate) kind: Kind,
    /// Start time in nanoseconds since the Unix epoch. Defaults to the end time.
    #[arg(long, value_name = "NANOS")]
    pub(crate) start_time_unix_nano: Option<u64>,
    /// End time in nanoseconds since the Unix epoch. Defaults to now.
    #[arg(long, value_name = "NANOS")]
    pub(crate) end_time_unix_nano: Option<u64>,
    /// Set the span status to Ok.
    #[arg(long, conflicts_with = "error")]
    pub(crate) ok: bool,
    /// Set the span status to Error with this description.
    #[arg(long, value_name = "DESCRIPTION")]
    pub(crate) error: Option<String>,
    #[command(flatten)]
    pub(crate) attributes: AttributeArgs,
}

#[derive(Debug, Args)]
pub(crate) struct MetricArgs {
    /// Instrument name.
    #[arg(long)]
    pub(crate) name: String,
    /// Instrument kind.
    #[arg(long, value_enum)]
    pub(crate) kind: MetricKind,
    /// Measurement value.
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) value: f64,
    /// Instrument unit, such as s or By.
    #[arg(long)]
    pub(crate) unit: Option<String>,
    /// Instrument description.
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[command(flatten)]
    pub(crate) attributes: AttributeArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LogSeverity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Kind {
    Internal,
    Server,
    Client,
    Producer,
    Consumer,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum MetricKind {
    Counter,
    UpDownCounter,
    Gauge,
    Histogram,
}

fn parse_header(value: &str) -> Result<(String, String), String> {
    match value.split_once('=') {
        Some((name, value)) if !name.is_empty() => Ok((name.to_owned(), value.to_owned())),
        _ => Err("expected NAME=VALUE".into()),
    }
}

fn parse_seconds(value: &str) -> Result<Duration, String> {
    match value.parse::<u64>() {
        Ok(seconds) if seconds > 0 => Ok(Duration::from_secs(seconds)),
        _ => Err("timeout must be a whole number of seconds greater than zero".into()),
    }
}

fn parse_trace_id(value: &str) -> Result<TraceId, String> {
    check_hex(value, 32)?;
    TraceId::from_hex(value).map_err(|error| error.to_string())
}

fn parse_span_id(value: &str) -> Result<SpanId, String> {
    check_hex(value, 16)?;
    SpanId::from_hex(value).map_err(|error| error.to_string())
}

/// The SDK parsers accept short and uppercase ids; OTLP ids are fixed-width lowercase hex.
fn check_hex(value: &str, digits: usize) -> Result<(), String> {
    let lowercase_hex = value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if value.len() == digits && lowercase_hex {
        Ok(())
    } else {
        Err(format!("expected {digits} lowercase hex digits"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, MetricKind};
    use clap::{Parser, error::ErrorKind};

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("sc-otel").chain(args.iter().copied()))
    }

    #[test]
    fn global_options_parse_on_either_side_of_the_command() {
        let cli = parse(&[
            "--endpoint",
            "https://collector:4318",
            "log",
            "--body",
            "hi",
            "--header",
            "authorization=Bearer a=b",
            "--timeout",
            "9",
        ])
        .expect("valid log command");
        assert_eq!(cli.endpoint.as_deref(), Some("https://collector:4318"));
        assert_eq!(
            cli.headers,
            [("authorization".to_owned(), "Bearer a=b".to_owned())]
        );
        assert_eq!(cli.timeout, Some(std::time::Duration::from_secs(9)));
        assert!(matches!(cli.command, Command::Log(_)));
    }

    #[test]
    fn metric_accepts_negative_values() {
        let cli = parse(&[
            "metric",
            "--name",
            "delta",
            "--kind",
            "up-down-counter",
            "--value",
            "-2.5",
        ])
        .expect("valid metric command");
        let Command::Metric(metric) = cli.command else {
            panic!("expected metric");
        };
        assert!(matches!(metric.kind, MetricKind::UpDownCounter));
        assert!((metric.value + 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_fields_are_usage_errors() {
        for args in [
            &["log"][..],
            &["log", "--body", "x", "--severity", "loud"],
            &[
                "log",
                "--body",
                "x",
                "--trace-id",
                "4bf92f3577b34da6a3ce929d0e0e4736",
            ],
            &[
                "log",
                "--body",
                "x",
                "--trace-id",
                "4BF92F3577B34DA6A3CE929D0E0E4736",
                "--span-id",
                "00f067aa0ba902b7",
            ],
            &["span", "--name", "x", "--span-id", "f067aa0ba902b7"],
            &[
                "span",
                "--name",
                "x",
                "--parent-span-id",
                "00f067aa0ba902b7",
            ],
            &["span", "--name", "x", "--ok", "--error", "boom"],
            &["span", "--name", "x", "--end-time-unix-nano", "-1"],
            &["metric", "--name", "x", "--kind", "counter"],
            &["metric", "--name", "x", "--kind", "summary", "--value", "1"],
            &[
                "metric", "--name", "x", "--kind", "gauge", "--value", "many",
            ],
            &["--header", "=value", "log", "--body", "x"],
            &["--header", "novalue", "log", "--body", "x"],
            &["--timeout", "0", "log", "--body", "x"],
            &["--store", "queue.sqlite", "log", "--body", "x"],
            &["emit", "--log", "{}"],
            &["flush"],
            &["status"],
        ] {
            let error = parse(args).expect_err(&format!("{args:?} must be rejected"));
            assert!(error.use_stderr(), "{args:?}: {:?}", error.kind());
            assert_ne!(error.kind(), ErrorKind::DisplayHelp, "{args:?}");
        }
    }
}
