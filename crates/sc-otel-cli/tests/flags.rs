use std::process::Command;

#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
#[path = "common/d29_fixtures.rs"]
mod d29_fixtures;
#[path = "common/d29_system_generated_fields.rs"]
mod d29_system_generated_fields;
#[path = "common/golden_root.rs"]
mod golden_root;
#[path = "common/mask_generated.rs"]
mod mask_generated;

use assert_result_v1::assert_result_v1;
use d29_fixtures::D29_FIXTURES;
use golden_root::golden_root;
use mask_generated::mask_generated;

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
fn equivalent_flag_and_stdin_input_produce_the_same_envelope_field_by_field() {
    for fixture in D29_FIXTURES {
        let path = golden_root().join(fixture);
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
        mask_generated(&mut flags, &input);
        mask_generated(&mut stdin, &input);
        assert_eq!(flags, stdin, "{fixture}: every non-generated field");
    }
}
