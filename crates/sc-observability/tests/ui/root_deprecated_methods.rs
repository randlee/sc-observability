#![deny(deprecated)]

use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig, ServiceName};

fn main() {
    let service = ServiceName::new("deprecated-root").expect("static service name");
    let config = LoggerConfig::default_for(service, PathBuf::from("target/deprecated-root"));
    let logger = Logger::new(config).expect("logger");
    let _ = logger.flush();
}
