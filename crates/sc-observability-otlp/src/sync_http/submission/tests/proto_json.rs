use serde_json::Value;

/// Checks proto-JSON forms unsupported by the pinned protobuf serde decoder.
pub(super) fn assert_special_forms(bodies: &[Value]) {
    let text = bodies
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("traceId"),
        "binary identifiers are base64 encoded"
    );
    assert!(
        text.contains("stringValueStrindex") || text.contains("keyStrindex"),
        "string indices retained"
    );
    assert!(
        text.contains("NaN") && text.contains("Infinity") && text.contains("-Infinity"),
        "non-finite doubles retained"
    );
}
