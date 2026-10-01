//! External regression coverage for released OTLP transport translation.

#![expect(
    deprecated,
    reason = "the integration test exercises the published 1.4.1 compatibility facade"
)]

use sc_observability_otlp::{
    LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, TelemetryConfigBuilder,
};
use sc_observability_types::{DiagnosticInfo, DurationMs, Remediation, ServiceName};

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
        .expect_err("released retry bounds must remain valid through the facade");

    assert_eq!(
        error.diagnostic().code,
        sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG
    );
    assert_eq!(
        error.diagnostic().message,
        "initial_backoff_ms must not exceed max_backoff_ms"
    );
    assert_eq!(
        error.diagnostic().remediation,
        Remediation::recoverable("fix the backoff configuration", ["use documented defaults"])
    );
}

#[test]
fn released_binary_and_disabled_transports_retain_released_bound_validation() {
    for transport in [
        released_transport(OtlpProtocol::HttpBinary, true),
        released_transport(OtlpProtocol::HttpJson, false),
    ] {
        let error = validate(transport)
            .expect_err("released retry bounds apply before backend selection or disabling");

        assert_eq!(
            error.diagnostic().code,
            sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG
        );
        assert_eq!(
            error.diagnostic().message,
            "initial_backoff_ms must not exceed max_backoff_ms"
        );
        assert_eq!(
            error.diagnostic().remediation,
            Remediation::recoverable("fix the backoff configuration", ["use documented defaults"])
        );
    }
}

#[test]
fn released_transport_matrix_accepts_ordered_bounds() {
    for protocol in [
        OtlpProtocol::HttpBinary,
        OtlpProtocol::HttpJson,
        OtlpProtocol::Grpc,
    ] {
        for enabled in [false, true] {
            let mut transport = released_transport(protocol, enabled);
            transport.initial_backoff_ms = DurationMs::from(100);
            transport.max_backoff_ms = DurationMs::from(200);
            validate(transport).expect("ordered released bounds must remain valid");
        }
    }
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
        sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG
    );
    assert_eq!(
        error.diagnostic().message,
        "timeout_ms must be greater than zero"
    );
    assert_eq!(
        error.diagnostic().remediation,
        Remediation::recoverable(
            "set timeout_ms to a positive value",
            ["use documented defaults"],
        )
    );
}
