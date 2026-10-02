//! Renders the stable machine-readable and compact text result forms.

use crate::{
    cli::OutputFormat,
    constants::{self, CommandName, OutcomeState},
    error::CliError,
};
use sc_observability_types::otlp::submission::{AdmissionReceipt, FlushReport, StoreStatus};
use serde_json::{Value, json};
use std::{error::Error, fmt::Write};

pub(crate) struct Outcome {
    pub(crate) command: CommandName,
    pub(crate) exit_code: u8,
    pub(crate) state: OutcomeState,
    result: OutcomeResult,
    pub(crate) flush: Option<FlushReport>,
    pub(crate) status: Option<StoreStatus>,
    pub(crate) envelope: Option<Value>,
}

enum OutcomeResult {
    Success,
    Admitted {
        receipt: AdmissionReceipt,
        error: Option<CliError>,
    },
    DeliveryFailure(CliError),
    Rejected(CliError),
}

impl Outcome {
    pub(crate) fn success(command: CommandName, state: OutcomeState) -> Self {
        Self {
            command,
            exit_code: constants::EXIT_OK,
            state,
            result: OutcomeResult::Success,
            flush: None,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn failure(command: CommandName, error: CliError) -> Self {
        let classification = crate::exit::classify(&error);
        let rejected = matches!(classification.state, OutcomeState::Rejected);
        Self {
            command,
            exit_code: classification.exit_code,
            state: classification.state,
            result: if rejected {
                OutcomeResult::Rejected(error)
            } else {
                OutcomeResult::DeliveryFailure(error)
            },
            flush: classification.flush,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn with_receipt(mut self, receipt: AdmissionReceipt) -> Self {
        self.result = OutcomeResult::Admitted {
            receipt,
            error: None,
        };
        self
    }

    pub(crate) fn admitted_failure(
        command: CommandName,
        receipt: AdmissionReceipt,
        error: CliError,
    ) -> Self {
        let classification = crate::exit::classify(&error);
        Self {
            command,
            exit_code: classification.exit_code,
            // Admission completed before the later operation failed, so this is
            // never a rejected outcome even when the later error has that class.
            state: if matches!(classification.state, OutcomeState::Rejected) {
                OutcomeState::AdmittedFailed
            } else {
                classification.state
            },
            result: OutcomeResult::Admitted {
                receipt,
                error: Some(error),
            },
            flush: classification.flush,
            status: None,
            envelope: None,
        }
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

impl Outcome {
    pub(crate) fn receipt(&self) -> Option<&AdmissionReceipt> {
        match &self.result {
            OutcomeResult::Admitted { receipt, .. } => Some(receipt),
            OutcomeResult::Success
            | OutcomeResult::DeliveryFailure(_)
            | OutcomeResult::Rejected(_) => None,
        }
    }

    fn error(&self) -> Option<&CliError> {
        match &self.result {
            OutcomeResult::Admitted { error, .. } => error.as_ref(),
            OutcomeResult::DeliveryFailure(error) | OutcomeResult::Rejected(error) => Some(error),
            OutcomeResult::Success => None,
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
    let mut text = format!("{} exit={}", outcome.state.as_str(), outcome.exit_code);
    if let Some(receipt) = outcome.receipt() {
        write!(text, " submission={}", receipt.submission_id)
            .expect("writing to a String cannot fail");
    }
    if let Some(error) = outcome.error() {
        write!(text, " error={}", error.code()).expect("writing to a String cannot fail");
        eprintln!("{}: {error}", error.code());
    }
    println!("{text}");
}

fn as_json(outcome: &Outcome) -> Value {
    let error = outcome.error().map(|error| {
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
        "receipt": outcome.receipt(),
        "flush": outcome.flush,
        "status": outcome.status,
        "envelope": outcome.envelope,
        "error": error,
    })
}
