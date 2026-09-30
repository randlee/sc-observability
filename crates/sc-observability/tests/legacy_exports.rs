#![expect(
    deprecated,
    reason = "this fixture verifies the released 1.x error signatures"
)]
//! Compile-only 1.x root-surface proof over the shared canonical runtime.

use std::path::PathBuf;

use sc_observability::{
    EventError, FlushError, InitError, LogError, LogEvent, Logger, LoggerBuilder, LoggerConfig,
    Running, TryLogError,
};
use sc_observability_types::ServiceName;

#[test]
fn released_root_signatures_remain_available() {
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _: fn(LoggerConfig) -> Result<Logger<Running>, InitError> = Logger::new;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), LogError> = Logger::log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), TryLogError> = Logger::try_log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), EventError> = Logger::emit;
    let _: for<'a> fn(&'a Logger) -> Result<(), FlushError> = Logger::flush;
}

#[test]
fn released_logger_config_queue_capacity_accepts_literals_and_assignment() {
    let service = ServiceName::new("released-queue-capacity").expect("static service name");
    let mut config = LoggerConfig {
        queue_capacity: 8,
        ..LoggerConfig::default_for(service, PathBuf::from("target/released-queue-capacity"))
    };
    assert_eq!(config.queue_capacity, 8);

    config.queue_capacity = 16;
    assert_eq!(config.queue_capacity, 16);

    let _: Result<Logger<Running>, InitError> = Logger::new(config);
}

#[test]
fn released_logger_config_rejects_zero_queue_capacity_at_the_builder_boundary() {
    let service = ServiceName::new("released-zero-queue-capacity").expect("static service name");
    let mut config = LoggerConfig::default_for(
        service,
        PathBuf::from("target/released-zero-queue-capacity"),
    );
    config.queue_capacity = 0;

    assert!(Logger::new(config).is_err());
}
