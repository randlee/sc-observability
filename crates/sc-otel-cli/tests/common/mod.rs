use std::{fs, path::PathBuf};

pub fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
}

#[allow(dead_code, reason = "shared integration-test fixture accessor")]
pub fn fixture_component(name: &str, field: &str) -> String {
    let input: serde_json::Value = serde_json::from_slice(
        &fs::read(golden_root().join(name).join("input.json")).expect("fixture reads"),
    )
    .expect("fixture JSON");
    input[field]
        .as_array()
        .and_then(|values| values.first())
        .unwrap_or(&input[field])
        .clone()
        .to_string()
}

/// Reads the canonicalization contract instead of carrying a second literal list.
#[allow(dead_code, reason = "shared integration-test contract accessor")]
pub fn d29_system_generated_fields() -> Vec<&'static str> {
    let source = include_str!("../../../sc-observability-types/src/otlp/submission/canonical.rs");
    [
        ("observed_time", "ids.now()"),
        ("trace_id", "ids.trace_id()"),
        ("span_id", "ids.span_id()"),
    ]
    .into_iter()
    .filter_map(|(field, generation)| source.contains(generation).then_some(field))
    .collect()
}
