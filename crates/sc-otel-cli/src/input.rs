//! Reads stdin and fragment arguments through the shared envelope parser.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed input diagnostics"
)]

use crate::cli::InputArgs;
use sc_observability_types::otlp::submission::{
    SubmissionEnvelope, SystemIds, TelemetryClientError,
};
use serde_json::{Map, Value, json};
use std::io::Read;

pub(crate) fn envelope(
    args: &InputArgs,
    record_key: Option<&str>,
) -> Result<SubmissionEnvelope, TelemetryClientError> {
    let json = if args.stdin {
        read_stdin()?
    } else {
        fragments(args, record_key)?
    };
    SubmissionEnvelope::from_json(&json, &mut SystemIds::new()).map_err(Into::into)
}

fn read_stdin() -> Result<String, TelemetryClientError> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| invalid_input())?;
    Ok(input)
}

fn fragments(args: &InputArgs, record_key: Option<&str>) -> Result<String, TelemetryClientError> {
    let mut input = Map::new();
    input.insert("version".into(), json!(1));
    input.insert("logs".into(), values(&args.log)?);
    input.insert("spans".into(), values(&args.span)?);
    input.insert("metrics".into(), values(&args.metric)?);
    if let Some(profile) = &args.profile {
        input.insert("profiles".into(), fragment(profile)?);
    }
    if let Some(key) = record_key {
        input.insert("record_key".into(), Value::String(key.into()));
    }
    Ok(Value::Object(input).to_string())
}

fn values(items: &[String]) -> Result<Value, TelemetryClientError> {
    items
        .iter()
        .map(|item| fragment(item))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn fragment(value: &str) -> Result<Value, TelemetryClientError> {
    let text = value
        .strip_prefix('@')
        .map_or_else(|| Ok(value.to_owned()), std::fs::read_to_string)
        .map_err(|_| invalid_input())?;
    serde_json::from_str(&text).map_err(|_| invalid_input())
}

fn invalid_input() -> TelemetryClientError {
    SubmissionEnvelope::from_json("{", &mut SystemIds::new())
        .expect_err("a deliberately truncated JSON document is invalid")
        .into()
}
