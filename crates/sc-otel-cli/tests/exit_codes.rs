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
    let result: serde_json::Value = serde_json::from_slice(&invalid.stdout).expect("result JSON");
    assert_eq!(result["exit_code"], 3);
}
