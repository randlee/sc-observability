use std::path::PathBuf;

use sc_observability::{LoggerBuilder, LoggerConfig, v2};
use sc_observability_types::ServiceName;

fn zero_capacity_config() -> LoggerConfig {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("released-builder-capacity").expect("static service name"),
        PathBuf::from("target/released-builder-capacity"),
    );
    config.queue_capacity = 0;
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config
}

#[test]
#[allow(
    deprecated,
    reason = "the test exercises the released root constructor"
)]
fn released_and_canonical_builders_reject_zero_capacity_before_sink_validation() {
    assert!(LoggerBuilder::new(zero_capacity_config()).is_err());
    assert!(LoggerBuilder::new_typed(zero_capacity_config()).is_err());
    assert!(v2::LoggerBuilder::new(zero_capacity_config()).is_err());
}

#[test]
fn valid_capacity_constructs_and_builds() {
    let mut config = zero_capacity_config();
    config.queue_capacity = 1;
    config.enable_console_sink = true;
    let builder = v2::LoggerBuilder::new(config).expect("valid capacity constructs");
    let _logger = builder.build().expect("valid capacity builds");
}
