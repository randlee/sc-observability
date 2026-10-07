//! Maps shared client failures to the stable CLI exit contract.

use crate::{constants, error::CliError, output::AdmittedState};
use sc_observability_types::otlp::submission::{DeliveryError, FlushReport, TelemetryClientError};

pub(crate) enum FailureState {
    Rejected,
    Delivery(AdmittedState),
}

pub(crate) struct Classification {
    pub(crate) exit_code: u8,
    pub(crate) state: FailureState,
    pub(crate) flush: Option<FlushReport>,
}

/// Maps clap's actual parser disposition to the CLI's process contract.
pub(crate) fn parser_exit(error: &clap::Error) -> u8 {
    if error.use_stderr() {
        constants::EXIT_USAGE
    } else {
        constants::EXIT_OK
    }
}

pub(crate) fn classify(error: &CliError) -> Classification {
    match error {
        CliError::Input(_) => rejected(constants::EXIT_INVALID_INPUT),
        CliError::Telemetry(TelemetryClientError::Submission(_)) => {
            rejected(constants::EXIT_INVALID_INPUT)
        }
        CliError::Telemetry(TelemetryClientError::Config(_)) => rejected(constants::EXIT_CONFIG),
        CliError::Telemetry(TelemetryClientError::Admission(_)) => {
            rejected(constants::EXIT_ADMISSION)
        }
        CliError::Telemetry(TelemetryClientError::Delivery(DeliveryError::DeadlineExceeded {
            report,
            ..
        })) => Classification {
            exit_code: constants::EXIT_DELIVERY_PENDING,
            state: FailureState::Delivery(AdmittedState::Pending),
            flush: Some(report.clone()),
        },
        CliError::Telemetry(TelemetryClientError::Delivery(DeliveryError::TerminalFailure {
            report,
            ..
        })) => Classification {
            exit_code: constants::EXIT_DELIVERY_FAILED,
            state: FailureState::Delivery(AdmittedState::Failed),
            flush: Some(report.clone()),
        },
        // `TelemetryClientError` is non-exhaustive. A future shared variant has no
        // CLI contract yet, so it remains an internal failure until this table is
        // explicitly extended rather than being misreported as another class.
        CliError::Internal(_) | CliError::Telemetry(_) => rejected(constants::EXIT_INTERNAL),
    }
}

const fn rejected(exit_code: u8) -> Classification {
    Classification {
        exit_code,
        state: FailureState::Rejected,
        flush: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::OutcomeState;
    use crate::error::InputError;
    use sc_observability_types::otlp::submission::{
        AdmissionError, DeliveryError, FlushReport, SubmissionError, TelemetryConfigError,
    };
    use sc_observability_types::{ErrorCode, ErrorContext, Remediation};

    fn context() -> Box<ErrorContext> {
        Box::new(ErrorContext::new(
            ErrorCode::new_static("SC_OTEL_TEST"),
            "test error",
            Remediation::not_recoverable("test"),
        ))
    }

    fn report() -> FlushReport {
        FlushReport::default()
    }

    #[test]
    fn classifies_every_supported_error_arm_once() {
        let cases = [
            (
                CliError::Input(InputError::Fragment {
                    flag: "--log",
                    source: serde_json::from_str::<serde_json::Value>("{")
                        .expect_err("invalid JSON"),
                }),
                constants::EXIT_INVALID_INPUT,
                OutcomeState::Rejected,
            ),
            (
                CliError::Internal("broken".into()),
                constants::EXIT_INTERNAL,
                OutcomeState::Rejected,
            ),
            (
                CliError::Telemetry(SubmissionError::InvalidJson { context: context() }.into()),
                constants::EXIT_INVALID_INPUT,
                OutcomeState::Rejected,
            ),
            (
                CliError::Telemetry(
                    TelemetryConfigError::MissingField {
                        field: "store_path",
                        context: context(),
                    }
                    .into(),
                ),
                constants::EXIT_CONFIG,
                OutcomeState::Rejected,
            ),
            (
                CliError::Telemetry(AdmissionError::Closed { context: context() }.into()),
                constants::EXIT_ADMISSION,
                OutcomeState::Rejected,
            ),
            (
                CliError::Telemetry(
                    DeliveryError::DeadlineExceeded {
                        report: report(),
                        context: context(),
                    }
                    .into(),
                ),
                constants::EXIT_DELIVERY_PENDING,
                OutcomeState::AdmittedPending,
            ),
            (
                CliError::Telemetry(
                    DeliveryError::TerminalFailure {
                        report: report(),
                        context: context(),
                    }
                    .into(),
                ),
                constants::EXIT_DELIVERY_FAILED,
                OutcomeState::AdmittedFailed,
            ),
        ];
        for (error, exit_code, state) in cases {
            let classification = classify(&error);
            assert_eq!(classification.exit_code, exit_code);
            let actual = match classification.state {
                FailureState::Rejected => OutcomeState::Rejected,
                FailureState::Delivery(state) => state.output(),
            };
            assert_eq!(actual.as_str(), state.as_str());
        }
    }
}
