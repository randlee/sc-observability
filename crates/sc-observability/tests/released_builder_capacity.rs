use std::path::PathBuf;

use sc_observability::{LoggerBuilder, LoggerConfig, v2};
use sc_observability_types::DiagnosticInfo;
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
    let Err(error) = LoggerBuilder::new(zero_capacity_config()) else {
        panic!("released root builder must reject zero queue capacity");
    };
    assert_zero_capacity_diagnostic(error.0.diagnostic());

    let Err(error) = LoggerBuilder::new_typed(zero_capacity_config()) else {
        panic!("released typed builder must reject zero queue capacity");
    };
    assert_zero_capacity_diagnostic(error.diagnostic());

    let Err(error) = v2::LoggerBuilder::new(zero_capacity_config()) else {
        panic!("canonical builder must reject zero queue capacity");
    };
    assert_zero_capacity_diagnostic(error.diagnostic());
}

fn assert_zero_capacity_diagnostic(diagnostic: &sc_observability::Diagnostic) {
    assert_eq!(
        diagnostic.code.as_str(),
        "SC_OBSERVABILITY_LOGGER_INIT_FAILED"
    );
    assert_eq!(
        diagnostic.message,
        "logger queue capacity must be greater than zero"
    );
    assert_eq!(
        diagnostic.remediation,
        sc_observability::Remediation::recoverable(
            "set LoggerConfig.queue_capacity to a positive value before constructing the logger",
            ["increase queue_capacity to at least 1"],
        )
    );
}

#[test]
fn valid_capacity_constructs_and_builds() {
    let mut config = zero_capacity_config();
    config.queue_capacity = 1;
    config.enable_console_sink = true;
    let builder = v2::LoggerBuilder::new(config).expect("valid capacity constructs");
    let _logger = builder.build().expect("valid capacity builds");
}
