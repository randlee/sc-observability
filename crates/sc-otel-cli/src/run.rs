//! Executes parsed commands through the shared telemetry client contract.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed client diagnostics"
)]

use crate::{
    cli::{Cli, Command, EmitArgs, FlushArgs, StatusArgs},
    client, config, constants,
    error::CliError,
    input,
    output::{self, Outcome},
};
use sc_observability_types::otlp::submission::{
    RecordKey, StatusQuery, SubmissionId, TelemetryClient, TelemetryClientConfig,
    TelemetryClientError,
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
            let mut outcome = Outcome::success(
                constants::CommandName::Validate,
                constants::OutcomeState::Validated,
            );
            outcome.envelope = match serde_json::to_value(envelope) {
                Ok(envelope) => Some(envelope),
                Err(error) => {
                    return failure(
                        constants::CommandName::Validate,
                        CliError::Internal(format!("unable to render validated envelope: {error}")),
                    );
                }
            };
            outcome
        }
        Err(error) => failure(constants::CommandName::Validate, error),
    }
}

fn emit(cli: &Cli, args: &EmitArgs) -> Outcome {
    let envelope = match input::envelope(&args.input, args.record_key.as_deref()) {
        Ok(envelope) => envelope,
        Err(error) => return failure(constants::CommandName::Emit, error),
    };
    let (config, client) = match open_client(cli, constants::CommandName::Emit) {
        Ok(client) => client,
        Err(outcome) => return outcome,
    };
    let receipt = match client.emit(envelope) {
        Ok(receipt) => receipt,
        Err(error) => {
            return shutdown_failure(constants::CommandName::Emit, client.as_ref(), error);
        }
    };
    if args.no_flush {
        let mut outcome = Outcome::success(
            constants::CommandName::Emit,
            constants::OutcomeState::AdmittedPending,
        );
        outcome.receipt = Some(receipt);
        return finish_shutdown(constants::CommandName::Emit, client.as_ref(), outcome);
    }
    match client.flush_submission(&receipt.submission_id, config.emit_flush_deadline) {
        Ok(report) => {
            let mut outcome =
                Outcome::success(constants::CommandName::Emit, delivery_state(&report));
            outcome.receipt = Some(receipt);
            outcome.flush = Some(report);
            finish_shutdown(constants::CommandName::Emit, client.as_ref(), outcome)
        }
        Err(error) => {
            let mut outcome = failure(constants::CommandName::Emit, error);
            outcome.receipt = Some(receipt);
            finish_shutdown(constants::CommandName::Emit, client.as_ref(), outcome)
        }
    }
}

fn flush(cli: &Cli, args: &FlushArgs) -> Outcome {
    let (config, client) = match open_client(cli, constants::CommandName::Flush) {
        Ok(client) => client,
        Err(outcome) => return outcome,
    };
    let deadline = args.timeout.unwrap_or(config.flush_deadline);
    match client.flush(deadline) {
        Ok(report) => {
            let mut outcome =
                Outcome::success(constants::CommandName::Flush, delivery_state(&report));
            outcome.flush = Some(report);
            finish_shutdown(constants::CommandName::Flush, client.as_ref(), outcome)
        }
        Err(error) => shutdown_failure(constants::CommandName::Flush, client.as_ref(), error),
    }
}

fn status(cli: &Cli, args: &StatusArgs) -> Outcome {
    let query = match status_query(args) {
        Ok(query) => query,
        Err(error) => return failure(constants::CommandName::Status, error),
    };
    let (_, client) = match open_client(cli, constants::CommandName::Status) {
        Ok(client) => client,
        Err(outcome) => return outcome,
    };
    match client.status(query) {
        Ok(status) => {
            let mut outcome = Outcome::success(
                constants::CommandName::Status,
                constants::OutcomeState::Status,
            );
            outcome.status = Some(status);
            finish_shutdown(constants::CommandName::Status, client.as_ref(), outcome)
        }
        Err(error) => shutdown_failure(constants::CommandName::Status, client.as_ref(), error),
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

fn open_client(
    cli: &Cli,
    command: constants::CommandName,
) -> Result<(TelemetryClientConfig, Box<dyn TelemetryClient>), Outcome> {
    let config = config::resolve(cli).map_err(|error| failure(command, error))?;
    let client = client::open_client(config.clone()).map_err(|error| failure(command, error))?;
    Ok((config, client))
}

fn shutdown_failure(
    command: constants::CommandName,
    client: &dyn TelemetryClient,
    error: TelemetryClientError,
) -> Outcome {
    if let Err(shutdown) = client.shutdown(Duration::ZERO) {
        eprintln!(
            "{}: shutdown failed after command failure: {shutdown}",
            shutdown.code()
        );
    }
    failure(command, error)
}

fn failure(command: constants::CommandName, error: impl Into<CliError>) -> Outcome {
    Outcome::failure(command, error.into())
}

fn delivery_state(
    report: &sc_observability_types::otlp::submission::FlushReport,
) -> constants::OutcomeState {
    if report.failed.total() > 0 || report.evicted.total() > 0 {
        constants::OutcomeState::AdmittedFailed
    } else if report.still_pending.total() > 0 {
        constants::OutcomeState::AdmittedPending
    } else {
        constants::OutcomeState::AdmittedDelivered
    }
}

fn finish_shutdown(
    command: constants::CommandName,
    client: &dyn TelemetryClient,
    outcome: Outcome,
) -> Outcome {
    match client.shutdown(Duration::ZERO) {
        Ok(_) => outcome,
        Err(error) => {
            let mut shutdown = failure(command, error);
            shutdown.receipt = outcome.receipt;
            shutdown.flush = outcome.flush.or(shutdown.flush);
            shutdown.status = outcome.status;
            shutdown
        }
    }
}
