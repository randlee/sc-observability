/// Validates the complete top-level sc-otel.result/v1 shape, retaining null keys.
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
