#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
use assert_result_v1::assert_result_v1;
use std::process::Command;

#[test]
fn usage_and_invalid_input_have_stable_exit_codes() {
    let usage = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .arg("--unknown")
        .output()
        .expect("binary runs");
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());

    let invalid = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["validate", "--log", "{"])
        .output()
        .expect("binary runs");
    assert_eq!(invalid.status.code(), Some(3));
    let result: serde_json::Value = assert_result_v1(&invalid.stdout, "validate");
    assert_eq!(result["schema"], "sc-otel.result/v1");
    assert_eq!(result["exit_code"], 3);
    assert_eq!(
        result["error"]["code"],
        "SC_OBSERVABILITY_SUBMIT_INVALID_JSON"
    );
    assert!(result["error"]["cause"].is_string());
}

#[test]
fn text_results_are_compact_and_errors_are_explained_on_stderr() {
    let valid = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["--output", "text", "validate", "--log", "{}"])
        .output()
        .expect("binary runs");
    assert!(valid.status.success(), "{valid:?}");
    assert_eq!(String::from_utf8_lossy(&valid.stdout), "validated exit=0\n");
    assert!(valid.stderr.is_empty());

    let invalid = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["--output", "text", "validate", "--log", "{"])
        .output()
        .expect("binary runs");
    assert_eq!(invalid.status.code(), Some(3));
    assert_eq!(
        String::from_utf8_lossy(&invalid.stdout),
        "rejected exit=3 error=SC_OBSERVABILITY_SUBMIT_INVALID_JSON\n"
    );
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid JSON for --log"));
}

#[cfg(feature = "test-double")]
#[test]
fn every_non_usage_exit_has_the_result_schema_and_expected_code() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    let missing_config = directory.path().join("missing.toml");
    let cases = [
        (0, vec!["validate", "--log", "{}"], None),
        (3, vec!["validate", "--log", "{"], None),
        (
            4,
            vec![
                "--config",
                missing_config.to_str().expect("UTF-8 path"),
                "flush",
            ],
            None,
        ),
        (
            5,
            vec!["emit", "--log", "{}"],
            Some(r#"{"admissions":[{"outcome":"reject","kind":"disk_bound_exceeded"}]}"#),
        ),
        (
            6,
            vec!["emit", "--log", "{}"],
            Some(r#"{"deliveries":[{"signal":"logs","outcome":"stall"}]}"#),
        ),
        (
            7,
            vec!["emit", "--log", "{}"],
            Some(r#"{"deliveries":[{"signal":"logs","outcome":"fail"}]}"#),
        ),
    ];

    for (expected_exit, args, script) in cases {
        let script_path = directory.path().join(format!("{expected_exit}.json"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        command.args(["--store", store.to_str().expect("UTF-8 store")]);
        let result_command = *args
            .iter()
            .find(|a| matches!(**a, "emit" | "validate" | "flush" | "status"))
            .expect("subcommand");
        command.args(&args);
        if let Some(script) = script {
            std::fs::write(&script_path, script).expect("script writes");
            command.env("SC_OTEL_TEST_DOUBLE", &script_path);
        }
        let output = command.output().expect("binary runs");
        assert_eq!(output.status.code(), Some(expected_exit), "{output:?}");
        let result: serde_json::Value = assert_result_v1(&output.stdout, result_command);
        assert_eq!(result["schema"], "sc-otel.result/v1");
        assert_eq!(result["exit_code"], expected_exit);
    }
}

#[test]
fn missing_store_path_is_a_config_exit_with_a_result_schema() {
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .arg("flush")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let result: serde_json::Value = assert_result_v1(&output.stdout, "flush");
    assert_eq!(result["schema"], "sc-otel.result/v1");
    assert_eq!(
        result["error"]["code"],
        "SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING"
    );
}

#[cfg(feature = "test-double")]
#[test]
fn delivery_failure_takes_precedence_and_config_diagnostics_redact_credentials() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let script = directory.path().join("precedence.json");
    let store = directory.path().join("store.sqlite");
    std::fs::write(
        &script,
        r#"{"deliveries":[{"signal":"logs","outcome":"fail"},{"signal":"metrics","outcome":"stall"}]}"#,
    )
    .expect("script writes");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            store.to_str().expect("UTF-8 store"),
            "emit",
            "--log",
            "{}",
        ])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .env("SC_OTEL_AUTH_HEADER", "secret-value")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert_result_v1(&output.stdout, "emit");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("secret-value"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-value"));
}

#[cfg(feature = "test-double")]
#[test]
fn status_and_flush_successes_have_the_zero_exit_contract() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    for args in [
        vec!["--store", store.to_str().expect("UTF-8 store"), "status"],
        vec!["--store", store.to_str().expect("UTF-8 store"), "flush"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(&args)
            .output()
            .expect("binary runs");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let result: serde_json::Value =
            assert_result_v1(&output.stdout, args.last().expect("command"));
        assert_eq!(result["exit_code"], 0);
    }
}
