use sc_observability::{Logger, LoggerConfig};
use sc_observability_otlp::{
    AuthHeader, OtlpEndpoint, SpanAssembler, Telemetry, TelemetryConfigBuilder,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, ErrorContext, InitError, LogEvent, Level, ProcessIdentity,
    ProjectionRegistration, Remediation, SchemaVersion,
    ServiceName, SpanId, SpanRecord, SpanSignal, SpanStarted, TargetCategory, Timestamp, ToolName,
    TraceContext, TraceId,
};
use sc_observe::{Observability, ObservabilityConfig};

fn main() {
    exercise_legacy_wrapper_inventory();
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
    let builder = sc_observability::LoggerBuilder::new(builder_config)
        .expect("legacy builder remains usable with warnings");
    let _ = builder.build().shutdown();

    let mut resolver_config = LoggerConfig::default_for(
        ServiceName::new("b1e-legacy-resolver").expect("valid service"),
        std::env::temp_dir().join("sc-observability-b1e-legacy-resolver"),
    );
    resolver_config.enable_file_sink = false;
    resolver_config.enable_console_sink = false;
    let resolver_builder = Logger::builder(resolver_config).expect("legacy logger builder");
    let _ = resolver_builder.build().shutdown();

    let logger = Logger::new(logger_config).expect("legacy logger remains usable with warnings");
    logger.log(event(ServiceName::new("b1e-legacy").expect("service"))).expect("legacy log");
    logger
        .try_log(event(ServiceName::new("b1e-legacy").expect("service")))
        .expect("legacy try log");
    logger
        .try_log_with_outcome(event(ServiceName::new("b1e-legacy").expect("service")))
        .expect("legacy outcome log");
    logger.flush().expect("legacy flush remains usable");
    let _ = logger.shutdown();

    let config = ObservabilityConfig::default_for(
        ToolName::new("b1e-legacy").expect("valid tool"),
        root,
    )
    .expect("legacy observation config");
    let _ = config.service_name().expect("legacy service accessor");
    let _ = Observability::new(config);

    let empty_builder_config = ObservabilityConfig::default_for(
        ToolName::new("b1e-legacy-empty").expect("valid tool"),
        std::env::temp_dir().join("sc-observability-b1e-legacy-empty"),
    )
    .expect("legacy observation config");
    assert!(Observability::builder(empty_builder_config).build().is_err());

    let route_config = ObservabilityConfig::default_for(
        ToolName::new("b1e-legacy-route").expect("valid tool"),
        std::env::temp_dir().join("sc-observability-b1e-legacy-route"),
    )
    .expect("legacy route config");
    let routed = Observability::builder(route_config)
        .register_projection(ProjectionRegistration::<String>::new())
        .build()
        .expect("legacy observation builder");
    routed.flush().expect("legacy observation flush");
    routed.shutdown().expect("legacy observation shutdown");

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
    assert_eq!(legacy.0.diagnostic().code.as_str(), "SC_OBSERVABILITY_OTLP_INVALID_CONFIG");

    let mut assembler = SpanAssembler::new();
    let signal = SpanSignal::Started(SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        ServiceName::new("b1e-legacy-span").expect("service"),
        ActionName::new("fixture.span").expect("action"),
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("trace"),
            span_id: SpanId::new("0123456789abcdef").expect("span"),
            parent_span_id: None,
        },
        serde_json::Map::new(),
    ));
    assert!(assembler.push(signal).expect("legacy span push").is_none());
}

fn exercise_legacy_wrapper_inventory() {
    let _ = std::mem::size_of::<sc_observability_types::IdentityError>();
    let _ = std::mem::size_of::<sc_observability_types::InitError>();
    let _ = std::mem::size_of::<sc_observability_types::EventError>();
    let _ = std::mem::size_of::<sc_observability_types::FlushError>();
    let _ = std::mem::size_of::<sc_observability_types::ShutdownError>();
    let _ = std::mem::size_of::<sc_observability_types::ProjectionError>();
    let _ = std::mem::size_of::<sc_observability_types::SubscriberError>();
    let _ = std::mem::size_of::<sc_observability_types::LogSinkError>();
    let _ = std::mem::size_of::<sc_observability_types::ExportError>();
}

fn event(service: ServiceName) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("schema"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service,
        target: TargetCategory::new("fixture").expect("target"),
        action: ActionName::new("fixture.event").expect("action"),
        message: Some("legacy fixture".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_FIXTURE_EVENT"),
            message: "fixture".to_owned(),
            cause: None,
            remediation: Remediation::not_recoverable("fixture"),
            docs: None,
            details: serde_json::Map::new(),
        }),
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}
