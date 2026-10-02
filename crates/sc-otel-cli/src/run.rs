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
use std::str::FromStr;

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
    with_session(cli, constants::CommandName::Emit, |config, client| {
        let receipt = match client.emit(envelope) {
            Ok(receipt) => receipt,
            Err(error) => return failure(constants::CommandName::Emit, error),
        };
        if args.no_flush {
            return Outcome::success(
                constants::CommandName::Emit,
                constants::OutcomeState::AdmittedPending,
            )
            .with_receipt(receipt);
        }
        flush_emission(client, receipt, config.emit_flush_deadline)
    })
}

fn flush_emission(
    client: &dyn TelemetryClient,
    receipt: sc_observability_types::otlp::submission::AdmissionReceipt,
    deadline: std::time::Duration,
) -> Outcome {
    match client.flush_submission(&receipt.submission_id, deadline) {
        Ok(report) => Outcome::success(constants::CommandName::Emit, delivery_state(&report))
            .with_receipt(receipt)
            .with_flush(report),
        Err(error) => {
            let error: CliError = error.into();
            Outcome::admitted_failure(constants::CommandName::Emit, receipt, error)
        }
    }
}

fn flush(cli: &Cli, args: &FlushArgs) -> Outcome {
    with_session(cli, constants::CommandName::Flush, |config, client| {
        let deadline = args.timeout.unwrap_or(config.flush_deadline);
        match client.flush(deadline) {
            Ok(report) => Outcome::success(constants::CommandName::Flush, delivery_state(&report))
                .with_flush(report),
            Err(error) => failure(constants::CommandName::Flush, error),
        }
    })
}

fn status(cli: &Cli, args: &StatusArgs) -> Outcome {
    let query = match status_query(args) {
        Ok(query) => query,
        Err(error) => return failure(constants::CommandName::Status, error),
    };
    with_session(
        cli,
        constants::CommandName::Status,
        |_, client| match client.status(query) {
            Ok(status) => Outcome::success(
                constants::CommandName::Status,
                constants::OutcomeState::Status,
            )
            .with_status(status),
            Err(error) => failure(constants::CommandName::Status, error),
        },
    )
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

fn with_session(
    cli: &Cli,
    command: constants::CommandName,
    action: impl FnOnce(&TelemetryClientConfig, &dyn TelemetryClient) -> Outcome,
) -> Outcome {
    let config = match config::resolve(cli) {
        Ok(config) => config,
        Err(error) => return failure(command, error),
    };
    let client = match client::open_client(config.clone()) {
        Ok(client) => client,
        Err(error) => return failure(command, error),
    };
    finish_shutdown(
        command,
        client.as_ref(),
        config.flush_deadline,
        action(&config, client.as_ref()),
    )
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
    deadline: std::time::Duration,
    outcome: Outcome,
) -> Outcome {
    match client.shutdown(deadline) {
        Ok(_) => outcome,
        Err(error) => {
            let error: CliError = error.into();
            let receipt = outcome.receipt().cloned();
            let mut shutdown = match receipt {
                Some(receipt) => Outcome::admitted_failure(command, receipt, error),
                None => failure(command, error),
            };
            shutdown.flush = outcome.flush.or(shutdown.flush);
            shutdown.status = outcome.status;
            shutdown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::otlp::submission::{
        AdmissionError, AdmissionReceipt, ConfigOverrides, ConfigSources, FlushReport, Signal,
        StatusQuery, StoreStatus, SubmissionEnvelope, SubmissionId, resolve_config,
    };
    use sc_observability_types::{ErrorCode, ErrorContext, Remediation, Timestamp};
    use std::{path::PathBuf, sync::Mutex, time::Duration};

    struct ShutdownDeadlineRecorder(Mutex<Option<Duration>>);

    impl TelemetryClient for ShutdownDeadlineRecorder {
        fn open(_: TelemetryClientConfig) -> Result<Self, TelemetryClientError>
        where
            Self: Sized,
        {
            unreachable!("the recorder is constructed directly")
        }

        fn emit(&self, _: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
            unreachable!("the regression exercises only shutdown")
        }

        fn flush(&self, _: Duration) -> Result<FlushReport, TelemetryClientError> {
            unreachable!("the regression exercises only shutdown")
        }

        fn flush_submission(
            &self,
            _: &SubmissionId,
            _: Duration,
        ) -> Result<FlushReport, TelemetryClientError> {
            unreachable!("the regression exercises only shutdown")
        }

        fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
            *self.0.lock().expect("shutdown deadline lock") = Some(deadline);
            Ok(FlushReport::default())
        }

        fn status(&self, _: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
            unreachable!("the regression exercises only shutdown")
        }
    }

    #[test]
    fn shutdown_uses_the_resolved_lifecycle_deadline_after_successful_delivery() {
        let mut overrides = ConfigOverrides::default();
        overrides.emit_flush_deadline = Some(Duration::from_millis(73));
        overrides.flush_deadline = Some(Duration::from_millis(73));
        overrides.store_path = Some(PathBuf::from("test-shutdown-deadline.sqlite"));
        let config = resolve_config(ConfigSources::new(&overrides, None, &no_environment))
            .expect("resolved telemetry config");
        let client = ShutdownDeadlineRecorder(Mutex::new(None));
        let outcome = Outcome::success(
            constants::CommandName::Emit,
            constants::OutcomeState::AdmittedDelivered,
        );

        let result = finish_shutdown(
            constants::CommandName::Emit,
            &client,
            config.flush_deadline,
            outcome,
        );

        assert_eq!(result.exit_code, constants::EXIT_OK);
        assert_eq!(
            *client.0.lock().expect("shutdown deadline lock"),
            Some(Duration::from_millis(73))
        );
    }

    fn no_environment(_: &str) -> Option<String> {
        None
    }

    struct FlushSubmissionFailure;

    impl TelemetryClient for FlushSubmissionFailure {
        fn open(_: TelemetryClientConfig) -> Result<Self, TelemetryClientError>
        where
            Self: Sized,
        {
            unreachable!("the client is constructed directly")
        }

        fn emit(&self, _: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
            unreachable!("the regression exercises flush_submission")
        }

        fn flush(&self, _: Duration) -> Result<FlushReport, TelemetryClientError> {
            unreachable!("the regression exercises flush_submission")
        }

        fn flush_submission(
            &self,
            _: &SubmissionId,
            _: Duration,
        ) -> Result<FlushReport, TelemetryClientError> {
            Err(AdmissionError::Closed {
                context: Box::new(ErrorContext::new(
                    ErrorCode::new_static("SC_OTEL_TEST_CLOSED"),
                    "closed after admission",
                    Remediation::not_recoverable("test"),
                )),
            }
            .into())
        }

        fn shutdown(&self, _: Duration) -> Result<FlushReport, TelemetryClientError> {
            unreachable!("the regression exercises flush_submission")
        }

        fn status(&self, _: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
            unreachable!("the regression exercises flush_submission")
        }
    }

    #[test]
    fn flush_submission_non_delivery_failure_keeps_admission_without_panicking() {
        let receipt = AdmissionReceipt::new(
            "018f8f5e-5c4c-7abc-8def-0123456789ab"
                .parse()
                .expect("submission id"),
            None,
            Timestamp::now_utc(),
            vec![Signal::Logs],
            false,
        );

        let outcome = flush_emission(&FlushSubmissionFailure, receipt, Duration::ZERO);

        assert_eq!(outcome.exit_code, constants::EXIT_ADMISSION);
        assert_eq!(outcome.state.as_str(), "admitted_failed");
        assert!(outcome.receipt().is_some());
    }
}
