//! Renders the stable machine-readable and compact text result forms.

use crate::{
    cli::OutputFormat,
    constants::{self, CommandName, OutcomeState},
    error::CliError,
};
use sc_observability_types::otlp::submission::{
    AdmissionReceipt, FlushReport, StoreStatus, TelemetryClientError,
};
use serde_json::{Value, json};
use std::{error::Error, fmt::Write};

pub(crate) struct Outcome {
    pub(crate) command: CommandName,
    pub(crate) exit_code: u8,
    pub(crate) state: OutcomeState,
    pub(crate) receipt: Option<AdmissionReceipt>,
    pub(crate) flush: Option<FlushReport>,
    pub(crate) status: Option<StoreStatus>,
    pub(crate) envelope: Option<Value>,
    pub(crate) error: Option<CliError>,
}

impl Outcome {
    pub(crate) fn success(command: CommandName, state: OutcomeState) -> Self {
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

    pub(crate) fn failure(command: CommandName, error: CliError) -> Self {
        let classification = crate::exit::classify(&error);
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
            exit_code: classification.exit_code,
            state: classification.state,
            receipt: None,
            flush,
            status: None,
            envelope: None,
            error: Some(error),
        }
    }

    pub(crate) fn with_receipt(mut self, receipt: AdmissionReceipt) -> Self {
        assert!(
            !matches!(self.state, OutcomeState::Rejected),
            "a rejected outcome cannot carry an admission receipt"
        );
        self.receipt = Some(receipt);
        self
    }

    pub(crate) fn with_flush(mut self, flush: FlushReport) -> Self {
        self.flush = Some(flush);
        self
    }

    pub(crate) fn with_status(mut self, status: StoreStatus) -> Self {
        self.status = Some(status);
        self
    }
}

pub(crate) fn print(format: OutputFormat, outcome: &Outcome) {
    match format {
        OutputFormat::Json => println!("{}", as_json(outcome)),
        OutputFormat::Text => print_text(outcome),
    }
}

fn print_text(outcome: &Outcome) {
    let mut text = format!("{} exit={}", outcome.state.as_str(), outcome.exit_code);
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
        "command": outcome.command.as_str(),
        "exit_code": outcome.exit_code,
        "state": outcome.state.as_str(),
        "receipt": outcome.receipt,
        "flush": outcome.flush,
        "status": outcome.status,
        "envelope": outcome.envelope,
        "error": error,
    })
}
