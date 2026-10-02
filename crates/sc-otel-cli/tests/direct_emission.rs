#![cfg(feature = "test-double")]

use std::process::Command;
mod common;
use common::fixture_component;

#[test]
fn test_double_emits_every_signal_and_combined_stdin() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let script = directory.path().join("double.json");
    let input = directory.path().join("input.json");
    let record = directory.path().join("recorded-envelope.json");
    std::fs::write(&script, "{}").expect("script writes");
    let span = fixture_component("traces", "spans");
    let metric = fixture_component("metric_gauge", "metrics");
    let profile = fixture_component("profiles", "profiles");
    let combined = serde_json::json!({
        "version": 1,
        "logs": [{}],
        "spans": [serde_json::from_str::<serde_json::Value>(&span).expect("span JSON")],
        "metrics": [serde_json::from_str::<serde_json::Value>(&metric).expect("metric JSON")],
        "profiles": serde_json::from_str::<serde_json::Value>(&profile).expect("profile JSON"),
    });
    std::fs::write(&input, combined.to_string()).expect("input writes");
    for (args, expected_signal) in [
        (vec!["emit", "--log", "{}"], "logs"),
        (vec!["emit", "--span", &span], "traces"),
        (vec!["emit", "--metric", &metric], "metrics"),
        (vec!["emit", "--profile", &profile], "profiles"),
        (vec!["emit", "--stdin"], "logs"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        command
            .args(["--store", directory.path().to_str().expect("UTF-8 store")])
            .args(&args)
            .env("SC_OTEL_TEST_DOUBLE", &script)
            .env("SC_OTEL_TEST_DOUBLE_RECORD", &record);
        if command.get_args().any(|arg| arg == "--stdin") {
            command.stdin(std::fs::File::open(&input).expect("input opens"));
        }
        let output = command.output().expect("binary runs");
        assert!(output.status.success(), "{output:?}");
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("result JSON");
        assert_eq!(result["state"], "admitted_delivered");
        assert!(
            result["receipt"]["signals"]
                .as_array()
                .expect("receipt signals")
                .iter()
                .any(|signal| signal == expected_signal),
            "{result}"
        );
        let validate_args = args.iter().skip(1).copied().collect::<Vec<_>>();
        let mut validate = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        validate.args(["validate"]).args(&validate_args);
        if validate.get_args().any(|arg| arg == "--stdin") {
            validate.stdin(std::fs::File::open(&input).expect("input opens"));
        }
        let validated = validate.output().expect("validate runs");
        assert!(validated.status.success(), "{validated:?}");
        let validated: serde_json::Value =
            serde_json::from_slice(&validated.stdout).expect("validated JSON");
        let mut recorded: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record).expect("record reads"))
                .expect("recorded envelope JSON");
        let mut validated = validated["envelope"].clone();
        normalize_system_generated_fields(&mut recorded);
        normalize_system_generated_fields(&mut validated);
        assert_eq!(recorded, validated, "double records validated envelope");
    }
}

fn normalize_system_generated_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_system_generated_fields(value);
            }
        }
        serde_json::Value::Object(values) => {
            for (field, value) in values.iter_mut() {
                if matches!(field.as_str(), "observed_time" | "trace_id" | "span_id")
                    && value.is_string()
                {
                    *value = serde_json::Value::String("<system-generated>".into());
                } else {
                    normalize_system_generated_fields(value);
                }
            }
        }
        _ => {}
    }
}

#[test]
fn test_double_uses_default_script_when_script_path_is_unset() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            directory.path().to_str().expect("UTF-8 store"),
            "emit",
            "--no-flush",
            "--log",
            "{}",
        ])
        .env_remove("SC_OTEL_TEST_DOUBLE")
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    assert_eq!(result["state"], "admitted_pending");
    assert!(result["flush"].is_null());
}

#[test]
fn text_emit_reports_the_admitted_submission_for_pending_and_flushed_work() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    for extra in [vec!["--no-flush"], Vec::new()] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        command
            .args(["--output", "text", "--store"])
            .arg(&store)
            .args(["emit", "--log", "{}"])
            .args(extra);
        let output = command.output().expect("binary runs");
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).expect("text output");
        assert!(text.starts_with("admitted_"), "{text}");
        assert!(text.contains("exit=0 submission="), "{text}");
    }
}
