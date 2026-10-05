//! Shared YAML configuration loader; consumer-specific keys are ignored by serde.
use sc_observability_types::otlp::submission::{
    TelemetryConfigError, TelemetryFileConfig, error_codes,
};
use std::{io::Read, path::Path};
/// Loads core telemetry settings and records the directory for relative store paths.
///
/// An omitted `otlp.timeout_ms` remains unset here. Resolving that file through
/// `resolve_config` applies the submission entry point's 10-second default;
/// the released `OtelConfig` facade continues to default to 3 seconds.
/// # Errors
/// Returns `ConfigFile` for an unreadable file or invalid YAML, without logging its contents.
pub fn load_telemetry_file(path: &Path) -> Result<TelemetryFileConfig, TelemetryConfigError> {
    let failure = |cause: &str| TelemetryConfigError::ConfigFile {
        path: path.to_path_buf(),
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
            &format!("cannot load telemetry YAML: {cause}"),
        ),
    };
    // Reject special files before opening: a FIFO must not block startup.
    if !std::fs::metadata(path)
        .map_err(|error| failure(&format!("filesystem {:?}", error.kind())))?
        .is_file()
    {
        return Err(failure("path is not a regular file"));
    }
    let mut yaml = String::new();
    std::fs::File::open(path)
        .map_err(|error| failure(&format!("filesystem {:?}", error.kind())))?
        .take(crate::constants::TELEMETRY_CONFIG_MAX_BYTES + 1)
        .read_to_string(&mut yaml)
        .map_err(|error| failure(&format!("read {:?}", error.kind())))?;
    if u64::try_from(yaml.len()).unwrap_or(u64::MAX) > crate::constants::TELEMETRY_CONFIG_MAX_BYTES
    {
        return Err(failure(
            "file exceeds the telemetry configuration size limit",
        ));
    }
    let mut config: TelemetryFileConfig =
        serde_saphyr::from_str(&yaml).map_err(|error: serde_saphyr::Error| {
            let reason = error.location().map_or_else(
                || "invalid YAML configuration".to_owned(),
                |location| {
                    format!(
                        "invalid YAML at line {}, column {}",
                        location.line(),
                        location.column()
                    )
                },
            );
            failure(&reason)
        })?;
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

    #[test]
    fn telemetry_yaml_loader_preserves_unknown_consumer_keys_and_existing_rejections() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("telemetry.yaml");
        std::fs::write(
            &path,
            "service: schema-test\nconsumer_extension: keep-me\notlp:\n  endpoint: http://collector:4318\nstore:\n  path: relative.db\n  consumer_extension: keep-me\n",
        )
        .unwrap();
        let loaded = load_telemetry_file(&path).unwrap();
        let overrides = ConfigOverrides::default();
        let resolved =
            resolve_config(ConfigSources::new(&overrides, Some(&loaded), &|_| None)).unwrap();
        assert_eq!(resolved.store_path, directory.path().join("relative.db"));
        assert_eq!(resolved.request_timeout, std::time::Duration::from_secs(10));

        std::fs::write(&path, "store: not-a-mapping\n").unwrap();
        assert!(matches!(
            load_telemetry_file(&path),
            Err(TelemetryConfigError::ConfigFile { .. })
        ));
    }
}
