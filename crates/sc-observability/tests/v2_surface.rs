use std::sync::Arc;
use std::time::Duration;

use sc_observability::v2::{
    FlushError, LogQuery, LogSnapshot, Logger, LoggerConfig, QueryError, ServiceName,
    ShutdownError, SinkRegistration, SinkRegistrationError,
};

fn v2_only_imports(logger: &Logger, query: &LogQuery) -> Result<LogSnapshot, QueryError> {
    let _ = std::any::TypeId::of::<SinkRegistration>();
    let _ = std::any::TypeId::of::<SinkRegistrationError>();
    let _: Option<FlushError> = None;
    let _: Option<ShutdownError> = None;
    logger.query(query)
}

#[test]
fn v2_logger_shutdown_is_shared_and_idempotent_without_v1_imports() {
    let service = ServiceName::new("v2-surface-test").expect("valid service name");
    let mut config = LoggerConfig::default_for(
        service,
        std::env::temp_dir().join(format!("sc-observability-v2-{}", std::process::id())),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = true;
    let logger = Arc::new(Logger::new(config).expect("logger starts"));

    let shared_logger = logger.clone();
    std::thread::spawn(move || {
        shared_logger
            .shutdown_with_timeout(Duration::from_secs(1))
            .expect("shared shutdown succeeds");
    })
    .join()
    .expect("shutdown thread completes");

    logger.shutdown().expect("second shutdown is idempotent");
    assert!(logger.flush().is_err());
    let _ = v2_only_imports(&logger, &LogQuery::default());
}
