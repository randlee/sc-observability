//! Renders the stable machine-readable and compact text result forms.

use crate::{cli::OutputFormat, constants, error::CliError};
use sc_observability_types::otlp::submission::{
    AdmissionReceipt, FlushReport, StoreStatus, TelemetryClientError,
};
use serde_json::{Value, json};
use std::{error::Error, fmt::Write};

pub(crate) struct Outcome {
    pub(crate) command: &'static str,
    pub(crate) exit_code: u8,
    pub(crate) state: &'static str,
    pub(crate) receipt: Option<AdmissionReceipt>,
    pub(crate) flush: Option<FlushReport>,
    pub(crate) status: Option<StoreStatus>,
    pub(crate) envelope: Option<Value>,
    pub(crate) error: Option<CliError>,
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

    pub(crate) fn failure(command: &'static str, error: CliError) -> Self {
        let exit_code = crate::exit::exit_code(&error);
        let flush = match error.telemetry() {
            Some(TelemetryClientError::Delivery(
                sc_observability_types::otlp::submission::DeliveryError::DeadlineExceeded {
                    report,
                    ..
                }
                | sc_observability_types::otlp::submission::DeliveryError::TerminalFailure {
                    report,
                    ..
                },
            )) => Some(report.clone()),
            _ => None,
        };
        Self {
            command,
            exit_code,
            state: if exit_code == constants::EXIT_DELIVERY_PENDING {
                constants::STATE_ADMITTED_PENDING
            } else if exit_code == constants::EXIT_DELIVERY_FAILED {
                constants::STATE_ADMITTED_FAILED
            } else {
                constants::STATE_REJECTED
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
        OutputFormat::Text => print_text(outcome),
    }
}

fn print_text(outcome: &Outcome) {
    let mut text = format!("{} exit={}", outcome.state, outcome.exit_code);
    if let Some(receipt) = &outcome.receipt {
        write!(text, " submission={}", receipt.submission_id)
            .expect("writing to a String cannot fail");
    }
    if let Some(error) = &outcome.error {
        write!(text, " error={}", error.code()).expect("writing to a String cannot fail");
        eprintln!("{}: {error}", error.code());
    }
    println!("{text}");
}

fn as_json(outcome: &Outcome) -> Value {
    let error = outcome.error.as_ref().map(|error| {
        json!({
            "code": error.code(),
            "message": error.to_string(),
            "cause": error.source().map(ToString::to_string),
            "remediation": error.remediation(),
        })
    });
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
