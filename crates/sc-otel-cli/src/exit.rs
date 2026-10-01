//! Maps shared client failures to the stable CLI exit contract.

use crate::{constants, error::CliError};
use sc_observability_types::otlp::submission::{DeliveryError, TelemetryClientError};

pub(crate) fn exit_code(error: &CliError) -> u8 {
    match error {
        CliError::Input(_) => constants::EXIT_INVALID_INPUT,
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
