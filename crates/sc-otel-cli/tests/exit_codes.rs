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
