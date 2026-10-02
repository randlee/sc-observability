use sc_observability_types::Timestamp;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// D29's `SystemIds` contract generates only these fields when their input omits them.
const D29_SYSTEM_GENERATED_FIELDS: &[&str] = &["observed_time", "trace_id", "span_id"];

fn golden_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for segment in [
        "..",
        "sc-observability-types",
        "tests",
        "fixtures",
        "otlp_submission",
        "golden",
    ] {
        path.push(segment);
    }
    path
}

fn installed_binary(root: &Path) -> PathBuf {
    let status = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "install",
            "--locked",
            "--path",
            env!("CARGO_MANIFEST_DIR"),
            "--root",
        ])
        .arg(root)
        .args(["--force", "--debug"])
        .status()
        .expect("cargo install starts");
    assert!(status.success(), "cargo install succeeds");
    root.join("bin").join("sc-otel")
}

#[test]
fn installed_cli_matches_every_shared_golden_fixture() {
    let temp = tempfile::tempdir().expect("temporary installation directory");
    let binary = installed_binary(temp.path());
    let mut fixtures = fs::read_dir(golden_root())
        .expect("golden root reads")
        .map(|entry| entry.expect("fixture directory reads").path())
        .collect::<Vec<_>>();
    fixtures.sort();

    for fixture in fixtures {
        let input = fixture.join("input.json");
        let input_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&input).expect("fixture input reads"))
                .expect("fixture input JSON");
        let started = Timestamp::now_utc();
        let flags = fixture.join("flags.args");
        let mut command = Command::new(&binary);
        command.current_dir(temp.path()).arg("validate");
        if let Ok(args) = fs::read_to_string(&flags) {
            command.args(args.lines().filter(|arg| !arg.is_empty()));
        } else {
            command
                .arg("--stdin")
                .stdin(fs::File::open(&input).expect("fixture input opens"));
        }
        let output = command.output().expect("installed CLI runs");
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("installed CLI emits JSON");
        let finished = Timestamp::now_utc();

        if let Ok(expected) = fs::read(fixture.join("expected.envelope.json")) {
            assert!(output.status.success(), "{}: {output:?}", fixture.display());
            let expected: serde_json::Value =
                serde_json::from_slice(&expected).expect("expected envelope JSON");
            let mut envelope = result["envelope"].clone();
            assert_system_generated_fields(
                &mut envelope,
                &expected,
                &input_json,
                started,
                finished,
            );
            assert_eq!(envelope, expected, "{}", fixture.display());
        } else {
            let expected = fs::read(fixture.join("expected.error.json"))
                .expect("every invalid fixture has expected error JSON");
            let expected: serde_json::Value =
                serde_json::from_slice(&expected).expect("expected error JSON");
            assert_eq!(output.status.code(), Some(3), "{}", fixture.display());
            assert_eq!(
                result["error"]["code"],
                expected["code"],
                "{}",
                fixture.display()
            );
        }
    }
}

fn assert_system_generated_fields(
    envelope: &mut serde_json::Value,
    expected: &serde_json::Value,
    input: &serde_json::Value,
    started: Timestamp,
    finished: Timestamp,
) {
    let empty_input = Vec::new();
    assert_eq!(
        D29_SYSTEM_GENERATED_FIELDS,
        ["observed_time", "trace_id", "span_id"],
        "the D29-generated-field allowance must stay explicit and bounded"
    );
    validate_records(
        envelope["logs"].as_array_mut().expect("logs array"),
        expected["logs"].as_array().expect("expected logs array"),
        input["logs"].as_array().unwrap_or(&empty_input),
        started,
        finished,
        false,
    );
    validate_records(
        envelope["spans"].as_array_mut().expect("spans array"),
        expected["spans"].as_array().expect("expected spans array"),
        input["spans"].as_array().unwrap_or(&empty_input),
        started,
        finished,
        true,
    );
}

fn validate_records(
    records: &mut [serde_json::Value],
    expected: &[serde_json::Value],
    input: &[serde_json::Value],
    started: Timestamp,
    finished: Timestamp,
    span: bool,
) {
    for ((record, expected), input) in records.iter_mut().zip(expected).zip(input) {
        let record = record["record"]
            .as_object_mut()
            .expect("canonical record object");
        let expected = expected["record"]
            .as_object()
            .expect("expected record object");
        if !span && input.get("observed_time").is_none() {
            let actual = record
                .get("observed_time")
                .expect("generated observed_time is present")
                .clone();
            let timestamp: Timestamp =
                serde_json::from_value(actual).expect("generated observed_time is RFC 3339 UTC");
            assert!(
                started <= timestamp && timestamp <= finished,
                "generated observed_time falls within the installed invocation"
            );
            record.insert("observed_time".into(), expected["observed_time"].clone());
        }
        for (field, width) in [("trace_id", 32), ("span_id", 16)] {
            if input.get(field).is_none() && expected[field].is_string() {
                let actual = record[field]
                    .as_str()
                    .expect("generated identifier is present and string");
                assert_hex_identifier(actual, width, field);
                record.insert(field.into(), expected[field].clone());
            }
        }
    }
}

fn assert_hex_identifier(value: &str, width: usize, field: &str) {
    assert_eq!(value.len(), width, "generated {field} width");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "generated {field} is lowercase hexadecimal"
    );
}
