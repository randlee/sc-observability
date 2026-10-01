use std::process::Command;

#[test]
fn stdin_fragment_and_profile_conflicts_are_usage_errors() {
    for args in [
        vec!["emit", "--stdin", "--log", "{}"],
        vec!["emit", "--log", "{}", "--profile", "{}", "--profile", "{}"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(args)
            .output()
            .expect("binary runs");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
