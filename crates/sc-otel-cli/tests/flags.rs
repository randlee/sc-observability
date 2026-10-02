use std::process::Command;
mod common;
use common::fixture_component;

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

#[cfg(feature = "test-double")]
#[test]
fn documented_global_and_repeatable_flags_reach_the_command_contract() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fragment = directory.path().join("log.json");
    let script = directory.path().join("double.json");
    let record = directory.path().join("record.json");
    std::fs::write(&fragment, "{}").expect("fragment writes");
    std::fs::write(&script, "{}").expect("double script writes");
    let file_arg = format!("@{}", fragment.display());
    let store = directory.path().join("store.sqlite");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            store.to_str().expect("UTF-8 store"),
            "--endpoint",
            "http://127.0.0.1:4318",
            "emit",
            "--record-key",
            "flag-record",
            "--log",
            &file_arg,
        ])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .env("SC_OTEL_TEST_DOUBLE_RECORD", &record)
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    assert_eq!(result["receipt"]["record_key"], "flag-record");
    assert!(
        record.is_file(),
        "the @file fragment reached the test double"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            store.to_str().expect("UTF-8 store"),
            "status",
            "--record-key",
            "flag-record",
        ])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("result JSON")["state"],
        "status"
    );
}

#[cfg(feature = "test-double")]
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

    let store = directory.path().join("store.sqlite");
    let script = directory.path().join("double.json");
    std::fs::write(&script, "{}").expect("double script writes");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            store.to_str().expect("UTF-8 store"),
            "status",
            "--record-key",
            "flag-record",
        ])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    assert_eq!(result["state"], "status");

    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "--store",
            store.to_str().expect("UTF-8 store"),
            "flush",
            "--timeout",
            "0",
        ])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("result JSON")["state"],
        "admitted_delivered"
    );
}

#[cfg(feature = "test-double")]
#[test]
fn config_environment_is_resolved_for_a_valid_test_double_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    let script = directory.path().join("double.json");
    std::fs::write(&script, "{}").expect("double script writes");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["--store", store.to_str().expect("UTF-8 store"), "flush"])
        .env("SC_OTEL_TEST_DOUBLE", &script)
        .env("SC_OTEL_AUTH_HEADER", "config-env-secret")
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !text.contains("config-env-secret"),
        "secret config is not rendered"
    );
}

#[test]
fn equivalent_flag_and_stdin_input_produce_the_same_envelope_field_by_field() {
    let stdin_path = common::golden_root().join("logs/input.json");
    let log = fixture_component("logs", "logs");
    let flags = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["validate", "--log", &log])
        .output()
        .expect("flag validation runs");
    let stdin = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["validate", "--stdin"])
        .stdin(std::fs::File::open(stdin_path).expect("fixture input opens"))
        .output()
        .expect("stdin validation runs");
    assert!(flags.status.success(), "{flags:?}");
    assert!(stdin.status.success(), "{stdin:?}");
    let mut flags: serde_json::Value = serde_json::from_slice::<serde_json::Value>(&flags.stdout)
        .expect("flag result JSON")["envelope"]
        .clone();
    let mut stdin: serde_json::Value = serde_json::from_slice::<serde_json::Value>(&stdin.stdout)
        .expect("stdin result JSON")["envelope"]
        .clone();
    normalize_system_generated_fields(&mut flags);
    normalize_system_generated_fields(&mut stdin);
    assert_eq!(
        flags, stdin,
        "equivalent inputs preserve every non-generated field"
    );
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
