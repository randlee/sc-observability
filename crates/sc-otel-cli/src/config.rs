//! Converts global CLI flags into the shared telemetry configuration.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed delivery diagnostics"
)]

use crate::cli::Cli;
use sc_observability_otlp::durable::load_telemetry_file;
use sc_observability_types::otlp::submission::{
    ConfigOverrides, ConfigSources, TelemetryClientConfig, TelemetryClientError, resolve_config,
};

pub(crate) fn resolve(cli: &Cli) -> Result<TelemetryClientConfig, TelemetryClientError> {
    let file = cli.config.as_deref().map(load_telemetry_file).transpose()?;
    let mut overrides = ConfigOverrides::default();
    #[cfg(test)]
    if let Some(unit_overrides) = &cli.unit_config_overrides {
        overrides.clone_from(unit_overrides);
    }
    overrides.store_path.clone_from(&cli.store);
    overrides.endpoint.clone_from(&cli.endpoint);
    let environment = |name: &str| std::env::var(name).ok();
    resolve_config(ConfigSources::new(&overrides, file.as_ref(), &environment)).map_err(Into::into)
}
