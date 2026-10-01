//! Executes parsed commands through the shared telemetry client contract.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed client diagnostics"
)]

use crate::{
    cli::{Cli, Command, EmitArgs, FlushArgs, StatusArgs},
    client, config, exit, input,
    output::{self, Outcome},
};
use sc_observability_types::otlp::submission::{
    RecordKey, StatusQuery, SubmissionId, TelemetryClient, TelemetryClientError,
};
use std::{str::FromStr, time::Duration};

pub(crate) fn run(cli: &Cli) -> u8 {
    let format = cli.output;
    let outcome = match &cli.command {
        Command::Validate(args) => validate(args),
        Command::Emit(args) => emit(cli, args),
        Command::Flush(args) => flush(cli, args),
        Command::Status(args) => status(cli, args),
    };
    let exit_code = outcome.exit_code;
    output::print(format, &outcome);
    exit_code
}

fn validate(args: &crate::cli::InputArgs) -> Outcome {
    match input::envelope(args, None) {
        Ok(envelope) => {
            let mut outcome = Outcome::success("validate", "validated");
            outcome.envelope = serde_json::from_str(&envelope.to_canonical_json()).ok();
            outcome
        }
        Err(error) => failure("validate", &error),
    }
}

fn emit(cli: &Cli, args: &EmitArgs) -> Outcome {
    let envelope = match input::envelope(&args.input, args.record_key.as_deref()) {
        Ok(envelope) => envelope,
        Err(error) => return failure("emit", &error),
    };
    let config = match config::resolve(cli) {
        Ok(config) => config,
        Err(error) => return failure("emit", &error),
    };
    let client = match client::open_client(config.clone()) {
        Ok(client) => client,
        Err(error) => return failure("emit", &error),
    };
    let receipt = match client.emit(envelope) {
        Ok(receipt) => receipt,
        Err(error) => return shutdown_failure("emit", client.as_ref(), &error),
    };
    if args.no_flush {
        let _ = client.shutdown(Duration::ZERO);
        let mut outcome = Outcome::success("emit", "admitted_delivered");
        outcome.receipt = Some(receipt);
        return outcome;
    }
    match client.flush_submission(&receipt.submission_id, config.emit_flush_deadline) {
        Ok(report) => {
            let _ = client.shutdown(Duration::ZERO);
            let mut outcome = Outcome::success("emit", "admitted_delivered");
            outcome.receipt = Some(receipt);
            outcome.flush = Some(report);
            outcome
        }
        Err(error) => {
            let _ = client.shutdown(Duration::ZERO);
            let mut outcome = failure("emit", &error);
            outcome.receipt = Some(receipt);
            outcome
        }
    }
}

fn flush(cli: &Cli, args: &FlushArgs) -> Outcome {
    let config = match config::resolve(cli) {
        Ok(config) => config,
        Err(error) => return failure("flush", &error),
    };
    let client = match client::open_client(config.clone()) {
        Ok(client) => client,
        Err(error) => return failure("flush", &error),
    };
    let deadline = args.timeout.unwrap_or(config.flush_deadline);
    match client.flush(deadline) {
        Ok(report) => {
            let _ = client.shutdown(Duration::ZERO);
            let mut outcome = Outcome::success("flush", "admitted_delivered");
            outcome.flush = Some(report);
            outcome
        }
        Err(error) => shutdown_failure("flush", client.as_ref(), &error),
    }
}

fn status(cli: &Cli, args: &StatusArgs) -> Outcome {
    let query = match status_query(args) {
        Ok(query) => query,
        Err(error) => return failure("status", &error),
    };
    let config = match config::resolve(cli) {
        Ok(config) => config,
        Err(error) => return failure("status", &error),
    };
    let client = match client::open_client(config) {
        Ok(client) => client,
        Err(error) => return failure("status", &error),
    };
    match client.status(query) {
        Ok(status) => {
            let _ = client.shutdown(Duration::ZERO);
            let mut outcome = Outcome::success("status", "status");
            outcome.status = Some(status);
            outcome
        }
        Err(error) => shutdown_failure("status", client.as_ref(), &error),
    }
}

fn status_query(args: &StatusArgs) -> Result<StatusQuery, TelemetryClientError> {
    if !args.submission.is_empty() {
        return args
            .submission
            .iter()
            .map(|value| SubmissionId::from_str(value).map_err(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map(StatusQuery::Submissions);
    }
    if !args.record_key.is_empty() {
        return args
            .record_key
            .iter()
            .map(|value| RecordKey::from_str(value).map_err(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map(StatusQuery::RecordKeys);
    }
    Ok(StatusQuery::Summary)
}

fn shutdown_failure(
    command: &'static str,
    client: &dyn TelemetryClient,
    error: &TelemetryClientError,
) -> Outcome {
    let _ = client.shutdown(Duration::ZERO);
    failure(command, error)
}

fn failure(command: &'static str, error: &TelemetryClientError) -> Outcome {
    Outcome::failure(command, error.clone(), exit::exit_code(error))
}
