#![deny(deprecated)]

use sc_observability::{Logger, LoggerBuilder};

fn main() {
    let _ = LoggerBuilder::new;
    let _ = Logger::builder;
    let _ = Logger::new;
    let _ = Logger::log;
    let _ = Logger::try_log;
    let _ = Logger::try_log_with_outcome;
    let _ = Logger::emit;
    let _ = Logger::flush;
}
