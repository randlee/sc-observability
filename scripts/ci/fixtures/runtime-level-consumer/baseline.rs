use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::ServiceName;

fn main() {
    let config = LoggerConfig::default_for(
        ServiceName::new("bp2-baseline").unwrap(),
        PathBuf::from("logs"),
    );
    let logger = Logger::new(config).unwrap();
    let _ = logger.shutdown();
}
