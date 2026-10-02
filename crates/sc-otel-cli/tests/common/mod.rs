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

/// The D29 validation checklist names the error/correlation fixtures and requires
/// one fixture per signal/metric point form (sprint-d-29, lines 1313–1326).
/// This inventory is independent of the directory scan.
#[allow(dead_code, reason = "shared integration-test contract inventory")]
pub const D29_FIXTURES: &[&str] = &[
    "logs",
    "traces",
    "profiles",
    "metric_gauge",
    "metric_sum",
    "metric_histogram",
    "metric_exponential_histogram",
    "metric_summary",
    "uint_over_i64_max",
    "correlation_conflict",
    "timing_conflict",
    "unsupported_version",
    "strindex_outside_profiles",
    "non_finite_doubles",
    "plain_attribute_map",
    "paired_log_span_generated_ids",
    "histogram_bucket_count_mismatch",
    "summary_quantile_out_of_range",
    "monotonic_sum_negative",
    "delta_empty_interval",
    "exponential_scale_out_of_range",
    "profile_index_out_of_range",
];

/// canonical.rs:238–262 generates observation time for logs and correlated IDs
/// for logs/spans; `canonical_span` generates missing span IDs. Metrics/profiles
/// generate no fields. Signal names here are canonical envelope keys.
#[allow(dead_code, reason = "shared integration-test contract inventory")]
pub const D29_SYSTEM_GENERATED_FIELDS: &[(&str, &str)] = &[
    ("logs", "observed_time"),
    ("logs", "trace_id"),
    ("logs", "span_id"),
    ("spans", "trace_id"),
    ("spans", "span_id"),
];

/// Validates the complete top-level sc-otel.result/v1 shape, retaining null keys.
#[allow(dead_code, reason = "shared integration-test result assertion")]
pub fn assert_result_v1(stdout: &[u8], command: &str) -> serde_json::Value {
    use serde_json::Value;
    use std::collections::BTreeSet;
    let result: Value = serde_json::from_slice(stdout).expect("one result JSON object");
    let object = result.as_object().expect("result object");
    let keys = [
        "schema",
        "command",
        "exit_code",
        "state",
        "receipt",
        "flush",
        "status",
        "envelope",
        "error",
    ];
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        keys.into_iter().collect()
    );
    assert_eq!(result["schema"], "sc-otel.result/v1");
    assert_eq!(result["command"].as_str(), Some(command));
    assert!(result["exit_code"].is_u64(), "integer exit_code");
    assert!(matches!(
        result["state"].as_str(),
        Some(
            "validated"
                | "admitted_delivered"
                | "admitted_pending"
                | "admitted_failed"
                | "rejected"
                | "status"
        )
    ));
    for key in ["receipt", "flush", "status", "envelope", "error"] {
        assert!(
            result[key].is_null() || result[key].is_object(),
            "{key}: nullable object"
        );
    }
    if command == "emit"
        && result["state"]
            .as_str()
            .expect("state")
            .starts_with("admitted_")
    {
        assert!(
            result["receipt"].is_object(),
            "admitted emit requires receipt"
        );
    }
    if command != "emit" {
        assert!(result["receipt"].is_null());
    }
    if command == "flush" && result["exit_code"] == 0 {
        assert!(result["flush"].is_object());
    }
    if result["state"] != "validated" {
        assert!(result["envelope"].is_null());
    }
    if result["state"] == "rejected" {
        assert!(result["receipt"].is_null());
    }
    if result["state"] == "validated" {
        assert!(result["envelope"].is_object());
    }
    if result["state"] == "status" {
        assert!(result["status"].is_object());
    }
    if result["exit_code"] == 0 {
        assert!(result["error"].is_null());
    } else {
        assert!(result["error"].is_object());
    }
    if let Some(receipt) = result["receipt"].as_object() {
        for key in ["submission_id", "admitted_at"] {
            assert!(receipt[key].is_string());
        }
        assert!(receipt.contains_key("record_key"));
        assert!(receipt["record_key"].is_null() || receipt["record_key"].is_string());
        assert!(receipt["duplicate"].is_boolean());
        assert!(
            receipt["signals"]
                .as_array()
                .expect("signal array")
                .iter()
                .all(Value::is_string)
        );
    }
    if let Some(error) = result["error"].as_object() {
        assert!(error["code"].is_string());
        assert!(error["message"].is_string());
        assert!(
            error.contains_key("cause") && (error["cause"].is_null() || error["cause"].is_string())
        );
        assert!(error.contains_key("remediation"));
    }
    result
}

#[allow(dead_code, reason = "shared integration-test canonical normalization")]
pub fn mask_generated(envelope: &mut serde_json::Value, input: &serde_json::Value) {
    for &(signal, field) in D29_SYSTEM_GENERATED_FIELDS {
        if let Some(records) = envelope[signal].as_array_mut() {
            for (index, record) in records.iter_mut().enumerate() {
                if input[signal][index][field].is_null() && record["record"][field].is_string() {
                    record["record"][field] = "<generated>".into();
                }
            }
        }
    }
}

#[allow(dead_code, reason = "shared generated-field format assertion")]
pub fn assert_generated_value(
    field: &str,
    value: &serde_json::Value,
    started: sc_observability_types::Timestamp,
    finished: sc_observability_types::Timestamp,
) {
    match field {
        "observed_time" => {
            let stamp: sc_observability_types::Timestamp =
                serde_json::from_value(value.clone()).expect("generated UTC RFC3339 time");
            assert!(value.as_str().expect("timestamp string").ends_with('Z'));
            assert!(
                started <= stamp && stamp <= finished,
                "generated observation time within invocation"
            );
        }
        "trace_id" | "span_id" => {
            let width = if field == "trace_id" { 32 } else { 16 };
            let value = value.as_str().expect("generated identifier string");
            assert_eq!(value.len(), width, "{field} width");
            assert!(
                value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
            assert!(value.bytes().any(|b| b != b'0'), "nonzero {field}");
        }
        _ => panic!("unhandled generated-field contract {field}"),
    }
}
