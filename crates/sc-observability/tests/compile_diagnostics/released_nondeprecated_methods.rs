#![deny(deprecated)]

use sc_observability::{Logger, LoggerBuilder, LoggerConfig, Running};

fn main() {
    #[expect(deprecated, reason = "the released constructor remains deprecated")]
    let _ = LoggerBuilder::new;
    #[allow(deprecated, reason = "pin the released constructor signature")]
    let _: fn(LoggerConfig) -> _ = LoggerBuilder::new;

    let _: fn(LoggerBuilder) -> Logger<Running> = LoggerBuilder::build;
    let _ = Logger::new_with_level_owner;
}
