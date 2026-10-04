#![expect(
    deprecated,
    reason = "this fixture verifies the released 1.x error signatures"
)]
//! Compile-only 1.x root-surface proof over the shared canonical runtime.

use std::path::PathBuf;

use sc_observability::constants::MAX_LOG_EVENT_BYTES;
use sc_observability::{
    AdmissionOutcome, EventError, FlushError, InitError, LevelOwner, LevelState, LogError,
    LogEvent, LogFollowSession, LogQuery, LogSnapshot, Logger, LoggerBuilder, LoggerConfig,
    LoggingHealthReport, Running, SinkRegistration, Stopped, TryLogError,
};
use sc_observability_types::typed::{FlushFailure, InitFailure, LogFailure, TryLogFailure};
use sc_observability_types::{QueryError, ServiceName};

type RootLoggerWithOwner<E> = Result<(Logger<Running>, LevelOwner), E>;

#[test]
fn released_max_log_event_bytes_remains_one_mebibyte() {
    assert_eq!(MAX_LOG_EVENT_BYTES, 1024 * 1024);
}

#[test]
fn released_root_signatures_remain_available() {
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = Logger::builder;
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitFailure> = Logger::builder_typed;
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitFailure> = LoggerBuilder::new_typed;
    let _: for<'a> fn(&'a mut LoggerBuilder, SinkRegistration) -> &'a mut LoggerBuilder =
        LoggerBuilder::register_sink;
    let _: fn(LoggerBuilder) -> Logger<Running> = LoggerBuilder::build;
    let _: fn(LoggerBuilder) -> Result<Logger<Running>, InitFailure> = LoggerBuilder::build_typed;
    let _: fn(LoggerBuilder) -> RootLoggerWithOwner<InitError> =
        LoggerBuilder::build_with_level_owner;
    let _: fn(LoggerBuilder) -> RootLoggerWithOwner<InitFailure> =
        LoggerBuilder::build_with_level_owner_typed;

    let _: fn(LoggerConfig) -> Result<Logger<Running>, InitError> = Logger::new;
    let _: fn(LoggerConfig) -> Result<Logger<Running>, InitFailure> = Logger::new_typed;
    let _: fn(LoggerConfig) -> RootLoggerWithOwner<InitError> = Logger::new_with_level_owner;
    let _: fn(LoggerConfig) -> RootLoggerWithOwner<InitFailure> =
        Logger::new_with_level_owner_typed;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), LogError> = Logger::log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), LogFailure> = Logger::log_typed;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), TryLogError> = Logger::try_log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), TryLogFailure> = Logger::try_log_typed;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<AdmissionOutcome, TryLogError> =
        Logger::try_log_with_outcome;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<AdmissionOutcome, TryLogFailure> =
        Logger::try_log_with_outcome_typed;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), EventError> = Logger::emit;
    let _: for<'a> fn(&'a Logger) -> Result<(), FlushError> = Logger::flush;
    let _: for<'a> fn(&'a Logger) -> Result<(), FlushFailure> = Logger::flush_typed;
    let _: for<'a> fn(&'a Logger, &'a LogQuery) -> Result<LogSnapshot, QueryError> = Logger::query;
    let _: for<'a> fn(&'a Logger, LogQuery) -> Result<LogFollowSession, QueryError> =
        Logger::follow;
    let _: fn(Logger) -> Logger<Stopped> = Logger::shutdown;
    let _: for<'a> fn(&'a Logger) -> &'a ServiceName = Logger::service_name;
    let _: for<'a> fn(&'a Logger) -> LevelState = Logger::level_state;
    let _: for<'a> fn(&'a Logger) -> LoggingHealthReport = Logger::health;
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
