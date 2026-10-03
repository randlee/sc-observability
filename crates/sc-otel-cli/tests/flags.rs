use std::process::Command;
#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
#[path = "common/d29_fixtures.rs"]
mod d29_fixtures;
#[path = "common/d29_system_generated_fields.rs"]
mod d29_system_generated_fields;
#[cfg(feature = "test-double")]
#[path = "common/fixture_component.rs"]
mod fixture_component;
#[path = "common/golden_root.rs"]
mod golden_root;
#[path = "common/mask_generated.rs"]
mod mask_generated;
mod common {
    pub use super::assert_result_v1::assert_result_v1;
    pub use super::d29_fixtures::D29_FIXTURES;
    #[cfg(feature = "test-double")]
    pub use super::fixture_component::fixture_component;
    pub use super::golden_root::golden_root;
    pub use super::mask_generated::mask_generated;
}
use common::assert_result_v1;

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
fn recorded_run(
    args: &[&str],
    endpoint: Option<&str>,
) -> (
    serde_json::Value,
    Vec<serde_json::Value>,
    Option<serde_json::Value>,
) {
    let directory = tempfile::tempdir().expect("record directory");
    let record = directory.path().join("record.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
    command
        .args(args)
        .env_remove("SC_OTEL_TEST_DOUBLE")
        .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT")
        .env("SC_OTEL_TEST_DOUBLE_RECORD", &record);
    if let Some(endpoint) = endpoint {
        command.env("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint);
    }
    let output = command.output().expect("CLI runs");
    assert!(output.status.success(), "{output:?}");
    let name = args
        .iter()
        .find(|arg| matches!(**arg, "emit" | "flush" | "status" | "validate"))
        .expect("subcommand");
    let result = assert_result_v1(&output.stdout, name);
    let calls = std::fs::read_to_string(record.with_extension("calls.jsonl"))
        .expect("calls read")
        .lines()
        .map(|line| serde_json::from_str(line).expect("call JSON"))
        .collect();
    let envelope = std::fs::read(record)
        .ok()
        .map(|bytes| serde_json::from_slice(&bytes).expect("envelope JSON"));
    (result, calls, envelope)
}

#[cfg(feature = "test-double")]
#[test]
fn documented_global_and_repeatable_flags_reach_the_command_contract() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fragment = directory.path().join("log.json");
    let bytes =
        br#"{"body":{"kind":"string","data":"file-specific payload"},"event_name":"from-file"}"#;
    std::fs::write(&fragment, bytes).expect("fragment writes");
    let store = directory.path().join("explicit.sqlite");
    let store = store.to_str().expect("store path");
    let file = format!("@{}", fragment.display());
    let (result, calls, envelope) = recorded_run(
        &[
            "--store",
            store,
            "--endpoint",
            "http://localhost:54321",
            "emit",
            "--record-key",
            "flag-record",
            "--log",
            &file,
        ],
        None,
    );
    assert_eq!(calls[0]["endpoint"], "http://localhost:54321");
    assert_eq!(calls[0]["store_path"], store);
    assert_eq!(result["receipt"]["record_key"], "flag-record");
    let supplied: serde_json::Value = serde_json::from_slice(bytes).expect("file JSON");
    let record = &envelope.expect("recorded envelope")["logs"][0]["record"];
    for (key, value) in supplied.as_object().expect("fragment object") {
        assert_eq!(&record[key], value, "file field {key}");
    }
}

#[cfg(feature = "test-double")]
#[test]
fn flag_table_success_paths_cover_fragments_record_keys_and_repeatable_status_queries() {
    let combined = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args([
            "validate",
            "--log",
            "{}",
            "--span",
            &common::fixture_component("traces", "spans"),
            "--metric",
            &common::fixture_component("metric_gauge", "metrics"),
            "--profile",
            &common::fixture_component("profiles", "profiles"),
        ])
        .output()
        .expect("combined flags run");
    assert!(combined.status.success(), "{combined:?}");
    let combined = assert_result_v1(&combined.stdout, "validate");
    for signal in ["logs", "spans", "metrics"] {
        assert_eq!(
            combined["envelope"][signal]
                .as_array()
                .expect("signal array")
                .len(),
            1
        );
    }
    assert!(combined["envelope"]["profiles"].is_object());
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    let store = store.to_str().expect("store path");
    let (_, explicit, _) = recorded_run(&["--store", store, "flush", "--timeout", "0"], None);
    let (_, default, _) = recorded_run(&["--store", store, "flush"], None);
    assert_eq!(explicit[1]["method"], "flush");
    assert_eq!(explicit[1]["deadline_ms"], 0);
    assert!(default[1]["deadline_ms"].as_u64().expect("deadline") > 0);
    let (_, selected, _) = recorded_run(
        &[
            "--store",
            store,
            "status",
            "--record-key",
            "K",
            "--record-key",
            "J",
        ],
        None,
    );
    let (_, summary, _) = recorded_run(&["--store", store, "status"], None);
    assert_eq!(
        selected[1]["query"],
        serde_json::json!({"kind":"record_keys","keys":["K","J"]})
    );
    assert_eq!(summary[1]["query"], serde_json::json!({"kind":"summary"}));
}

#[cfg(feature = "test-double")]
#[test]
fn config_environment_is_resolved_for_a_valid_test_double_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    let args = ["--store", store.to_str().expect("store path"), "flush"];
    let (_, set, _) = recorded_run(&args, Some("http://localhost:54322"));
    let (_, unset, _) = recorded_run(&args, None);
    assert_eq!(set[0]["endpoint"], "http://localhost:54322");
    assert_ne!(unset[0]["endpoint"], set[0]["endpoint"]);
}

#[test]
fn equivalent_flag_and_stdin_input_produce_the_same_envelope_field_by_field() {
    for fixture in common::D29_FIXTURES {
        let path = common::golden_root().join(fixture);
        if !path.join("expected.envelope.json").is_file() {
            continue;
        }
        let input: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.join("input.json")).expect("input"))
                .expect("JSON");
        let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        command.arg("validate");
        for (key, flag) in [
            ("logs", "--log"),
            ("spans", "--span"),
            ("metrics", "--metric"),
        ] {
            if let Some(records) = input[key].as_array() {
                for record in records {
                    command.arg(flag).arg(record.to_string());
                }
            }
        }
        if !input["profiles"].is_null() {
            command.arg("--profile").arg(input["profiles"].to_string());
        }
        let flags = command.output().expect("flags run");
        let stdin = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(["validate", "--stdin"])
            .stdin(std::fs::File::open(path.join("input.json")).expect("input opens"))
            .output()
            .expect("stdin runs");
        assert!(flags.status.success(), "{fixture}: {flags:?}");
        assert!(stdin.status.success(), "{fixture}: {stdin:?}");
        let mut flags = assert_result_v1(&flags.stdout, "validate")["envelope"].clone();
        let mut stdin = assert_result_v1(&stdin.stdout, "validate")["envelope"].clone();
        common::mask_generated(&mut flags, &input);
        common::mask_generated(&mut stdin, &input);
        assert_eq!(flags, stdin, "{fixture}: every non-generated field");
    }
}
