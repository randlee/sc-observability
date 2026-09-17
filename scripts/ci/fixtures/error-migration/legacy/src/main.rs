use sc_observability::{Logger, LoggerConfig};
use sc_observability_otlp::{
    AuthHeader, OtlpEndpoint, Telemetry, TelemetryConfigBuilder,
};
use sc_observability_types::{ErrorCode, ErrorContext, InitError, Remediation, ServiceName, ToolName};
use sc_observe::{Observability, ObservabilityConfig};

fn main() {
    let root = std::env::temp_dir().join("sc-observability-b1e-legacy");
    let service = ServiceName::new("b1e-legacy").expect("valid service");
    let mut logger_config = LoggerConfig::default_for(service, root.clone());
    logger_config.enable_file_sink = false;
    logger_config.enable_console_sink = false;

    let mut builder_config = LoggerConfig::default_for(
        ServiceName::new("b1e-legacy-builder").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-legacy-builder"),
    );
    builder_config.enable_file_sink = false;
    builder_config.enable_console_sink = false;
    let _builder = sc_observability::LoggerBuilder::new(builder_config)
        .expect("legacy builder remains usable with warnings");
    let logger = Logger::new(logger_config).expect("legacy logger remains usable with warnings");
    logger.flush().expect("legacy flush remains usable");
    let _ = logger.shutdown();

    let config = ObservabilityConfig::default_for(
        ToolName::new("b1e-legacy").expect("valid tool"),
        root,
    )
    .expect("legacy observation config");
    let _ = config.service_name().expect("legacy service accessor");
    let _ = Observability::new(config);

    let endpoint = OtlpEndpoint::new("https://otel.example.invalid").expect("legacy endpoint");
    let _header = AuthHeader::new("Bearer fixture").expect("legacy auth header");
    let telemetry_config = TelemetryConfigBuilder::new(
        ServiceName::new("b1e-legacy").expect("valid service"),
    )
    .build()
    .expect("legacy telemetry config");
    let telemetry = Telemetry::new(telemetry_config).expect("legacy telemetry");
    telemetry.flush().expect("legacy telemetry flush");
    telemetry.shutdown().expect("legacy telemetry shutdown");
    assert_eq!(endpoint.as_str(), "https://otel.example.invalid");

    let legacy = InitError(Box::new(ErrorContext::new(
        ErrorCode::new_static("SC_OBSERVABILITY_OTLP_INVALID_CONFIG"),
        "legacy serialized initialization failure",
        Remediation::recoverable("review the telemetry configuration", ["use a valid endpoint"]),
    )));
    let encoded = serde_json::to_string(&legacy).expect("legacy wrapper serializes");
    let golden = include_str!("../legacy-init-error.json").trim();
    let mut actual: serde_json::Value = serde_json::from_str(&encoded).expect("legacy JSON");
    actual
        .get_mut("diagnostic")
        .and_then(serde_json::Value::as_object_mut)
        .expect("diagnostic object")
        .remove("timestamp");
    let expected: serde_json::Value = serde_json::from_str(golden).expect("legacy golden JSON");
    assert_eq!(actual, expected);
}
