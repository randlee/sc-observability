//! Renders the stable machine-readable and compact text result forms.

use crate::{cli::OutputFormat, constants};
use sc_observability_types::otlp::submission::{
    AdmissionReceipt, FlushReport, StoreStatus, TelemetryClientError,
};
use serde_json::{Value, json};

pub(crate) struct Outcome {
    pub(crate) command: &'static str,
    pub(crate) exit_code: u8,
    pub(crate) state: &'static str,
    pub(crate) receipt: Option<AdmissionReceipt>,
    pub(crate) flush: Option<FlushReport>,
    pub(crate) status: Option<StoreStatus>,
    pub(crate) envelope: Option<Value>,
    pub(crate) error: Option<TelemetryClientError>,
}

impl Outcome {
    pub(crate) fn success(command: &'static str, state: &'static str) -> Self {
        Self {
            command,
            exit_code: constants::EXIT_OK,
            state,
            receipt: None,
            flush: None,
            status: None,
            envelope: None,
            error: None,
        }
    }

    pub(crate) fn failure(
        command: &'static str,
        error: TelemetryClientError,
        exit_code: u8,
    ) -> Self {
        let flush = match &error {
            TelemetryClientError::Delivery(
                sc_observability_types::otlp::submission::DeliveryError::DeadlineExceeded {
                    report,
                    ..
                }
                | sc_observability_types::otlp::submission::DeliveryError::TerminalFailure {
                    report,
                    ..
                },
            ) => Some(report.clone()),
            _ => None,
        };
        Self {
            command,
            exit_code,
            state: if exit_code == constants::EXIT_DELIVERY_PENDING {
                "admitted_pending"
            } else if exit_code == constants::EXIT_DELIVERY_FAILED {
                "admitted_failed"
            } else {
                "rejected"
            },
            receipt: None,
            flush,
            status: None,
            envelope: None,
            error: Some(error),
        }
    }
}

pub(crate) fn print(format: OutputFormat, outcome: &Outcome) {
    match format {
        OutputFormat::Json => println!("{}", as_json(outcome)),
        OutputFormat::Text => println!(
            "{} {} exit={}",
            outcome.command, outcome.state, outcome.exit_code
        ),
    }
}

fn as_json(outcome: &Outcome) -> Value {
    let error = outcome
        .error
        .as_ref()
        .map(|error| json!({"code": error.code().as_str(), "message": error.to_string()}));
    json!({
        "schema": constants::RESULT_SCHEMA,
        "command": outcome.command,
        "exit_code": outcome.exit_code,
        "state": outcome.state,
        "receipt": outcome.receipt,
        "flush": outcome.flush,
        "status": outcome.status,
        "envelope": outcome.envelope,
        "error": error,
    })
}
