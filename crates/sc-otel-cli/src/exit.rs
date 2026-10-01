//! Maps shared client failures to the stable CLI exit contract.

use crate::{
    constants::{self, OutcomeState},
    error::CliError,
};
use sc_observability_types::otlp::submission::{DeliveryError, TelemetryClientError};

pub(crate) struct Classification {
    pub(crate) exit_code: u8,
    pub(crate) state: OutcomeState,
}

pub(crate) fn classify(error: &CliError) -> Classification {
    let exit_code = exit_code(error);
    let state = match exit_code {
        constants::EXIT_DELIVERY_PENDING => OutcomeState::AdmittedPending,
        constants::EXIT_DELIVERY_FAILED => OutcomeState::AdmittedFailed,
        _ => OutcomeState::Rejected,
    };
    Classification { exit_code, state }
}

fn exit_code(error: &CliError) -> u8 {
    match error {
        CliError::Input(_) => constants::EXIT_INVALID_INPUT,
        CliError::Internal(_) => constants::EXIT_INTERNAL,
        CliError::Telemetry(error) => telemetry_exit_code(error),
    }
}

fn telemetry_exit_code(error: &TelemetryClientError) -> u8 {
    match error {
        TelemetryClientError::Submission(_) => constants::EXIT_INVALID_INPUT,
        TelemetryClientError::Config(_) => constants::EXIT_CONFIG,
        TelemetryClientError::Admission(_) => constants::EXIT_ADMISSION,
        TelemetryClientError::Delivery(DeliveryError::DeadlineExceeded { .. }) => {
            constants::EXIT_DELIVERY_PENDING
        }
        TelemetryClientError::Delivery(DeliveryError::TerminalFailure { .. }) => {
            constants::EXIT_DELIVERY_FAILED
        }
        _ => constants::EXIT_INTERNAL,
    }
}
