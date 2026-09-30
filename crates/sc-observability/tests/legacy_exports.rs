#![expect(
    deprecated,
    reason = "this fixture verifies the released 1.x error signatures"
)]
//! Compile-only 1.x root-surface proof over the shared canonical runtime.

use sc_observability::{
    EventError, FlushError, InitError, LogError, LogEvent, Logger, LoggerBuilder, LoggerConfig,
    Running, TryLogError,
};

#[test]
fn released_root_signatures_remain_available() {
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _: fn(LoggerConfig) -> Result<Logger<Running>, InitError> = Logger::new;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), LogError> = Logger::log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), TryLogError> = Logger::try_log;
    let _: for<'a> fn(&'a Logger, LogEvent) -> Result<(), EventError> = Logger::emit;
    let _: for<'a> fn(&'a Logger) -> Result<(), FlushError> = Logger::flush;
}
