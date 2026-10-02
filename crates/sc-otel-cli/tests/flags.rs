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
fn stdin_fragment_and_profile_conflicts_are_usage_errors() {
    for args in [
        vec!["emit", "--stdin", "--log", "{}"],
        vec!["emit", "--stdin", "--record-key", "id"],
        vec!["emit", "--log", "{}", "--profile", "{}", "--profile", "{}"],
        vec!["status", "--submission", "id", "--record-key", "key"],
        vec!["flush", "--timeout", "not-a-duration"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(args)
            .output()
            .expect("binary runs");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn documented_global_and_repeatable_flags_reach_the_command_contract() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fragment = directory.path().join("log.json");
    std::fs::write(&fragment, "{}").expect("fragment writes");
    let file_arg = format!("@{}", fragment.display());
    let missing_config = directory.path().join("missing.toml");
    let store = directory.path().join("store.sqlite");
    for args in [
        vec![
            "--config",
            missing_config.to_str().expect("UTF-8 config"),
            "--store",
            store.to_str().expect("UTF-8 store"),
            "--endpoint",
            "http://127.0.0.1:4318",
            "emit",
            "--log",
            &file_arg,
        ],
        vec![
            "--config",
            missing_config.to_str().expect("UTF-8 config"),
            "status",
            "--submission",
            "018f8f5e-5c4c-7abc-8def-0123456789ab",
            "--submission",
            "018f8f5e-5c4d-7abc-8def-0123456789ab",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(args)
            .output()
            .expect("binary runs");
        assert_eq!(output.status.code(), Some(4), "{output:?}");
    }
}

#[test]
fn flag_table_success_paths_cover_fragments_record_keys_and_repeatable_status_queries() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let log = directory.path().join("log.json");
    let span = directory.path().join("span.json");
    let metric = directory.path().join("metric.json");
    let profile = directory.path().join("profile.json");
    std::fs::write(&log, "{}").expect("log writes");
    std::fs::write(&span, fixture_component("traces", "spans")).expect("span writes");
    std::fs::write(&metric, fixture_component("metric_gauge", "metrics")).expect("metric writes");
    std::fs::write(&profile, fixture_component("profiles", "profiles")).expect("profile writes");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "validate",
            "--log",
            &format!("@{}", log.display()),
            "--span",
            &format!("@{}", span.display()),
            "--metric",
            &format!("@{}", metric.display()),
            "--profile",
            &format!("@{}", profile.display()),
        ])
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");

    let config = directory.path().join("missing.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--config",
            config.to_str().expect("UTF-8 config"),
            "status",
            "--submission",
            "018f8f5e-5c4c-7abc-8def-0123456789ab",
            "--submission",
            "018f8f5e-5c4d-7abc-8def-0123456789ab",
        ])
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(4));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    assert_eq!(
        result["error"]["code"],
        "SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE"
    );
}
