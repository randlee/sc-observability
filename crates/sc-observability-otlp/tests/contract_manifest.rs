use serde_json::Value;
#[test]
fn durable_store_binding() {
    let output =
        std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
            .output()
            .expect("cargo metadata starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let package = packages
        .iter()
        .find(|p| p["name"] == "sc-observability-otlp")
        .unwrap();
    let features = package["features"].as_object().unwrap();
    let durable = features["durable-store"].as_array().unwrap();
    assert!(durable.contains(&Value::from("sync-http")));
    for name in ["rusqlite", "serde-saphyr", "uuid"] {
        let dep = package["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == name)
            .unwrap();
        assert_eq!(dep["optional"], true, "{name}");
        let activation = format!("dep:{name}");
        assert!(durable.contains(&Value::from(activation.clone())));
        for (feature, values) in features {
            if feature != "durable-store" {
                assert!(
                    !values
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v == &activation || v == name),
                    "{feature} activates {name}"
                );
            }
        }
    }
}
