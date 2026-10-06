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
    output::{self, AdmittedState, Outcome, SuccessState},
};
use sc_observability_types::otlp::submission::{
    RecordKey, StatusQuery, SubmissionId, TelemetryClient, TelemetryClientConfig,
    TelemetryClientError,
};
use std::str::FromStr;

pub(crate) fn run(cli: &Cli) -> u8 {
    let format = cli.output;
    let outcome = execute(cli);
    let exit_code = outcome.exit_code;
    output::print(format, &outcome);
    exit_code
}

fn execute(cli: &Cli) -> Outcome {
    match &cli.command {
        Command::Validate(args) => validate(args),
        Command::Emit(args) => emit(cli, args),
        Command::Flush(args) => flush(cli, args),
        Command::Status(args) => status(cli, args),
    }
}

fn validate(args: &crate::cli::InputArgs) -> Outcome {
    match input::envelope(args, None) {
        Ok(envelope) => {
            let mut outcome =
                Outcome::success(constants::CommandName::Validate, SuccessState::Validated);
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
    let teardown = if args.no_flush {
        SessionTeardown::AdmissionOnly
    } else {
        SessionTeardown::Delivery
    };
    with_session(
        cli,
        constants::CommandName::Emit,
        teardown,
        |config, client| {
            let receipt = match client.emit(envelope) {
                Ok(receipt) => receipt,
                Err(error) => return failure(constants::CommandName::Emit, error),
            };
            if args.no_flush {
                return Outcome::admitted(
                    constants::CommandName::Emit,
                    receipt,
                    AdmittedState::Pending,
                );
            }
            flush_emission(client, receipt, config.emit_flush_deadline)
        },
    )
}

fn flush_emission(
    client: &dyn TelemetryClient,
    receipt: sc_observability_types::otlp::submission::AdmissionReceipt,
    deadline: std::time::Duration,
) -> Outcome {
    match client.flush_submission(&receipt.submission_id, deadline) {
        Ok(report) => Outcome::admitted(
            constants::CommandName::Emit,
            receipt,
            delivery_state(&report),
        )
        .with_flush(report),
        Err(error) => {
            let error: CliError = error.into();
            Outcome::admitted_failure(constants::CommandName::Emit, receipt, error)
        }
    }
}

fn flush(cli: &Cli, args: &FlushArgs) -> Outcome {
    with_session(
        cli,
        constants::CommandName::Flush,
        SessionTeardown::Delivery,
        |config, client| {
            let deadline = args.timeout.unwrap_or(config.flush_deadline);
            match client.flush(deadline) {
                Ok(report) => Outcome::success(
                    constants::CommandName::Flush,
                    SuccessState::Delivery(delivery_state(&report)),
                )
                .with_flush(report),
                Err(error) => failure(constants::CommandName::Flush, error),
            }
        },
    )
}

fn status(cli: &Cli, args: &StatusArgs) -> Outcome {
    let query = match status_query(args) {
        Ok(query) => query,
        Err(error) => return failure(constants::CommandName::Status, error),
    };
    with_session(
        cli,
        constants::CommandName::Status,
        SessionTeardown::Delivery,
        |_, client| match client.status(query) {
            Ok(status) => Outcome::success(constants::CommandName::Status, SuccessState::Status)
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
    teardown: SessionTeardown,
    action: impl FnOnce(&TelemetryClientConfig, &dyn TelemetryClient) -> Outcome,
) -> Outcome {
    let config = match config::resolve(cli) {
        Ok(config) => config,
        Err(error) => return failure(command, error),
    };
    let client = match client::open_client(
        config.clone(),
        #[cfg(test)]
        cli.unit_client_paths.as_ref(),
    ) {
        Ok(client) => client,
        Err(error) => return failure(command, error),
    };
    let outcome = action(&config, client.as_ref());
    match teardown {
        SessionTeardown::Delivery => {
            finish_shutdown(command, client.as_ref(), config.flush_deadline, outcome)
        }
        // Dropping the durable client stops its workers without adding a delivery attempt.
        SessionTeardown::AdmissionOnly => outcome,
    }
}

#[derive(Clone, Copy)]
enum SessionTeardown {
    AdmissionOnly,
    Delivery,
}

fn failure(command: constants::CommandName, error: impl Into<CliError>) -> Outcome {
    Outcome::failure(command, error.into())
}

fn delivery_state(report: &sc_observability_types::otlp::submission::FlushReport) -> AdmittedState {
    if report.failed.total() > 0 || report.evicted.total() > 0 {
        AdmittedState::Failed
    } else if report.still_pending.total() > 0 {
        AdmittedState::Pending
    } else {
        AdmittedState::Delivered
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
    use crate::client::UnitClientPaths;
    use clap::Parser;
    use sc_observability_types::otlp::submission::{
        AdmissionError, AdmissionReceipt, ConfigOverrides, ConfigSources, FlushReport, Secret,
        Signal, StatusQuery, StoreStatus, SubmissionEnvelope, SubmissionId, resolve_config,
    };
    use sc_observability_types::{ErrorCode, ErrorContext, Remediation, Timestamp};
    use std::{path::PathBuf, sync::Mutex, time::Duration};

    fn test_cli(args: &[String], paths: UnitClientPaths) -> Cli {
        let mut cli = Cli::try_parse_from(args).expect("test CLI arguments parse");
        cli.unit_client_paths = Some(paths);
        cli
    }

    fn recorded_run(
        directory: &std::path::Path,
        command: &[String],
        script: Option<&str>,
    ) -> (u8, Vec<serde_json::Value>, Option<serde_json::Value>) {
        let store = directory.join("store.sqlite");
        let record = directory.join("record.json");
        let script = script.map(|contents| {
            let path = directory.join("script.json");
            std::fs::write(&path, contents).expect("script writes");
            path
        });
        let args = std::iter::once("sc-otel".to_owned())
            .chain(["--store".to_owned(), store.display().to_string()])
            .chain(command.iter().cloned())
            .collect::<Vec<_>>();
        let exit_code = run(&test_cli(
            &args,
            UnitClientPaths {
                script,
                record: Some(record.clone()),
            },
        ));
        let calls = std::fs::read_to_string(record.with_extension("calls.jsonl"))
            .ok()
            .map(|calls| {
                calls
                    .lines()
                    .map(|line| serde_json::from_str(line).expect("recorded call JSON"))
                    .collect()
            })
            .unwrap_or_default();
        let envelope = std::fs::read(record)
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes).expect("recorded envelope JSON"));
        (exit_code, calls, envelope)
    }

    fn fixture_component(name: &str, field: &str) -> String {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
            .join(name)
            .join("input.json");
        let input: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture).expect("fixture reads"))
                .expect("fixture JSON");
        input[field]
            .as_array()
            .and_then(|values| values.first())
            .unwrap_or(&input[field])
            .to_string()
    }

    fn test_stdin(cli: &mut Cli, input: &serde_json::Value) {
        let Command::Emit(args) = &mut cli.command else {
            panic!("test stdin is only supported for emit");
        };
        args.input.stdin_input = Some(input.to_string());
    }

    #[test]
    fn run_uses_explicit_unit_client_paths_without_environment_mutation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let script = directory.path().join("double.json");
        let record = directory.path().join("record.json");
        std::fs::write(&script, "{}").expect("script writes");
        let store = directory.path().join("store.sqlite");
        let cli = test_cli(
            &[
                "sc-otel",
                "--store",
                store.to_str().expect("UTF-8 store"),
                "emit",
                "--log",
                "{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
            UnitClientPaths {
                script: Some(script),
                record: Some(record.clone()),
            },
        );

        assert_eq!(run(&cli), constants::EXIT_OK);
        assert!(
            record.is_file(),
            "the test-only client records its envelope"
        );
        assert!(record.with_extension("calls.jsonl").is_file());
    }

    #[test]
    fn scripted_unit_client_preserves_admission_and_delivery_exit_codes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = directory.path().join("store.sqlite");
        for (name, script, expected) in [
            (
                "rejected",
                r#"{"admissions":[{"outcome":"reject","kind":"disk_bound_exceeded"}]}"#,
                constants::EXIT_ADMISSION,
            ),
            (
                "pending",
                r#"{"deliveries":[{"signal":"logs","outcome":"stall"}]}"#,
                constants::EXIT_DELIVERY_PENDING,
            ),
            (
                "failed",
                r#"{"deliveries":[{"signal":"logs","outcome":"fail"}]}"#,
                constants::EXIT_DELIVERY_FAILED,
            ),
        ] {
            let script_path = directory.path().join(format!("{name}.json"));
            std::fs::write(&script_path, script).expect("script writes");
            let cli = test_cli(
                &[
                    "sc-otel".to_owned(),
                    "--store".to_owned(),
                    store.display().to_string(),
                    "emit".to_owned(),
                    "--log".to_owned(),
                    "{}".to_owned(),
                ],
                UnitClientPaths {
                    script: Some(script_path),
                    record: None,
                },
            );
            assert_eq!(run(&cli), expected, "{name}");
        }
    }

    #[test]
    fn scripted_unit_client_redacts_explicit_credentials_from_serialized_outcomes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let secret = "Bearer regression-secret";
        let script = directory.path().join("delivery.json");
        std::fs::write(
            &script,
            r#"{"deliveries":[{"signal":"logs","outcome":"fail"},{"signal":"metrics","outcome":"stall"}]}"#,
        )
        .expect("script writes");
        let mut cli = test_cli(
            &[
                "sc-otel".to_owned(),
                "--store".to_owned(),
                directory.path().join("store.sqlite").display().to_string(),
                "emit".to_owned(),
                "--log".to_owned(),
                "{}".to_owned(),
                "--metric".to_owned(),
                fixture_component("metric_gauge", "metrics"),
            ],
            UnitClientPaths {
                script: Some(script),
                record: None,
            },
        );
        let mut overrides = ConfigOverrides::default();
        overrides.auth_header = Some(Secret::new(secret.to_owned()));
        cli.unit_config_overrides = Some(overrides);

        assert_eq!(
            config::resolve(&cli)
                .expect("resolved telemetry config")
                .auth_header
                .as_ref()
                .expect("explicit credential")
                .expose(),
            secret
        );
        let outcome = execute(&cli);

        assert_eq!(outcome.exit_code, constants::EXIT_DELIVERY_FAILED);
        assert!(
            !output::as_json(&outcome).to_string().contains(secret),
            "serialized result must never reveal configured credentials"
        );
    }

    #[test]
    fn unit_client_covers_direct_signal_arguments_and_the_default_script() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let span = fixture_component("traces", "spans");
        let metric = fixture_component("metric_gauge", "metrics");
        let profile = fixture_component("profiles", "profiles");
        for (arguments, expected_field) in [
            (
                vec!["emit".to_owned(), "--log".to_owned(), "{}".to_owned()],
                "logs",
            ),
            (vec!["emit".to_owned(), "--span".to_owned(), span], "traces"),
            (
                vec!["emit".to_owned(), "--metric".to_owned(), metric],
                "metrics",
            ),
            (
                vec!["emit".to_owned(), "--profile".to_owned(), profile],
                "profiles",
            ),
        ] {
            let (exit_code, _, envelope) = recorded_run(directory.path(), &arguments, None);
            assert_eq!(exit_code, constants::EXIT_OK, "{expected_field}");
            assert!(
                !envelope
                    .expect("recorded envelope")
                    .get(match expected_field {
                        "traces" => "spans",
                        field => field,
                    })
                    .expect("signal field")
                    .is_null(),
                "{expected_field} reaches the unit-only client"
            );
        }

        let (exit_code, _, _) = recorded_run(
            directory.path(),
            &[
                "emit".to_owned(),
                "--no-flush".to_owned(),
                "--log".to_owned(),
                "{}".to_owned(),
            ],
            None,
        );
        assert_eq!(exit_code, constants::EXIT_OK);
    }

    #[test]
    fn unit_client_covers_combined_stdin_without_mutating_process_environment() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let script = directory.path().join("script.json");
        let record = directory.path().join("record.json");
        std::fs::write(&script, "{}").expect("script writes");
        let span = serde_json::from_str::<serde_json::Value>(&fixture_component("traces", "spans"))
            .expect("span JSON");
        let metric = serde_json::from_str::<serde_json::Value>(&fixture_component(
            "metric_gauge",
            "metrics",
        ))
        .expect("metric JSON");
        let profile =
            serde_json::from_str::<serde_json::Value>(&fixture_component("profiles", "profiles"))
                .expect("profile JSON");
        let args = [
            "sc-otel".to_owned(),
            "--store".to_owned(),
            directory.path().join("store.sqlite").display().to_string(),
            "emit".to_owned(),
            "--stdin".to_owned(),
        ];
        let mut cli = test_cli(
            &args,
            UnitClientPaths {
                script: Some(script),
                record: Some(record.clone()),
            },
        );
        test_stdin(
            &mut cli,
            &serde_json::json!({
                "version": 1,
                "logs": [{}],
                "spans": [span],
                "metrics": [metric],
                "profiles": profile,
            }),
        );

        assert_eq!(run(&cli), constants::EXIT_OK);
        let envelope: serde_json::Value =
            serde_json::from_slice(&std::fs::read(record).expect("record reads"))
                .expect("recorded envelope JSON");
        for field in ["logs", "spans", "metrics", "profiles"] {
            assert!(
                !envelope[field].is_null(),
                "{field} reaches the unit-only client"
            );
        }
    }

    #[test]
    fn unit_client_preserves_documented_flags_and_status_flush_contracts() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let fragment = directory.path().join("log.json");
        let fragment_json =
            r#"{"body":{"kind":"string","data":"file-specific payload"},"event_name":"from-file"}"#;
        std::fs::write(&fragment, fragment_json).expect("fragment writes");
        let (exit_code, calls, envelope) = recorded_run(
            directory.path(),
            &[
                "--endpoint".to_owned(),
                "http://localhost:54321".to_owned(),
                "emit".to_owned(),
                "--record-key".to_owned(),
                "flag-record".to_owned(),
                "--log".to_owned(),
                format!("@{}", fragment.display()),
            ],
            None,
        );
        assert_eq!(exit_code, constants::EXIT_OK);
        assert_eq!(calls[0]["endpoint"], "http://localhost:54321");
        assert_eq!(
            envelope.expect("recorded envelope")["record_key"],
            "flag-record"
        );

        let (exit_code, calls, _) = recorded_run(
            directory.path(),
            &[
                "status".to_owned(),
                "--record-key".to_owned(),
                "K".to_owned(),
                "--record-key".to_owned(),
                "J".to_owned(),
            ],
            None,
        );
        assert_eq!(exit_code, constants::EXIT_OK);
        assert_eq!(
            calls.last().expect("status call")["query"],
            serde_json::json!({"kind":"record_keys","keys":["K","J"]})
        );

        let (exit_code, calls, _) = recorded_run(
            directory.path(),
            &["flush".to_owned(), "--timeout".to_owned(), "0".to_owned()],
            None,
        );
        assert_eq!(exit_code, constants::EXIT_OK);
        assert_eq!(
            calls
                .iter()
                .find(|call| call["method"] == "flush")
                .expect("flush call")["deadline_ms"],
            0
        );
    }

    #[test]
    fn unit_client_preserves_every_non_usage_exit_code_and_delivery_precedence() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let missing_config = directory.path().join("missing.toml");
        for (expected_exit, arguments, script) in [
            (
                constants::EXIT_OK,
                vec!["validate".to_owned(), "--log".to_owned(), "{}".to_owned()],
                None,
            ),
            (
                constants::EXIT_INVALID_INPUT,
                vec!["validate".to_owned(), "--log".to_owned(), "{".to_owned()],
                None,
            ),
            (
                constants::EXIT_CONFIG,
                vec![
                    "--config".to_owned(),
                    missing_config.display().to_string(),
                    "flush".to_owned(),
                ],
                None,
            ),
            (
                constants::EXIT_ADMISSION,
                vec!["emit".to_owned(), "--log".to_owned(), "{}".to_owned()],
                Some(r#"{"admissions":[{"outcome":"reject","kind":"disk_bound_exceeded"}]}"#),
            ),
            (
                constants::EXIT_DELIVERY_PENDING,
                vec!["emit".to_owned(), "--log".to_owned(), "{}".to_owned()],
                Some(r#"{"deliveries":[{"signal":"logs","outcome":"stall"}]}"#),
            ),
            (
                constants::EXIT_DELIVERY_FAILED,
                vec![
                    "emit".to_owned(),
                    "--log".to_owned(),
                    "{}".to_owned(),
                    "--metric".to_owned(),
                    fixture_component("metric_gauge", "metrics"),
                ],
                Some(
                    r#"{"deliveries":[{"signal":"logs","outcome":"fail"},{"signal":"metrics","outcome":"stall"}]}"#,
                ),
            ),
        ] {
            let (exit_code, _, _) = recorded_run(directory.path(), &arguments, script);
            assert_eq!(exit_code, expected_exit, "{arguments:?}");
        }
    }

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
            SuccessState::Delivery(AdmittedState::Delivered),
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
        assert_eq!(outcome.state().as_str(), "admitted_failed");
        assert!(outcome.receipt().is_some());
    }
}
