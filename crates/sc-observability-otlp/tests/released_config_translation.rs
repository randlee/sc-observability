//! External regression coverage for released OTLP transport translation.

#![expect(
    deprecated,
    reason = "the integration test exercises the published 1.4.1 compatibility facade"
)]

use sc_observability_otlp::{
    LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, TelemetryConfigBuilder,
};
use sc_observability_types::{DiagnosticInfo, DurationMs, ServiceName};

fn service() -> ServiceName {
    ServiceName::new("released-config-translation").expect("valid service")
}

fn released_transport(protocol: OtlpProtocol, enabled: bool) -> OtelConfig {
    OtelConfig {
        enabled,
        endpoint: Some(OtlpEndpoint::new("http://localhost:4318").expect("valid endpoint")),
        protocol,
        timeout_ms: DurationMs::from(500),
        max_retries: 7,
        initial_backoff_ms: DurationMs::from(300),
        max_backoff_ms: DurationMs::from(200),
        ..OtelConfig::default()
    }
}

fn validate(transport: OtelConfig) -> Result<(), sc_observability_types::typed::InitFailure> {
    TelemetryConfigBuilder::new(service())
        .with_transport(transport)
        .enable_logs(LogsConfig::default())
        .build_typed()
        .map(|_| ())
}

#[test]
fn released_http_json_retains_retry_bounds_through_the_facade() {
    let error = validate(released_transport(OtlpProtocol::HttpJson, true))
        .expect_err("HTTP/JSON must preserve its released retry settings");

    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_CONFIG_BOUND_ORDER
    );
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("legacy_retry.initial_backoff_ms")
    );
    assert_eq!(
        error.diagnostic().details["lower_value"].as_u64(),
        Some(300)
    );
    assert_eq!(
        error.diagnostic().details["upper_value"].as_u64(),
        Some(200)
    );
}

#[test]
fn released_binary_and_disabled_transports_discard_retry_bounds() {
    validate(released_transport(OtlpProtocol::HttpBinary, true))
        .expect("HTTP/binary selects the SDK, which has no released retry policy");
    validate(released_transport(OtlpProtocol::HttpJson, false))
        .expect("disabled transport has no backend or released retry policy");
}

#[test]
fn released_facade_carries_the_transport_timeout() {
    let mut transport = released_transport(OtlpProtocol::HttpJson, true);
    transport.initial_backoff_ms = DurationMs::from(100);
    transport.max_backoff_ms = DurationMs::from(200);
    transport.timeout_ms = DurationMs::from(0);

    let error = validate(transport).expect_err("zero timeout must remain invalid through facade");

    assert_eq!(
        error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_CONFIG_ZERO_DURATION
    );
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("timeout_ms")
    );
}
