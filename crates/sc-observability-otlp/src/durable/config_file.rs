//! Core YAML loader signature staged for D33.
use sc_observability_types::otlp::submission::{
    TelemetryConfigError, TelemetryFileConfig, error_codes,
};
use std::path::Path;
/// Loads the shared telemetry configuration file.
/// Returns a typed configuration failure until D33 implements file loading.
pub fn load_telemetry_file(path: &Path) -> Result<TelemetryFileConfig, TelemetryConfigError> {
    Err(TelemetryConfigError::ConfigFile {
        path: path.to_path_buf(),
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
            "telemetry file loading is not implemented yet",
        ),
    })
}
