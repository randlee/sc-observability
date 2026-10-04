//! Renders the stable machine-readable and compact text result forms.

use crate::{
    cli::OutputFormat,
    constants::{self, CommandName, OutcomeState},
    error::CliError,
};
use sc_observability_types::otlp::submission::{AdmissionReceipt, FlushReport, StoreStatus};
use serde_json::{Value, json};
use std::{error::Error, fmt::Write};

#[derive(Clone, Copy)]
pub(crate) enum AdmittedState {
    Pending,
    Delivered,
    Failed,
}
impl AdmittedState {
    pub(crate) const fn output(self) -> OutcomeState {
        match self {
            Self::Pending => OutcomeState::AdmittedPending,
            Self::Delivered => OutcomeState::AdmittedDelivered,
            Self::Failed => OutcomeState::AdmittedFailed,
        }
    }
}

pub(crate) enum SuccessState {
    Validated,
    Status,
    Delivery(AdmittedState),
}

pub(crate) struct Outcome {
    pub(crate) command: CommandName,
    pub(crate) exit_code: u8,
    result: OutcomeResult,
    pub(crate) flush: Option<FlushReport>,
    pub(crate) status: Option<StoreStatus>,
    pub(crate) envelope: Option<Value>,
}

enum OutcomeResult {
    Success(SuccessState),
    Admitted {
        receipt: AdmissionReceipt,
        state: AdmittedState,
        error: Option<CliError>,
    },
    DeliveryFailure {
        state: AdmittedState,
        error: CliError,
    },
    Rejected(CliError),
}

impl Outcome {
    pub(crate) fn success(command: CommandName, state: SuccessState) -> Self {
        Self {
            command,
            exit_code: constants::EXIT_OK,
            result: OutcomeResult::Success(state),
            flush: None,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn admitted(
        command: CommandName,
        receipt: AdmissionReceipt,
        state: AdmittedState,
    ) -> Self {
        Self {
            command,
            exit_code: constants::EXIT_OK,
            result: OutcomeResult::Admitted {
                receipt,
                state,
                error: None,
            },
            flush: None,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn failure(command: CommandName, error: CliError) -> Self {
        let classification = crate::exit::classify(&error);
        let result = match classification.state {
            crate::exit::FailureState::Rejected => OutcomeResult::Rejected(error),
            crate::exit::FailureState::Delivery(state) => {
                OutcomeResult::DeliveryFailure { state, error }
            }
        };
        Self {
            command,
            exit_code: classification.exit_code,
            result,
            flush: classification.flush,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn admitted_failure(
        command: CommandName,
        receipt: AdmissionReceipt,
        error: CliError,
    ) -> Self {
        let classification = crate::exit::classify(&error);
        let state = match classification.state {
            crate::exit::FailureState::Rejected => AdmittedState::Failed,
            crate::exit::FailureState::Delivery(state) => state,
        };
        Self {
            command,
            exit_code: classification.exit_code,
            result: OutcomeResult::Admitted {
                receipt,
                state,
                error: Some(error),
            },
            flush: classification.flush,
            status: None,
            envelope: None,
        }
    }

    pub(crate) fn state(&self) -> OutcomeState {
        match &self.result {
            OutcomeResult::Success(SuccessState::Validated) => OutcomeState::Validated,
            OutcomeResult::Success(SuccessState::Status) => OutcomeState::Status,
            OutcomeResult::Success(SuccessState::Delivery(state))
            | OutcomeResult::Admitted { state, .. }
            | OutcomeResult::DeliveryFailure { state, .. } => state.output(),
            OutcomeResult::Rejected(_) => OutcomeState::Rejected,
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
            OutcomeResult::Success(_)
            | OutcomeResult::DeliveryFailure { .. }
            | OutcomeResult::Rejected(_) => None,
        }
    }

    fn error(&self) -> Option<&CliError> {
        match &self.result {
            OutcomeResult::Admitted { error, .. } => error.as_ref(),
            OutcomeResult::DeliveryFailure { error, .. } | OutcomeResult::Rejected(error) => {
                Some(error)
            }
            OutcomeResult::Success(_) => None,
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
    let mut text = format!("{} exit={}", outcome.state().as_str(), outcome.exit_code);
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
        "state": outcome.state().as_str(),
        "receipt": outcome.receipt(),
        "flush": outcome.flush,
        "status": outcome.status,
        "envelope": outcome.envelope,
        "error": error,
    })
}

/// Returns the versioned description of the JSON object emitted by [`print`].
///
/// Keep this adjacent to the renderer: clap describes only input syntax, while
/// this projection is derived from the actual result constants and output
/// fields.  The checked-in artifact is a contract snapshot, not another
/// response parser or renderer.
#[cfg(test)]
pub(crate) fn result_contract() -> Value {
    json!({
        "contract": constants::RESULT_SCHEMA,
        "type": "object",
        "required_fields": [
            "schema", "command", "exit_code", "state", "receipt", "flush", "status", "envelope", "error"
        ],
        "fields": {
            "schema": { "type": "string", "const": constants::RESULT_SCHEMA },
            "command": {
                "type": "string",
                "enum": constants::CommandName::ALL.map(CommandName::as_str),
            },
            "exit_code": { "type": "integer", "enum": constants::EXIT_CODE_MEANINGS.iter().map(|(code, _)| code).collect::<Vec<_>>() },
            "state": {
                "type": "string",
                "enum": constants::OutcomeState::ALL.map(OutcomeState::as_str),
            },
            "receipt": { "type": ["object", "null"], "rust_type": "AdmissionReceipt" },
            "flush": { "type": ["object", "null"], "rust_type": "FlushReport" },
            "status": { "type": ["object", "null"], "rust_type": "StoreStatus" },
            "envelope": { "type": ["object", "null"], "rust_type": "SubmissionEnvelope" },
            "error": {
                "type": ["object", "null"],
                "required_fields": ["code", "message", "cause", "remediation"],
                "fields": {
                    "code": { "type": "string" },
                    "message": { "type": "string" },
                    "cause": { "type": ["string", "null"] },
                    "remediation": { "type": "object" },
                },
            },
        },
        "exit_code_meanings": constants::EXIT_CODE_MEANINGS.iter().map(|(code, meaning)| json!({
            "code": code,
            "meaning": meaning,
        })).collect::<Vec<_>>(),
    })
}
