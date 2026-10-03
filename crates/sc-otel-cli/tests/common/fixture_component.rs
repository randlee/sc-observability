use std::fs;

pub fn fixture_component(name: &str, field: &str) -> String {
    let input: serde_json::Value = serde_json::from_slice(
        &fs::read(
            crate::golden_root::golden_root()
                .join(name)
                .join("input.json"),
        )
        .expect("fixture reads"),
    )
    .expect("fixture JSON");
    input[field]
        .as_array()
        .and_then(|values| values.first())
        .unwrap_or(&input[field])
        .clone()
        .to_string()
}
