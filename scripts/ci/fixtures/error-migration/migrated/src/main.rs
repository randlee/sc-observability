#![deny(deprecated)]

#[allow(
    deprecated,
    reason = "the matrix module is an explicit compatibility fixture for both adapter directions"
)]
mod compatibility_matrix;

use sc_observability::{Logger, LoggerBuilder, LoggerConfig};
use sc_observability_otlp::{
    AuthHeader, OtlpEndpoint, Telemetry, TelemetryConfigBuilder,
};
use sc_observability_types::typed::{ClassifiedError, InitFailureKind};
use sc_observability_types::{ServiceName, ToolName};
use sc_observe::{Observability, ObservabilityConfig};

fn main() {
    let root = std::env::temp_dir().join("sc-observability-b1e-migrated");
    let service = ServiceName::new("b1e-migrated").expect("valid service");
    let mut logger_config = LoggerConfig::default_for(service, root.clone());
    logger_config.enable_file_sink = false;
    logger_config.enable_console_sink = false;

    let mut builder_config = LoggerConfig::default_for(
        ServiceName::new("b1e-migrated-builder").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-migrated-builder"),
    );
    builder_config.enable_file_sink = false;
    builder_config.enable_console_sink = false;
    let builder = LoggerBuilder::new_typed(builder_config).expect("typed builder");
    let logger = builder.build_typed().expect("typed logger");
    logger.flush_typed().expect("typed logger flush");
    let logger = Logger::new_typed(logger_config).expect("typed logger constructor");
    logger.flush_typed().expect("typed logger flush");
    let _ = logger.shutdown();

    let mut owner_config = LoggerConfig::default_for(
        ServiceName::new("b1e-owner-legacy").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-owner-legacy"),
    );
    owner_config.enable_file_sink = false;
    owner_config.enable_console_sink = false;
    let (owner_logger, _owner) = Logger::new_with_level_owner(owner_config)
        .expect("supported owner constructor remains callable");
    let _ = owner_logger.shutdown();

    let mut typed_owner_config = LoggerConfig::default_for(
        ServiceName::new("b1e-owner-typed").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-owner-typed"),
    );
    typed_owner_config.enable_file_sink = false;
    typed_owner_config.enable_console_sink = false;
    let (typed_owner_logger, _typed_owner) = Logger::new_with_level_owner_typed(typed_owner_config)
        .expect("typed owner constructor");
    let _ = typed_owner_logger.shutdown();

    let mut owner_builder_config = LoggerConfig::default_for(
        ServiceName::new("b1e-builder-owner").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-builder-owner"),
    );
    owner_builder_config.enable_file_sink = false;
    owner_builder_config.enable_console_sink = false;
    let (builder_owner_logger, _builder_owner) = LoggerBuilder::new_typed(owner_builder_config)
        .expect("typed builder")
        .build_with_level_owner()
        .expect("supported builder owner constructor remains callable");
    let _ = builder_owner_logger.shutdown();

    let mut typed_builder_owner_config = LoggerConfig::default_for(
        ServiceName::new("b1e-builder-owner-typed").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-builder-owner-typed"),
    );
    typed_builder_owner_config.enable_file_sink = false;
    typed_builder_owner_config.enable_console_sink = false;
    let (typed_builder_owner_logger, _typed_builder_owner) =
        LoggerBuilder::new_typed(typed_builder_owner_config)
            .expect("typed builder")
            .build_with_level_owner_typed()
            .expect("typed builder owner constructor");
    let _ = typed_builder_owner_logger.shutdown();

    let config = ObservabilityConfig::default_for_typed(
        ToolName::new("b1e-migrated").expect("valid tool"),
        root,
    )
    .expect("typed observation config");
    let service_name = config.service_name_typed().expect("typed service accessor");
    assert!(
        Observability::builder(config).build_typed().is_err(),
        "empty route set is classified"
    );

    let endpoint = OtlpEndpoint::new_typed("https://otel.example.invalid").expect("typed endpoint");
    let _header = AuthHeader::new_typed("Bearer fixture").expect("typed auth header");
    let telemetry_config = TelemetryConfigBuilder::new(service_name)
        .build_typed()
        .expect("typed telemetry config");
    let telemetry = Telemetry::new_typed(telemetry_config).expect("typed telemetry");
    telemetry.flush_typed().expect("typed telemetry flush");
    telemetry.shutdown_typed().expect("typed telemetry shutdown");
    assert_eq!(endpoint.as_str(), "https://otel.example.invalid");

    let failure = sc_observability_types::typed::InitFailure::invalid_telemetry_config(
        "fixture classification",
        sc_observability_types::Remediation::recoverable(
            "inspect the config",
            ["compare the typed configuration fields"],
        ),
    );
    match failure.kind() {
        InitFailureKind::InvalidTelemetryConfig | InitFailureKind::Unclassified => {}
        _ => panic!("unexpected initialization classification"),
    }

    compatibility_matrix::run();
}
