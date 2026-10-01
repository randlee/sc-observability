//! Shared YAML configuration loader; consumer-specific keys are ignored by serde.
use sc_observability_types::otlp::submission::{
    TelemetryConfigError, TelemetryFileConfig, error_codes,
};
use std::path::Path;
/// Loads core telemetry settings and records the directory for relative store paths.
/// # Errors
/// Returns `ConfigFile` for an unreadable file or invalid YAML, without logging its contents.
pub fn load_telemetry_file(path: &Path) -> Result<TelemetryFileConfig, TelemetryConfigError> {
    let failure = || TelemetryConfigError::ConfigFile {
        path: path.to_path_buf(),
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
            "cannot load telemetry YAML; check the file path, permissions and syntax",
        ),
    };
    let yaml = std::fs::read_to_string(path).map_err(|_| failure())?;
    let mut config: TelemetryFileConfig = serde_saphyr::from_str(&yaml).map_err(|_| failure())?;
    config.base_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::otlp::submission::{
        ConfigOverrides, ConfigSources, resolve_config,
    };
    #[test]
    fn unknown_consumer_keys_relative_path_and_timeout() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/telemetry_yaml");
        let loaded = load_telemetry_file(&dir.join("full.yaml")).unwrap();
        let config = resolve_config(ConfigSources::new(
            &ConfigOverrides::default(),
            Some(&loaded),
            &|_| None,
        ))
        .unwrap();
        assert_eq!(config.service_name, "sanity");
        assert_eq!(config.store_path, dir.join("data/telemetry.db"));
        assert_eq!(
            config.request_timeout,
            std::time::Duration::from_millis(1234)
        );
        assert!(matches!(
            load_telemetry_file(&dir.join("malformed.yaml")),
            Err(TelemetryConfigError::ConfigFile { .. })
        ));
        assert!(matches!(
            load_telemetry_file(&dir.join("missing.yaml")),
            Err(TelemetryConfigError::ConfigFile { .. })
        ));
    }
}
