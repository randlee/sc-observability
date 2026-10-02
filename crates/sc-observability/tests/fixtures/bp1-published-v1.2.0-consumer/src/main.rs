use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::ServiceName;

fn main() {
    let config = LoggerConfig::default_for(
        ServiceName::new("published-baseline").expect("valid service name"),
        PathBuf::from("logs"),
    );
    #[expect(
        deprecated,
        reason = "legacy v1.2.0 compatibility fixture: proves deprecated Logger::new/builder still work"
    )]
    let logger = Logger::new(config).expect("legacy Logger::new remains available");
    let _stopped = logger.shutdown();

    let builder_config = LoggerConfig::default_for(
        ServiceName::new("published-builder").expect("valid service name"),
        PathBuf::from("logs"),
    );
    #[expect(
        deprecated,
        reason = "legacy v1.2.0 compatibility fixture: proves deprecated Logger::new/builder still work"
    )]
    let logger = Logger::builder(builder_config)
        .expect("legacy builder remains available")
        .build();
    let _stopped = logger.shutdown();

    let typed_builder_config = LoggerConfig::default_for(
        ServiceName::new("published-typed-builder").expect("valid service name"),
        PathBuf::from("logs"),
    );
    let logger = Logger::builder_typed(typed_builder_config)
        .expect("published typed builder remains available")
        .build_typed()
        .expect("published typed build remains available");
    let _stopped = logger.shutdown();

    let typed_owner_config = LoggerConfig::default_for(
        ServiceName::new("published-typed-owner").expect("valid service name"),
        PathBuf::from("logs"),
    );
    let (logger, _owner) = Logger::new_with_level_owner_typed(typed_owner_config)
        .expect("published typed owner constructor remains available");
    let _stopped = logger.shutdown();
}
