use std::process::Command;

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
