#[path = "common/assert_generated_value.rs"]
mod assert_generated_value;
#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
#[path = "common/d29_fixtures.rs"]
mod d29_fixtures;
#[path = "common/d29_system_generated_fields.rs"]
mod d29_system_generated_fields;
#[path = "common/golden_root.rs"]
mod golden_root;
use assert_generated_value::assert_generated_value;
use assert_result_v1::assert_result_v1;
use d29_fixtures::D29_FIXTURES;
use d29_system_generated_fields::D29_SYSTEM_GENERATED_FIELDS;
use golden_root::golden_root;
use sc_observability_types::Timestamp;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
    let fixture_root = golden_root();
    let mut fixtures = fs::read_dir(&fixture_root)
        .expect("golden root reads")
        .map(|entry| entry.expect("fixture directory reads").path())
        .filter(|path| path.join("input.json").is_file())
        .collect::<Vec<_>>();
    fixtures.sort();
    let scanned = fixtures
        .iter()
        .map(|path| {
            path.file_name()
                .expect("name")
                .to_str()
                .expect("UTF-8 name")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        scanned,
        D29_FIXTURES.iter().copied().collect(),
        "independent D29 fixture-name inventory"
    );
    assert_eq!(D29_FIXTURES.len(), 22);

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
        let result: serde_json::Value = assert_result_v1(&output.stdout, "validate");
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
    for &(signal, field) in D29_SYSTEM_GENERATED_FIELDS {
        let records = envelope[signal].as_array_mut().expect("signal records");
        for (index, record) in records.iter_mut().enumerate() {
            if input[signal][index][field].is_null()
                && expected[signal][index]["record"][field].is_string()
            {
                let actual = record["record"]
                    .get(field)
                    .expect("generated field present");
                assert_generated_value(field, actual, started, finished);
                record["record"][field] = expected[signal][index]["record"][field].clone();
            }
        }
    }
    for signal in ["metrics", "profiles"] {
        assert!(
            !D29_SYSTEM_GENERATED_FIELDS
                .iter()
                .any(|(owner, _)| *owner == signal)
        );
        assert_eq!(
            envelope[signal], expected[signal],
            "{signal} has no generated fields"
        );
    }
}
