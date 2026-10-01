//! OTLP configuration types, builder defaults, and construction-time validation.
//!
//! This module defines the caller-facing telemetry config surface used to build
//! a `Telemetry` runtime, including transport options, per-signal batch
//! settings, and the eager validation rules enforced at initialization time.
#![expect(
    clippy::missing_errors_doc,
    reason = "configuration-builder error behavior is documented at the telemetry facade level, and repeating it here would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "builder and accessor methods intentionally avoid repetitive must_use decoration across the config surface"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "builder-style chaining is explicit from the signatures and intentionally lightweight"
)]

mod types;
mod validation;

pub use types::{
    AuthHeader, ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
    ResourceAttributes, SyncHttpRetryPolicy, TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
};
pub(crate) use validation::{
    BackendTransportBounds, ValidatedBackendConnection, ValidatedTransportBounds,
    prepared_backend_connection, validated_released_telemetry_bounds, validated_telemetry_bounds,
};
#[cfg(test)]
pub(crate) use validation::{validate_config_typed, validated_transport_bounds};

#[cfg(feature = "sync-http")]
pub(crate) use validation::RetryPolicy;
#[cfg(any(feature = "sdk-test-support", all(test, feature = "otlp-sdk")))]
pub(crate) use validation::validated_backend_connection;

#[cfg(test)]
use crate::error_codes;
#[cfg(test)]
use serde_json::{Map, Value};

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::v2::ConfigFailure;
    use sc_observability_types::{DiagnosticInfo, ServiceName};

    #[test]
    fn otlp_endpoint_accepts_valid_http_and_https_values() {
        let https = OtlpEndpoint::new_typed("https://otel.example.internal").expect("valid https");
        let http = OtlpEndpoint::new_typed("http://localhost:4318").expect("valid http");
        let ipv6 = OtlpEndpoint::new_typed("https://[::1]:4318/v1/logs?signal=logs")
            .expect("valid bracketed IPv6 endpoint");

        assert_eq!(https.as_ref(), "https://otel.example.internal");
        assert_eq!(https.to_string(), "https://otel.example.internal");
        assert_eq!(http.as_str(), "http://localhost:4318");
        assert_eq!(ipv6.as_str(), "https://[::1]:4318/v1/logs?signal=logs");
    }

    #[test]
    fn otlp_endpoint_rejects_empty_or_scheme_less_values() {
        assert!(OtlpEndpoint::new_typed("").is_err());
        assert!(OtlpEndpoint::new_typed("otel.example.internal").is_err());
    }

    #[test]
    fn otlp_endpoint_enforces_the_documented_http_subset() {
        for endpoint in [
            "https://user:password@otel.example.internal",
            "https://otel_collector.example.internal",
            "https://münich.example.internal",
            "https://::1:4318",
        ] {
            assert!(
                OtlpEndpoint::new_typed(endpoint).is_err(),
                "the documented subset rejects {endpoint:?}"
            );
        }
    }

    #[test]
    fn typed_endpoint_rejects_missing_hosts_with_the_canonical_failure() {
        for value in ["https://", "http://", "https://?signal=logs", "not-a-url"] {
            let error = OtlpEndpoint::new_typed(value).expect_err("invalid endpoint");
            assert!(matches!(error, ConfigFailure::InvalidEndpoint { .. }));
            assert_eq!(
                error.diagnostic().code,
                error_codes::OTLP_CONFIG_INVALID_ENDPOINT
            );
        }
    }

    #[test]
    fn endpoint_and_header_diagnostics_name_explicit_stable_fields() {
        let endpoint = OtlpEndpoint::new_typed("not-a-url").expect_err("invalid endpoint");
        assert_eq!(
            endpoint.diagnostic().details["field"].as_str(),
            Some("endpoint")
        );
        assert_eq!(
            endpoint.diagnostic().details["origin"].as_str(),
            Some("explicit")
        );

        let header = AuthHeader::new_typed(" ").expect_err("invalid header");
        assert_eq!(
            header.diagnostic().details["field"].as_str(),
            Some("auth_header")
        );
        assert_eq!(
            header.diagnostic().details["origin"].as_str(),
            Some("explicit")
        );
    }

    #[test]
    fn auth_header_rejects_empty_values() {
        assert!(AuthHeader::new_typed("").is_err());
        assert!(AuthHeader::new_typed("   ").is_err());
    }

    #[test]
    fn typed_auth_header_rejects_control_characters_with_the_canonical_failure() {
        for value in [
            "Bearer token\r\nInjected: true",
            "Bearer\u{0000}token",
            "Bearer\t token",
        ] {
            let error = AuthHeader::new_typed(value).expect_err("invalid header");
            assert!(matches!(error, ConfigFailure::InvalidHeader { .. }));
            assert_eq!(
                error.diagnostic().code,
                error_codes::OTLP_CONFIG_INVALID_HEADER
            );
        }
    }

    #[test]
    fn auth_header_accepts_non_empty_values() {
        let header = AuthHeader::new_typed("Bearer abc123").expect("valid header");
        assert_eq!(header.as_ref(), "Bearer abc123");
        assert_eq!(header.to_string(), "Bearer abc123");
    }

    #[test]
    fn endpoint_and_auth_header_constructors_preserve_surrounding_whitespace() {
        // Leading whitespace before the endpoint's required http(s):// scheme is
        // rejected by the scheme check itself (unrelated to this finding); this
        // covers the actually-reachable retained-whitespace case, trailing space.
        let padded_endpoint = "https://otel.example.internal  ";
        let typed = OtlpEndpoint::new_typed(padded_endpoint).expect("typed endpoint");
        assert_eq!(typed.as_str(), padded_endpoint);

        let padded_header = "  Bearer abc123  ";
        let typed_header = AuthHeader::new_typed(padded_header).expect("typed header");
        assert_eq!(typed_header.as_str(), padded_header);
    }

    #[test]
    fn auth_header_debug_redacts_but_display_and_as_str_retain_the_raw_credential() {
        let secret = "Bearer super-secret-token";
        let header = AuthHeader::new_typed(secret).expect("valid header");

        let debug_output = format!("{header:?}");
        assert!(
            !debug_output.contains(secret),
            "Debug output must never contain the raw credential: {debug_output}"
        );
        assert_eq!(debug_output, "AuthHeader(\"<redacted>\")");

        // Display and as_str remain the documented explicit raw-value accessors.
        assert_eq!(header.to_string(), secret);
        assert_eq!(header.as_str(), secret);

        let config = OtelConfig {
            enabled: true,
            auth_header: Some(header),
            ..OtelConfig::default()
        };
        let config_debug = format!("{config:?}");
        assert!(
            !config_debug.contains(secret),
            "OtelConfig Debug must not leak the auth header credential: {config_debug}"
        );
    }

    #[test]
    fn telemetry_config_builder_build_validates_transport() {
        let failure = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build_typed()
            .expect_err("typed missing endpoint");
        assert_eq!(
            failure.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );
    }

    #[test]
    fn public_builder_preserves_typed_validation_for_all_config_rejections() {
        fn transport() -> OtelConfig {
            OtelConfig {
                enabled: true,
                endpoint: Some(
                    OtlpEndpoint::new_typed("https://otel.example.internal").expect("endpoint"),
                ),
                ..OtelConfig::default()
            }
        }
        let zero_batch = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_logs(LogsConfig { batch_size: 0 })
            .build_typed()
            .expect_err("zero batch");
        assert_eq!(
            zero_batch.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );

        let zero_interval = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_metrics(MetricsConfig {
                batch_size: 1,
                export_interval_ms: 0_u64.into(),
            })
            .build_typed()
            .expect_err("zero interval");
        assert_eq!(
            zero_interval.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );

        let zero_timeout = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                timeout_ms: Some(0_u64.into()),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect_err("zero timeout");
        assert_eq!(
            zero_timeout.diagnostic().code,
            error_codes::OTLP_CONFIG_ZERO_DURATION
        );

        let inverted_backoff =
            TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
                .with_transport(OtelConfig {
                    backend: ExporterBackend::SyncHttp,
                    sync_http_retry: Some(SyncHttpRetryPolicy {
                        initial_backoff_ms: Some(2_000_u64.into()),
                        max_backoff_ms: Some(1_000_u64.into()),
                        ..SyncHttpRetryPolicy::default()
                    }),
                    ..transport()
                })
                .enable_logs(LogsConfig::default())
                .build_typed()
                .expect_err("inverted backoff");
        assert_eq!(
            inverted_backoff.diagnostic().code,
            error_codes::OTLP_CONFIG_BOUND_ORDER
        );

        let missing_signal =
            TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
                .with_transport(transport())
                .build_typed()
                .expect_err("missing signal");
        assert_eq!(
            missing_signal.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );
    }

    #[test]
    fn public_builder_preserves_existing_protocol_endpoint_acceptance() {
        // Endpoint validation intentionally admits documented HTTP(S) endpoints for
        // every protocol. Protocol-specific transport handling is deferred to the
        // exporter layer, so the canonical API does not invent a rejection.
        let transport = OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new_typed("https://otel.example.internal").expect("endpoint"),
            ),
            protocol: OtlpProtocol::Grpc,
            ..OtelConfig::default()
        };
        let config = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport)
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect("valid transport combination");

        assert_eq!(config.transport.protocol, OtlpProtocol::Grpc);
        assert_eq!(
            config.transport.endpoint.as_ref().map(OtlpEndpoint::as_str),
            Some("https://otel.example.internal")
        );
    }

    #[test]
    fn validate_config_rejects_zero_timeout() {
        let service_name = ServiceName::new("demo").expect("service");
        let config = TelemetryConfig {
            service_name,
            resource: ResourceAttributes::default(),
            transport: OtelConfig {
                timeout_ms: Some(0_u64.into()),
                ..OtelConfig::default()
            },
            logs: None,
            traces: None,
            metrics: None,
        };

        let failure = validate_config_typed(&config).expect_err("zero timeout");
        assert_eq!(
            failure.diagnostic().code,
            error_codes::OTLP_CONFIG_ZERO_DURATION
        );
    }

    #[test]
    fn validate_config_checks_transport_before_missing_enabled_endpoint() {
        let config = TelemetryConfig {
            service_name: ServiceName::new("demo").expect("service"),
            resource: ResourceAttributes::default(),
            transport: OtelConfig {
                enabled: true,
                timeout_ms: Some(0_u64.into()),
                ..OtelConfig::default()
            },
            logs: Some(LogsConfig::default()),
            traces: None,
            metrics: None,
        };

        let typed = validate_config_typed(&config)
            .expect_err("transport validation precedes the missing endpoint check");
        assert_eq!(
            typed.diagnostic().code,
            error_codes::OTLP_CONFIG_ZERO_DURATION
        );
    }

    #[test]
    fn validate_config_rejects_backoff_inversion() {
        let service_name = ServiceName::new("demo").expect("service");
        let config = TelemetryConfig {
            service_name,
            resource: ResourceAttributes::default(),
            transport: OtelConfig {
                backend: ExporterBackend::SyncHttp,
                sync_http_retry: Some(SyncHttpRetryPolicy {
                    initial_backoff_ms: Some(2000_u64.into()),
                    max_backoff_ms: Some(1000_u64.into()),
                    ..SyncHttpRetryPolicy::default()
                }),
                ..OtelConfig::default()
            },
            logs: None,
            traces: None,
            metrics: None,
        };

        let failure = validate_config_typed(&config).expect_err("backoff inversion");
        assert_eq!(
            failure.diagnostic().details["field"].as_str(),
            Some("sync_http_retry.initial_backoff_ms")
        );
    }

    #[test]
    fn validate_config_rejects_enabled_transport_without_signals() {
        let service_name = ServiceName::new("demo").expect("service");
        let config = TelemetryConfig {
            service_name,
            resource: ResourceAttributes::default(),
            transport: OtelConfig {
                enabled: true,
                endpoint: Some(
                    OtlpEndpoint::new_typed("https://otel.example.internal")
                        .expect("valid endpoint"),
                ),
                ..OtelConfig::default()
            },
            logs: None,
            traces: None,
            metrics: None,
        };

        let failure = validate_config_typed(&config).expect_err("no signals");
        assert_eq!(
            failure.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );
    }

    #[test]
    fn validate_config_rejects_zero_batch_or_interval() {
        let service_name = ServiceName::new("demo").expect("service");
        let base_transport = OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new_typed("https://otel.example.internal").expect("valid endpoint"),
            ),
            ..OtelConfig::default()
        };

        let zero_logs = TelemetryConfig {
            service_name: service_name.clone(),
            resource: ResourceAttributes::default(),
            transport: base_transport.clone(),
            logs: Some(LogsConfig { batch_size: 0 }),
            traces: None,
            metrics: None,
        };
        let failure = validate_config_typed(&zero_logs).expect_err("zero logs batch");
        assert_eq!(
            failure.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );

        let zero_metrics = TelemetryConfig {
            service_name,
            resource: ResourceAttributes::default(),
            transport: base_transport,
            logs: None,
            traces: None,
            metrics: Some(MetricsConfig {
                batch_size: 1,
                export_interval_ms: 0_u64.into(),
            }),
        };
        let failure = validate_config_typed(&zero_metrics).expect_err("zero metric interval");
        assert_eq!(
            failure.diagnostic().code,
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED
        );
    }

    #[test]
    fn telemetry_config_builder_with_resource_preserves_attributes() {
        let service_name = ServiceName::new("demo").expect("service");
        let resource = ResourceAttributes {
            attributes: Map::from_iter([("service.version".to_string(), Value::from("1.0.0"))]),
        };

        let config = TelemetryConfigBuilder::new(service_name)
            .with_resource(resource.clone())
            .build_typed()
            .expect("valid telemetry config");

        assert_eq!(config.resource, resource);
    }
}
