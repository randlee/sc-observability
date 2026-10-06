//! Reads stdin and fragment arguments through the shared envelope parser.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed input diagnostics"
)]

use crate::{
    cli::InputArgs,
    error::{CliError, InputError},
};
use sc_observability_types::otlp::submission::{
    SubmissionEnvelope, SystemIds, TelemetryClientError,
};
use serde_json::{Map, Value, json};
use std::io::Read;

pub(crate) fn envelope(
    args: &InputArgs,
    record_key: Option<&str>,
) -> Result<SubmissionEnvelope, CliError> {
    let json = if args.stdin {
        #[cfg(test)]
        {
            override_or_read_stdin(args.stdin_input.clone(), read_stdin)?
        }
        #[cfg(not(test))]
        {
            read_stdin()?
        }
    } else {
        fragments(args, record_key)?
    };
    SubmissionEnvelope::from_json(&json, &mut SystemIds::new())
        .map_err(|error| CliError::from(TelemetryClientError::from(error)))
}

#[cfg(test)]
fn override_or_read_stdin(
    stdin_input: Option<String>,
    read: impl FnOnce() -> Result<String, CliError>,
) -> Result<String, CliError> {
    match stdin_input {
        Some(input) => Ok(input),
        None => read(),
    }
}

#[cfg(test)]
#[test]
fn stdin_override_does_not_read_process_stdin() {
    let input = override_or_read_stdin(Some("{}".into()), || {
        panic!("stdin reader must not run when an override is present")
    })
    .expect("override is returned");

    assert_eq!(input, "{}");
}

fn read_stdin() -> Result<String, CliError> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|source| InputError::Stdin { source })?;
    Ok(input)
}

fn fragments(args: &InputArgs, record_key: Option<&str>) -> Result<String, CliError> {
    let mut input = Map::new();
    input.insert("version".into(), json!(1));
    input.insert("logs".into(), values(&args.log, "--log")?);
    input.insert("spans".into(), values(&args.span, "--span")?);
    input.insert("metrics".into(), values(&args.metric, "--metric")?);
    if let Some(profile) = &args.profile {
        input.insert("profiles".into(), fragment(profile, "--profile")?);
    }
    if let Some(key) = record_key {
        input.insert("record_key".into(), Value::String(key.into()));
    }
    Ok(Value::Object(input).to_string())
}

fn values(items: &[String], flag: &'static str) -> Result<Value, CliError> {
    items
        .iter()
        .map(|item| fragment(item, flag))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn fragment(value: &str, flag: &'static str) -> Result<Value, CliError> {
    let text = match value.strip_prefix('@') {
        Some(file) => {
            let path = std::path::PathBuf::from(file);
            std::fs::read_to_string(&path).map_err(|source| InputError::File { path, source })?
        }
        None => value.to_owned(),
    };
    serde_json::from_str(&text).map_err(|source| InputError::Fragment { flag, source }.into())
}
