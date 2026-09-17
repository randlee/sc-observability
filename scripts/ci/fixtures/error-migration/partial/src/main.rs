#![deny(deprecated)]

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::ToolName;
use sc_observe::ObservabilityConfig;

#[allow(
    deprecated,
    reason = "the application keeps one explicitly documented legacy boundary during an incremental rollout"
)]
fn legacy_boundary(config: LoggerConfig) {
    let logger = Logger::new(config).expect("legacy boundary remains executable");
    logger.flush().expect("legacy boundary flush");
}

fn main() {
    let root = std::env::temp_dir().join("sc-observability-b1e-partial");
    let config = ObservabilityConfig::default_for_typed(
        ToolName::new("b1e-partial").expect("valid tool"),
        root.clone(),
    )
    .expect("typed config");
    let service = config.service_name_typed().expect("typed service");
    let mut logger_config = LoggerConfig::default_for(service, root);
    logger_config.enable_file_sink = false;
    logger_config.enable_console_sink = false;
    legacy_boundary(logger_config);
}
