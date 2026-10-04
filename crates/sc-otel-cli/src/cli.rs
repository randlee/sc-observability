//! Command-line parsing for the `sc-otel` executable.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(
    name = "sc-otel",
    version,
    about = "Submit and inspect durable telemetry"
)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub(crate) store: Option<PathBuf>,
    #[arg(long, global = true)]
    pub(crate) endpoint: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub(crate) output: OutputFormat,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Emit(EmitArgs),
    Validate(InputArgs),
    Flush(FlushArgs),
    Status(StatusArgs),
}

#[derive(Debug, Args)]
#[group(id = "source", required = true, multiple = true)]
pub(crate) struct InputArgs {
    #[arg(long, group = "source", conflicts_with_all = ["log", "span", "metric", "profile"])]
    pub(crate) stdin: bool,
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) log: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) span: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) metric: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) profile: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct EmitArgs {
    #[command(flatten)]
    pub(crate) input: InputArgs,
    #[arg(long, conflicts_with = "stdin")]
    pub(crate) record_key: Option<String>,
    #[arg(long)]
    pub(crate) no_flush: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FlushArgs {
    #[arg(long, value_name = "SECONDS", value_parser = parse_seconds)]
    pub(crate) timeout: Option<Duration>,
}

#[derive(Debug, Args)]
pub(crate) struct StatusArgs {
    #[arg(long, conflicts_with = "record_key")]
    pub(crate) submission: Vec<String>,
    #[arg(long)]
    pub(crate) record_key: Vec<String>,
}

fn parse_seconds(value: &str) -> Result<Duration, String> {
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|_| "timeout must be an unsigned number of seconds".into())
}
