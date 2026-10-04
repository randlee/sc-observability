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

// Frozen oracle: actual unmodified 1.4.1 c578912653233c7dc678fefe5af575118dbbaaa1
// compiled by the release audit. Expected diagnostics are independent data, not
// computed with the candidate validator. Shutdown/collector behavior is separate.
#[cfg(all(feature = "otlp-sdk", feature = "sync-http"))]
mod released_oracle {
    #![expect(
        deprecated,
        reason = "four released constructor entry points are the oracle subjects"
    )]
    use sc_observability_otlp::*;
    use sc_observability_types::{DiagnosticInfo, DurationMs, ServiceName};
    use serde_json::{Value, json};
    fn oracle_config(protocol: OtlpProtocol, enabled: bool, case: &str) -> TelemetryConfig {
        let mut t = OtelConfig {
            enabled,
            protocol,
            endpoint: Some(OtlpEndpoint::new_typed("http://127.0.0.1:4318").unwrap()),
            timeout_ms: DurationMs::from(1000),
            initial_backoff_ms: DurationMs::from(100),
            max_backoff_ms: DurationMs::from(200),
            ..OtelConfig::default()
        };
        if case == "insecure_true" {
            t.insecure_skip_verify = true;
        }
        if case.contains("missing_endpoint") {
            t.endpoint = None;
        }
        if case.contains("zero_timeout") {
            t.timeout_ms = DurationMs::from(0);
        }
        if case.contains("inverted") {
            t.initial_backoff_ms = DurationMs::from(300);
        }
        match case {
            "zero_initial" => t.initial_backoff_ms = DurationMs::from(0),
            "zero_both" => {
                t.initial_backoff_ms = DurationMs::from(0);
                t.max_backoff_ms = DurationMs::from(0);
            }
            "zero_max" => t.max_backoff_ms = DurationMs::from(0),
            "large_timeout" => t.timeout_ms = DurationMs::from(60001),
            _ => (),
        }
        let mut logs = LogsConfig::default();
        if case.contains("zero_batch") {
            logs.batch_size = 0;
        }
        let traces = if case == "zero_trace_batch" {
            Some(TracesConfig { batch_size: 0 })
        } else {
            None
        };
        let metrics = if case.starts_with("zero_metric") {
            let mut x = MetricsConfig::default();
            if case == "zero_metric_batch" {
                x.batch_size = 0;
            } else {
                x.export_interval_ms = DurationMs::from(0);
            }
            Some(x)
        } else {
            None
        };
        TelemetryConfig {
            service_name: ServiceName::new("release-audit").unwrap(),
            resource: ResourceAttributes::default(),
            transport: t,
            logs: if case.contains("no_signals") {
                None
            } else {
                Some(logs)
            },
            traces,
            metrics,
        }
    }
    fn oracle_builder(c: TelemetryConfig) -> TelemetryConfigBuilder {
        let mut b = TelemetryConfigBuilder::new(c.service_name)
            .with_resource(c.resource)
            .with_transport(c.transport);
        if let Some(l) = c.logs {
            b = b.enable_logs(l);
        }
        if let Some(t) = c.traces {
            b = b.enable_traces(t);
        }
        if let Some(m) = c.metrics {
            b = b.enable_metrics(m);
        }
        b
    }

    fn construction_result<T, E: DiagnosticInfo>(value: Result<T, E>) -> Value {
        match value {
            Ok(_) => json!({"ok":true}),
            Err(error) => {
                let diagnostic = error.diagnostic();
                json!({"ok":false,"code":diagnostic.code.as_str(),"message":diagnostic.message,
                    "remediation":format!("{:?}",diagnostic.remediation),"details":diagnostic.details})
            }
        }
    }
    fn runtime_result<E: DiagnosticInfo>(value: Result<Telemetry, E>) -> Value {
        match value {
            Ok(telemetry) => {
                // No events are emitted. Real exporter cleanup is performed but
                // is not confused with the released constructor validation oracle.
                let _shutdown = telemetry.shutdown_typed();
                json!({"ok":true})
            }
            Err(error) => construction_result::<(), E>(Err(error)),
        }
    }
    fn expected_cases() -> Vec<(&'static str, usize, usize)> {
        vec![
            ("valid", 0, 0),
            ("inverted", 1, 1),
            ("zero_initial", 0, 0),
            ("zero_both", 0, 0),
            ("zero_max", 1, 1),
            ("zero_timeout", 2, 2),
            ("missing_endpoint", 0, 4),
            ("missing_endpoint_zero_timeout", 2, 4),
            ("missing_endpoint_inverted", 1, 4),
            ("zero_timeout_inverted", 2, 2),
            ("no_signals", 0, 5),
            ("no_signals_zero_timeout", 2, 2),
            ("zero_batch", 3, 3),
            ("zero_batch_inverted", 1, 1),
            ("large_timeout", 0, 0),
            ("zero_trace_batch", 3, 3),
            ("zero_metric_batch", 3, 3),
            ("zero_metric_interval", 3, 3),
            ("missing_endpoint_zero_batch", 3, 4),
            ("no_signals_inverted", 1, 1),
            ("insecure_true", 0, 0),
        ]
    }
    #[test]
    fn all_504_released_construction_rows_match_frozen_141() {
        let expected: Vec<Value> = serde_json::from_str(r#"[{"ok":true},{"code":"SC_OBSERVABILITY_OTLP_INVALID_CONFIG","details":{},"message":"initial_backoff_ms must not exceed max_backoff_ms","ok":false,"remediation":"Recoverable { steps: RecoverableSteps { steps: [\"fix the backoff configuration\", \"use documented defaults\"] } }"},{"code":"SC_OBSERVABILITY_OTLP_INVALID_CONFIG","details":{},"message":"timeout_ms must be greater than zero","ok":false,"remediation":"Recoverable { steps: RecoverableSteps { steps: [\"set timeout_ms to a positive value\", \"use documented defaults\"] } }"},{"code":"SC_OBSERVABILITY_OTLP_INVALID_CONFIG","details":{},"message":"telemetry batch sizing and export intervals must be positive","ok":false,"remediation":"Recoverable { steps: RecoverableSteps { steps: [\"set batch sizes and export intervals above zero\", \"use documented defaults\"] } }"},{"code":"SC_OBSERVABILITY_OTLP_INVALID_CONFIG","details":{},"message":"enabled telemetry requires an endpoint","ok":false,"remediation":"Recoverable { steps: RecoverableSteps { steps: [\"set OtelConfig.endpoint before constructing Telemetry\", \"disable telemetry for local-only runs if OTLP is not required\"] } }"},{"code":"SC_OBSERVABILITY_OTLP_INVALID_CONFIG","details":{},"message":"at least one telemetry signal must be enabled","ok":false,"remediation":"Recoverable { steps: RecoverableSteps { steps: [\"enable logs, traces, or metrics before constructing Telemetry\", \"disable the OTLP layer entirely if telemetry is not needed\"] } }"}]"#).unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let _entered = runtime.enter();
        let mut count = 0;
        for protocol in [
            OtlpProtocol::HttpBinary,
            OtlpProtocol::HttpJson,
            OtlpProtocol::Grpc,
        ] {
            for enabled in [false, true] {
                for (case, disabled_expected, enabled_expected) in expected_cases() {
                    let config = oracle_config(protocol, enabled, case);
                    let expected = expected
                        .get(if enabled {
                            enabled_expected
                        } else {
                            disabled_expected
                        })
                        .unwrap();
                    for (entry, actual) in [
                        (
                            "build_typed",
                            construction_result(oracle_builder(config.clone()).build_typed()),
                        ),
                        (
                            "build",
                            construction_result(oracle_builder(config.clone()).build()),
                        ),
                        (
                            "new_typed",
                            runtime_result(Telemetry::new_typed(config.clone())),
                        ),
                        ("new", runtime_result(Telemetry::new(config))),
                    ] {
                        assert_eq!(&actual, expected, "{protocol:?}/{enabled}/{case}/{entry}");
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 504);
    }
}
