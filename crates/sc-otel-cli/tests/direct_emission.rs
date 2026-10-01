#![cfg(feature = "test-double")]

use std::process::Command;

fn fixture_component(name: &str, field: &str) -> String {
    let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
        .join(name)
        .join("input.json");
    let input: serde_json::Value =
        serde_json::from_slice(&std::fs::read(input).expect("fixture reads"))
            .expect("fixture JSON");
    input[field]
        .as_array()
        .and_then(|values| values.first())
        .unwrap_or(&input[field])
        .clone()
        .to_string()
}

#[test]
fn test_double_emits_every_signal_and_combined_stdin() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let script = directory.path().join("double.json");
    let input = directory.path().join("input.json");
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
            .args(args)
            .env("SC_OTEL_TEST_DOUBLE", &script);
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
