use std::process::Command;

#[test]
fn test_double_emits_a_log_and_combined_stdin() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let script = directory.path().join("double.json");
    let input = directory.path().join("input.json");
    std::fs::write(&script, "{}").expect("script writes");
    std::fs::write(&input, r#"{"version":1,"logs":[{}]}"#).expect("input writes");
    for args in [vec!["emit", "--log", "{}"], vec!["emit", "--stdin"]] {
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
    }
}
