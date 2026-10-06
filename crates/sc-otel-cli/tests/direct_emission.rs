use std::process::Command;

#[test]
fn text_emit_reports_the_admitted_submission_for_pending_and_flushed_work() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store.sqlite");
    for extra in [vec!["--no-flush"], Vec::new()] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
        command
            .args(["--output", "text", "--store"])
            .arg(&store)
            .args(["emit", "--log", "{}"])
            .args(extra);
        let output = command.output().expect("binary runs");
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).expect("text output");
        assert!(text.starts_with("admitted_"), "{text}");
        assert!(text.contains("exit=0 submission="), "{text}");
    }
}
