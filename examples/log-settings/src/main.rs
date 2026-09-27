use std::path::PathBuf;

use sc_observability::{EnvSnapshot, LogSettings, LogSettingsInputs};
use sc_observability_types::{EnvPrefix, ServiceName};
use serde::Deserialize;

#[derive(Deserialize)]
struct ApplicationConfig {
    #[serde(default)]
    logging: Option<LogSettings>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: ApplicationConfig = serde_json::from_str(r#"{"logging":{"level":"Info"}}"#)?;
    let snapshot = EnvSnapshot::capture();
    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: config.logging,
        shared_env: LogSettings::from_env(&snapshot, EnvPrefix::new("SC")?)?,
        application_env: Some(LogSettings::from_application_env(
            &snapshot,
            EnvPrefix::new("APP")?,
        )?),
        default_root: PathBuf::from("./var"),
    })?;
    let _logger_config = resolved.into_logger_config(ServiceName::new("example")?);
    Ok(())
}
