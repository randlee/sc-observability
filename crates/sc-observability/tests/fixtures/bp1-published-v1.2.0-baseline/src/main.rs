use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::ServiceName;

fn main() {
    let config = LoggerConfig::default_for(
        ServiceName::new("published-baseline").expect("valid service name"),
        PathBuf::from("logs"),
    );
    let logger = Logger::new(config).expect("legacy Logger::new remains available");
    let _stopped = logger.shutdown();

    let builder_config = LoggerConfig::default_for(
        ServiceName::new("published-builder").expect("valid service name"),
        PathBuf::from("logs"),
    );
    let logger = Logger::builder(builder_config)
        .expect("legacy builder remains available")
        .build();
    let _stopped = logger.shutdown();
}
