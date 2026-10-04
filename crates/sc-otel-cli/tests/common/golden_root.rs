use std::path::PathBuf;

pub fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
}
