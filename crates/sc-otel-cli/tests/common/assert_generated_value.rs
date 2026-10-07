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
