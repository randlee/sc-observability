//! External consumer proof for the released 1.4.1 root OTLP API.

#![allow(deprecated)]

use sc_observability_otlp::{
    AuthHeader, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
    ResourceAttributes, SpanAssembler, Telemetry, TelemetryConfig, TelemetryConfigBuilder,
    TracesConfig,
};
use sc_observability_types::{
    ActionName, Diagnostic, DiagnosticInfo, DurationMs, ErrorCode, EventError, FlushError,
    InitError, Level, LogEvent, Observable, ProcessIdentity, Remediation, SchemaVersion,
    ServiceName, ShutdownError, SpanSignal, TargetCategory, TelemetryError, Timestamp,
};
use serde_json::Map;
use std::sync::Arc;

fn service() -> ServiceName {
    ServiceName::new("released-client").expect("valid service")
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
        target: TargetCategory::new("compat.test").expect("valid target"),
        action: ActionName::new("compat.emit").expect("valid action"),
        message: Some("legacy consumer".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("OTLP_COMPAT_TEST"),
            message: "test diagnostic".to_owned(),
            cause: None,
            remediation: Remediation::not_recoverable("no action required"),
            docs: None,
            details: Map::new(),
        }),
        state_transition: None,
        fields: Map::new(),
    }
}

#[allow(dead_code)]
fn released_projector_signature<T: Observable>(
    telemetry: Arc<Telemetry>,
) -> sc_observability_otlp::TelemetryProjectors<T> {
    sc_observability_otlp::TelemetryProjectors::new(telemetry)
}

#[test]
fn released_struct_literals_and_result_signatures_remain_source_compatible() {
    let _: fn(
        &mut SpanAssembler,
        SpanSignal,
    ) -> Result<Option<sc_observability_otlp::CompleteSpan>, EventError> = SpanAssembler::push;
    let _: fn(String) -> Result<OtlpEndpoint, InitError> = OtlpEndpoint::new;
    let _: fn(String) -> Result<OtlpEndpoint, sc_observability_types::typed::InitFailure> =
        OtlpEndpoint::new_typed;
    let _: fn(String) -> Result<AuthHeader, InitError> = AuthHeader::new;
    let _: fn(String) -> Result<AuthHeader, sc_observability_types::typed::InitFailure> =
        AuthHeader::new_typed;
    let _: fn(TelemetryConfigBuilder) -> Result<TelemetryConfig, InitError> =
        TelemetryConfigBuilder::build;
    let _: fn(TelemetryConfig) -> Result<Telemetry, InitError> = Telemetry::new;
    let _: fn(TelemetryConfig) -> Result<Telemetry, sc_observability_types::typed::InitFailure> =
        Telemetry::new_typed;
    let _: fn(&Telemetry) -> Result<(), FlushError> = Telemetry::flush;
    let _: fn(&Telemetry) -> Result<(), sc_observability_types::typed::FlushFailure> =
        Telemetry::flush_typed;
    let _: fn(&Telemetry) -> Result<(), ShutdownError> = Telemetry::shutdown;
    let _: fn(&Telemetry) -> Result<(), sc_observability_types::typed::ShutdownFailure> =
        Telemetry::shutdown_typed;
    let _: fn(&Telemetry, &LogEvent) -> Result<(), TelemetryError> = Telemetry::emit_log;
    let _: fn(&Telemetry, &sc_observability_types::SpanSignal) -> Result<(), TelemetryError> =
        Telemetry::emit_span;
    let _: fn(&Telemetry, &sc_observability_types::MetricRecord) -> Result<(), TelemetryError> =
        Telemetry::emit_metric;

    let defaults = OtelConfig::default();
    assert_eq!(defaults.protocol, OtlpProtocol::HttpBinary);
    assert_eq!(u64::from(defaults.timeout_ms), 3_000);
    assert_eq!(defaults.max_retries, 3);
    assert_eq!(u64::from(defaults.initial_backoff_ms), 250);
    assert_eq!(u64::from(defaults.max_backoff_ms), 5_000);

    let endpoint = OtlpEndpoint::new("http://localhost:4318").expect("endpoint");
    let header = AuthHeader::new("Bearer legacy-token").expect("header");
    let transport = OtelConfig {
        enabled: false,
        endpoint: Some(endpoint),
        protocol: OtlpProtocol::HttpBinary,
        auth_header: Some(header),
        ca_file: None,
        insecure_skip_verify: false,
        timeout_ms: DurationMs::from(1_000),
        debug_local_export: false,
        max_retries: 3,
        initial_backoff_ms: DurationMs::from(100),
        max_backoff_ms: DurationMs::from(1_000),
    };
    let config = TelemetryConfig {
        service_name: service(),
        resource: ResourceAttributes::default(),
        transport,
        logs: Some(LogsConfig::default()),
        traces: Some(TracesConfig::default()),
        metrics: Some(MetricsConfig::default()),
    };

    let telemetry = Telemetry::new(config).expect("released configuration builds");
    telemetry.flush().expect("disabled transport flushes");
    telemetry.shutdown().expect("disabled transport shuts down");
    assert!(matches!(
        telemetry.emit_log(&event()),
        Err(TelemetryError::Shutdown)
    ));
}

#[test]
fn released_builder_keeps_both_error_contracts() {
    let _: fn(
        TelemetryConfigBuilder,
    ) -> Result<TelemetryConfig, sc_observability_types::typed::InitFailure> =
        TelemetryConfigBuilder::build_typed;
    let config = TelemetryConfigBuilder::new(service())
        .enable_logs(LogsConfig::default())
        .build_typed()
        .expect("valid config");
    assert_eq!(
        config.logs.expect("configured logs").batch_size,
        LogsConfig::default().batch_size
    );
}

#[test]
fn released_facade_maps_invalid_configuration_to_the_legacy_error_owner() {
    let transport = OtelConfig {
        enabled: true,
        ..OtelConfig::default()
    };
    let config = TelemetryConfig {
        service_name: service(),
        resource: ResourceAttributes::default(),
        transport,
        logs: Some(LogsConfig::default()),
        traces: None,
        metrics: None,
    };

    let Err(legacy) = Telemetry::new(config.clone()) else {
        panic!("missing endpoint must fail");
    };
    let Err(typed) = Telemetry::new_typed(config) else {
        panic!("missing endpoint must fail");
    };
    assert_eq!(
        legacy.diagnostic().code,
        typed.diagnostic().code,
        "compatibility conversion must preserve the canonical diagnostic"
    );
}
