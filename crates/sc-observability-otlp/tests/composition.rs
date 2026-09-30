//! Public composition checks for enabled and disabled transport selection.

use sc_observability_otlp::v2::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry,
    TelemetryConfigBuilder, TracesConfig,
};
use sc_observability_types::{
    ActionName, DiagnosticInfo, ErrorCode, Level, LogEvent, ProcessIdentity, SchemaVersion,
    ServiceName, TargetCategory, TelemetryHealthState, Timestamp,
};
use serde_json::Map;

fn service() -> ServiceName {
    ServiceName::new("composition-test").expect("valid service")
}

fn event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service(),
        target: TargetCategory::new("composition").expect("valid target"),
        action: ActionName::new("composition.test").expect("valid action"),
        message: Some("composition".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Map::new(),
    }
}

fn config(transport: OtelConfig) -> sc_observability_otlp::v2::TelemetryConfig {
    TelemetryConfigBuilder::new(service())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(transport)
        .build_typed()
        .expect("valid telemetry config")
}

#[test]
fn disabled_composition_is_real_and_has_no_network_exporter() {
    let telemetry = Telemetry::new(config(OtelConfig::default())).expect("disabled runtime");
    assert_eq!(telemetry.health().state, TelemetryHealthState::Disabled);
    telemetry
        .emit_log(&event())
        .expect("disabled emit is accepted");
    telemetry.flush().expect("disabled flush is a no-op");
}

#[test]
fn enabled_sdk_selection_never_falls_back_to_disabled_exporters() {
    let mut transport = OtelConfig::default();
    transport.enabled = true;
    transport.endpoint =
        Some(OtlpEndpoint::new_typed("https://otel.example.internal").expect("endpoint"));
    let Err(error) = Telemetry::new(config(transport)) else {
        panic!("enabled SDK must compose or fail");
    };
    assert_ne!(error.diagnostic().code, ErrorCode::new_static("SC_OK"));
}

#[test]
fn sdk_backend_rejects_http_json_protocol_instead_of_switching_backend() {
    let mut transport = OtelConfig::default();
    transport.enabled = true;
    transport.backend = ExporterBackend::OpenTelemetrySdk;
    transport.protocol = OtlpProtocol::HttpJson;
    transport.endpoint =
        Some(OtlpEndpoint::new_typed("https://otel.example.internal").expect("endpoint"));
    let Err(error) = Telemetry::new(config(transport)) else {
        panic!("protocol mismatch must fail");
    };
    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL
    );
}

#[test]
fn legacy_backend_rejects_grpc_protocol_instead_of_switching_backend() {
    let mut transport = OtelConfig::default();
    transport.enabled = true;
    transport.backend = ExporterBackend::LegacyHttpJson;
    transport.protocol = OtlpProtocol::Grpc;
    transport.endpoint =
        Some(OtlpEndpoint::new_typed("https://otel.example.internal").expect("endpoint"));
    let Err(error) = Telemetry::new(config(transport)) else {
        panic!("protocol mismatch must fail");
    };
    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL
    );
}
