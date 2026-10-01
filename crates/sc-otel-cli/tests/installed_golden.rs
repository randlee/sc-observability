use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
        .join(name)
        .join("input.json")
}

#[test]
fn validate_matches_shared_golden_envelopes() {
    for name in ["logs", "traces", "metric_gauge", "profiles"] {
        let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(["validate", "--stdin"])
            .stdin(std::fs::File::open(fixture(name)).expect("fixture opens"))
            .output()
            .expect("binary runs");
        assert!(output.status.success(), "{name}: {output:?}");
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("result JSON");
        assert_eq!(result["schema"], "sc-otel.result/v1");
        assert_eq!(result["state"], "validated");
        assert!(result["envelope"].is_object());
    }
}
