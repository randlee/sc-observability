//! Compile-only proof that the v2 facade owns the clean canonical signatures.

use std::path::PathBuf;
use std::time::Duration;

use sc_observability::v2::{
    AdmissionOutcome, EventError, FlushError, InitError, LevelOwner, LogEvent, LogQuery,
    LogSnapshot, Logger, LoggerBuilder, LoggerConfig, QueryError, ServiceName, ShutdownError,
};

fn requires_send_sync<T: Send + Sync>() {}

#[test]
fn v2_logger_exposes_clean_canonical_signatures() {
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = Logger::builder;
    let _: fn(LoggerConfig) -> Result<Logger, InitError> = Logger::new;
    let _: fn(LoggerConfig) -> Result<(Logger, LevelOwner), InitError> =
        Logger::new_with_level_owner;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), EventError> = Logger::log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), EventError> = Logger::try_log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<AdmissionOutcome, EventError> =
        Logger::try_log_with_outcome;
    let _: for<'a> fn(&'a Logger) -> Result<(), FlushError> = Logger::flush;
    let _: for<'a> fn(&'a Logger, Duration) -> Result<(), FlushError> = Logger::flush_with_timeout;
    let _: for<'a> fn(&'a Logger, &'a LogQuery) -> Result<LogSnapshot, QueryError> = Logger::query;
    let _: for<'a> fn(&'a Logger) -> Result<(), ShutdownError> = Logger::shutdown;
    let _: for<'a> fn(&'a Logger, Duration) -> Result<(), ShutdownError> =
        Logger::shutdown_with_timeout;
    let _: for<'a> fn(&'a Logger) -> &'a ServiceName = Logger::service_name;
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _: fn(LoggerBuilder) -> Result<Logger, InitError> = LoggerBuilder::build;
    let _: fn(ServiceName, PathBuf) -> LoggerConfig = LoggerConfig::default_for;

    requires_send_sync::<Logger>();
    requires_send_sync::<LoggerBuilder>();
}
