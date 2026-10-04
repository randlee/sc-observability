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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cli::Cli, constants, error::InputError, exit};
    use clap::Parser;
    use sc_observability_types::{
        ErrorCode, ErrorContext, Remediation, Timestamp,
        otlp::submission::{
            AdmissionError, AdmissionReceipt, DeliveryError, FlushReport, Signal, SubmissionError,
            TelemetryConfigError,
        },
    };
    use std::{collections::BTreeSet, path::PathBuf};

    fn context() -> Box<ErrorContext> {
        Box::new(ErrorContext::new(
            ErrorCode::new_static("SC_OTEL_TEST"),
            "test error",
            Remediation::not_recoverable("test"),
        ))
    }

    fn receipt() -> AdmissionReceipt {
        AdmissionReceipt::new(
            "018f8f5e-5c4c-7abc-8def-0123456789ab"
                .parse()
                .expect("fixture submission id"),
            None,
            Timestamp::now_utc(),
            vec![Signal::Logs],
            false,
        )
    }

    fn expected_contract() -> Value {
        serde_json::from_str(include_str!("../../../schema/cli/sc-otel/results/v1.json"))
            .expect("checked-in result contract is valid JSON")
    }

    fn actual_outcomes() -> Vec<Outcome> {
        let mut validated = Outcome::success(CommandName::Validate, SuccessState::Validated);
        validated.envelope = Some(json!({"resource_logs": []}));

        vec![
            validated,
            Outcome::success(CommandName::Status, SuccessState::Status)
                .with_status(StoreStatus::default()),
            Outcome::success(
                CommandName::Flush,
                SuccessState::Delivery(AdmittedState::Delivered),
            )
            .with_flush(FlushReport::default()),
            Outcome::admitted(CommandName::Emit, receipt(), AdmittedState::Pending)
                .with_flush(FlushReport::default()),
            Outcome::failure(
                CommandName::Validate,
                CliError::Input(InputError::Stdin {
                    source: std::io::Error::other("stdin failed"),
                }),
            ),
            Outcome::failure(
                CommandName::Emit,
                CliError::Input(InputError::File {
                    path: PathBuf::from("fragment.json"),
                    source: std::io::Error::other("file failed"),
                }),
            ),
            Outcome::failure(
                CommandName::Flush,
                CliError::Input(InputError::Fragment {
                    flag: "--log",
                    source: serde_json::from_str::<Value>("{").expect_err("invalid JSON"),
                }),
            ),
            Outcome::failure(CommandName::Status, CliError::Internal("broken".into())),
            Outcome::failure(
                CommandName::Validate,
                CliError::Telemetry(SubmissionError::InvalidJson { context: context() }.into()),
            ),
            Outcome::failure(
                CommandName::Flush,
                CliError::Telemetry(
                    TelemetryConfigError::MissingField {
                        field: "store_path",
                        context: context(),
                    }
                    .into(),
                ),
            ),
            Outcome::failure(
                CommandName::Emit,
                CliError::Telemetry(AdmissionError::Closed { context: context() }.into()),
            ),
            Outcome::failure(
                CommandName::Emit,
                CliError::Telemetry(
                    DeliveryError::DeadlineExceeded {
                        report: FlushReport::default(),
                        context: context(),
                    }
                    .into(),
                ),
            ),
            Outcome::failure(
                CommandName::Emit,
                CliError::Telemetry(
                    DeliveryError::TerminalFailure {
                        report: FlushReport::default(),
                        context: context(),
                    }
                    .into(),
                ),
            ),
        ]
    }

    fn documented_exit(meaning: &str) -> u8 {
        constants::EXIT_CODE_MEANINGS
            .iter()
            .find_map(|(code, documented)| (*documented == meaning).then_some(*code))
            .expect("every classified exit meaning is documented")
    }

    fn assert_exit_contract(contract: &Value, payloads: &[Value]) {
        let usage = Cli::try_parse_from(["sc-otel", "--unknown"])
            .expect_err("unknown options are actual clap usage errors");
        let mut actual_exit_codes = payloads
            .iter()
            .map(|payload| {
                u8::try_from(payload["exit_code"].as_u64().expect("integer exit code"))
                    .expect("exit code fits its u8 process representation")
            })
            .collect::<BTreeSet<_>>();
        actual_exit_codes.insert(exit::parser_exit(&usage));
        assert_eq!(
            actual_exit_codes,
            constants::EXIT_CODE_MEANINGS
                .iter()
                .map(|(code, _)| *code)
                .collect(),
            "every documented exit is exercised by actual output or clap parsing"
        );
        assert_eq!(
            contract["exit_code_meanings"],
            serde_json::to_value(
                constants::EXIT_CODE_MEANINGS
                    .iter()
                    .map(|(code, meaning)| json!({"code": code, "meaning": meaning}))
                    .collect::<Vec<_>>(),
            )
            .expect("exit meanings serialize"),
        );

        for (error, meaning) in [
            (
                CliError::Input(InputError::Stdin {
                    source: std::io::Error::other("stdin failed"),
                }),
                "invalid_submission_input",
            ),
            (CliError::Internal("broken".into()), "internal_failure"),
            (
                CliError::Telemetry(
                    TelemetryConfigError::MissingField {
                        field: "store_path",
                        context: context(),
                    }
                    .into(),
                ),
                "invalid_or_missing_configuration",
            ),
            (
                CliError::Telemetry(AdmissionError::Closed { context: context() }.into()),
                "submission_not_admitted",
            ),
            (
                CliError::Telemetry(
                    DeliveryError::DeadlineExceeded {
                        report: FlushReport::default(),
                        context: context(),
                    }
                    .into(),
                ),
                "admitted_delivery_pending",
            ),
            (
                CliError::Telemetry(
                    DeliveryError::TerminalFailure {
                        report: FlushReport::default(),
                        context: context(),
                    }
                    .into(),
                ),
                "admitted_delivery_failed",
            ),
        ] {
            assert_eq!(exit::classify(&error).exit_code, documented_exit(meaning));
        }
    }

    fn assert_payload_matches_contract(contract: &Value, payload: &Value) {
        let object = payload.as_object().expect("as_json emits an object");
        let required = contract["required_fields"]
            .as_array()
            .expect("contract lists required fields")
            .iter()
            .map(|value| value.as_str().expect("field name"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            required
        );

        let fields = contract["fields"].as_object().expect("contract fields");
        for (name, specification) in fields {
            let value = &payload[name];
            let allowed = specification["type"].as_array().map_or_else(
                || vec![specification["type"].as_str().expect("type")],
                |values| {
                    values
                        .iter()
                        .map(|value| value.as_str().expect("type"))
                        .collect::<Vec<_>>()
                },
            );
            let actual = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            assert!(
                allowed.contains(&actual),
                "{name} emitted {actual}, allowed {allowed:?}"
            );
            if let Some(values) = specification.get("enum").and_then(Value::as_array) {
                assert!(
                    values.contains(value),
                    "{name} emitted {value}, absent from {values:?}"
                );
            }
        }

        if let Some(error) = payload["error"].as_object() {
            let error_contract = &fields["error"];
            for name in error_contract["required_fields"]
                .as_array()
                .expect("error required fields")
            {
                assert!(error.contains_key(name.as_str().expect("error field name")));
            }
            assert!(error["code"].is_string());
            assert!(error["message"].is_string());
            assert!(error["cause"].is_null() || error["cause"].is_string());
            assert!(error["remediation"].is_object());
        }
    }

    #[test]
    fn result_contract_is_exercised_by_actual_output_and_all_exit_paths() {
        let contract = expected_contract();
        let payloads = actual_outcomes().iter().map(as_json).collect::<Vec<_>>();
        for payload in &payloads {
            assert_payload_matches_contract(&contract, payload);
        }

        let commands = payloads
            .iter()
            .map(|payload| payload["command"].as_str().expect("command").to_owned())
            .collect::<BTreeSet<_>>();
        let states = payloads
            .iter()
            .map(|payload| payload["state"].as_str().expect("state").to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            commands,
            contract["fields"]["command"]["enum"]
                .as_array()
                .expect("command enum")
                .iter()
                .map(|value| value.as_str().expect("command value").to_owned())
                .collect()
        );
        assert_eq!(
            states,
            contract["fields"]["state"]["enum"]
                .as_array()
                .expect("state enum")
                .iter()
                .map(|value| value.as_str().expect("state value").to_owned())
                .collect()
        );

        assert_exit_contract(&contract, &payloads);
    }

    #[test]
    fn changed_actual_result_output_is_rejected_by_the_selected_contract() {
        let contract = expected_contract();
        let mut changed = as_json(&actual_outcomes().remove(0));
        changed["state"] = json!("changed");
        let failure =
            std::panic::catch_unwind(|| assert_payload_matches_contract(&contract, &changed));
        assert!(
            failure.is_err(),
            "changed as_json output must not satisfy v1"
        );
    }
}
