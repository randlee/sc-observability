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
    AuthHeader, ExporterBackend, LegacyRetryPolicy, LogsConfig, MetricsConfig, OtelConfig,
    OtlpEndpoint, OtlpProtocol, ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder,
    TracesConfig,
};
pub(crate) use validation::{
    BackendTransportBounds, RetryPolicy, ValidatedBackendConnection, ValidatedTransportBounds,
    validated_backend_connection, validated_telemetry_bounds, validated_transport_bounds,
};
#[cfg(test)]
pub(crate) use validation::{validate_config, validate_config_typed};

#[cfg(test)]
use crate::error_codes;
#[cfg(test)]
use sc_observability_types::{typed::InitFailure, v2::InitError};
#[cfg(test)]
use serde_json::{Map, Value};

#[cfg(test)]
#[allow(
    deprecated,
    reason = "OTLP config compatibility tests exercise retained constructors and builder"
)]
mod tests {
    use super::*;
    use sc_observability_types::v2::ConfigFailure;
    use sc_observability_types::{DiagnosticInfo, ServiceName};

    #[test]
    fn typed_config_entry_points_preserve_legacy_diagnostics() {
        fn assert_stable_diagnostic_parity(
            legacy: &sc_observability_types::Diagnostic,
            typed: &sc_observability_types::Diagnostic,
        ) {
            assert_eq!(legacy.code, typed.code);
            assert_eq!(legacy.message, typed.message);
            assert_eq!(legacy.cause, typed.cause);
            assert_eq!(legacy.remediation, typed.remediation);
            assert_eq!(legacy.docs, typed.docs);
            assert_eq!(legacy.details, typed.details);
        }

        let legacy_endpoint = OtlpEndpoint::new("not-a-url").expect_err("legacy endpoint");
        let typed_endpoint = OtlpEndpoint::new_typed("not-a-url").expect_err("typed endpoint");
        assert_stable_diagnostic_parity(legacy_endpoint.diagnostic(), typed_endpoint.diagnostic());

        for value in ["", "   "] {
            let legacy_endpoint = OtlpEndpoint::new(value).expect_err("legacy empty endpoint");
            let typed_endpoint = OtlpEndpoint::new_typed(value).expect_err("typed empty endpoint");
            assert_stable_diagnostic_parity(
                legacy_endpoint.diagnostic(),
                typed_endpoint.diagnostic(),
            );
        }

        let legacy_header = AuthHeader::new(" ").expect_err("legacy header");
        let typed_header = AuthHeader::new_typed(" ").expect_err("typed header");
        assert_stable_diagnostic_parity(legacy_header.diagnostic(), typed_header.diagnostic());

        let transport = OtelConfig {
            enabled: true,
            endpoint: None,
            ..OtelConfig::default()
        };
        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport.clone())
            .build()
            .expect_err("legacy configuration");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport)
            .build_typed()
            .expect_err("typed configuration");
        assert_stable_diagnostic_parity(legacy.diagnostic(), typed.diagnostic());
    }

    #[test]
    fn otlp_endpoint_accepts_valid_http_and_https_values() {
        let https = OtlpEndpoint::new("https://otel.example.internal").expect("valid https");
        let http = OtlpEndpoint::try_from("http://localhost:4318".to_string()).expect("valid http");
        let ipv6 = OtlpEndpoint::new_typed("https://[::1]:4318/v1/logs?signal=logs")
            .expect("valid bracketed IPv6 endpoint");

        assert_eq!(https.as_ref(), "https://otel.example.internal");
        assert_eq!(https.to_string(), "https://otel.example.internal");
        assert_eq!(http.as_str(), "http://localhost:4318");
        assert_eq!(ipv6.as_str(), "https://[::1]:4318/v1/logs?signal=logs");
    }

    #[test]
    fn otlp_endpoint_rejects_empty_or_scheme_less_values() {
        assert!(OtlpEndpoint::new("").is_err());
        assert!(OtlpEndpoint::new("otel.example.internal").is_err());
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
        assert!(AuthHeader::new("").is_err());
        assert!(AuthHeader::new("   ").is_err());
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
        let header = AuthHeader::try_from("Bearer abc123".to_string()).expect("valid header");
        assert_eq!(header.as_ref(), "Bearer abc123");
        assert_eq!(header.to_string(), "Bearer abc123");
    }

    #[test]
    fn endpoint_and_auth_header_legacy_and_typed_constructors_preserve_surrounding_whitespace() {
        // Leading whitespace before the endpoint's required http(s):// scheme is
        // rejected by the scheme check itself (unrelated to this finding); this
        // covers the actually-reachable retained-whitespace case, trailing space.
        let padded_endpoint = "https://otel.example.internal  ";
        let legacy = OtlpEndpoint::new(padded_endpoint).expect("legacy endpoint");
        let typed = OtlpEndpoint::new_typed(padded_endpoint).expect("typed endpoint");
        assert_eq!(legacy.as_str(), padded_endpoint);
        assert_eq!(typed.as_str(), padded_endpoint);

        let padded_header = "  Bearer abc123  ";
        let legacy_header = AuthHeader::new(padded_header).expect("legacy header");
        let typed_header = AuthHeader::new_typed(padded_header).expect("typed header");
        assert_eq!(legacy_header.as_str(), padded_header);
        assert_eq!(typed_header.as_str(), padded_header);
    }

    #[test]
    fn auth_header_debug_redacts_but_display_and_as_str_retain_the_raw_credential() {
        let secret = "Bearer super-secret-token";
        let header = AuthHeader::try_from(secret.to_string()).expect("valid header");

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
        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build()
            .expect_err("legacy missing endpoint");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build_typed()
            .expect_err("typed missing endpoint");

        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
    }

    #[test]
    fn public_builder_preserves_typed_validation_for_all_config_rejections() {
        fn transport() -> OtelConfig {
            OtelConfig {
                enabled: true,
                endpoint: Some(
                    OtlpEndpoint::new("https://otel.example.internal").expect("endpoint"),
                ),
                ..OtelConfig::default()
            }
        }

        fn assert_parity(legacy: &InitError, typed: &InitFailure) {
            assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
            assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
        }

        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_logs(LogsConfig { batch_size: 0 })
            .build()
            .expect_err("legacy zero batch");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_logs(LogsConfig { batch_size: 0 })
            .build_typed()
            .expect_err("typed zero batch");
        assert_parity(&legacy, &typed);

        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_metrics(MetricsConfig {
                batch_size: 1,
                export_interval_ms: 0_u64.into(),
            })
            .build()
            .expect_err("legacy zero interval");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .enable_metrics(MetricsConfig {
                batch_size: 1,
                export_interval_ms: 0_u64.into(),
            })
            .build_typed()
            .expect_err("typed zero interval");
        assert_parity(&legacy, &typed);

        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                timeout_ms: Some(0_u64.into()),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build()
            .expect_err("legacy zero timeout");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                timeout_ms: Some(0_u64.into()),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect_err("typed zero timeout");
        assert_parity(&legacy, &typed);

        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                backend: ExporterBackend::LegacyHttpJson,
                legacy_retry: Some(LegacyRetryPolicy {
                    initial_backoff_ms: Some(2_000_u64.into()),
                    max_backoff_ms: Some(1_000_u64.into()),
                    ..LegacyRetryPolicy::default()
                }),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build()
            .expect_err("legacy inverted backoff");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                backend: ExporterBackend::LegacyHttpJson,
                legacy_retry: Some(LegacyRetryPolicy {
                    initial_backoff_ms: Some(2_000_u64.into()),
                    max_backoff_ms: Some(1_000_u64.into()),
                    ..LegacyRetryPolicy::default()
                }),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect_err("typed inverted backoff");
        assert_parity(&legacy, &typed);

        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .build()
            .expect_err("legacy missing signal");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport())
            .build_typed()
            .expect_err("typed missing signal");
        assert_parity(&legacy, &typed);
    }

    #[test]
    fn public_builder_preserves_existing_protocol_endpoint_acceptance() {
        // Endpoint validation intentionally admits documented HTTP(S) endpoints for
        // every protocol. Protocol-specific transport handling is deferred to the
        // exporter layer, so neither API invents a protocol/endpoint rejection.
        let transport = OtelConfig {
            enabled: true,
            endpoint: Some(OtlpEndpoint::new("https://otel.example.internal").expect("endpoint")),
            protocol: OtlpProtocol::Grpc,
            ..OtelConfig::default()
        };
        let legacy = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport.clone())
            .enable_logs(LogsConfig::default())
            .build()
            .expect("legacy accepts the transport combination");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(transport)
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect("typed accepts the transport combination");

        assert_eq!(legacy.transport.protocol, typed.transport.protocol);
        assert_eq!(legacy.transport.endpoint, typed.transport.endpoint);
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

        let legacy = validate_config(&config).expect_err("legacy zero timeout");
        let typed = validate_config_typed(&config).expect_err("typed zero timeout");
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
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
                backend: ExporterBackend::LegacyHttpJson,
                legacy_retry: Some(LegacyRetryPolicy {
                    initial_backoff_ms: Some(2000_u64.into()),
                    max_backoff_ms: Some(1000_u64.into()),
                    ..LegacyRetryPolicy::default()
                }),
                ..OtelConfig::default()
            },
            logs: None,
            traces: None,
            metrics: None,
        };

        let legacy = validate_config(&config).expect_err("legacy backoff inversion");
        let typed = validate_config_typed(&config).expect_err("typed backoff inversion");
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
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
                    OtlpEndpoint::new("https://otel.example.internal").expect("valid endpoint"),
                ),
                ..OtelConfig::default()
            },
            logs: None,
            traces: None,
            metrics: None,
        };

        let legacy = validate_config(&config).expect_err("legacy no signals");
        let typed = validate_config_typed(&config).expect_err("typed no signals");
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
    }

    #[test]
    fn validate_config_rejects_zero_batch_or_interval() {
        let service_name = ServiceName::new("demo").expect("service");
        let base_transport = OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new("https://otel.example.internal").expect("valid endpoint"),
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
        let legacy = validate_config(&zero_logs).expect_err("legacy zero logs batch");
        let typed = validate_config_typed(&zero_logs).expect_err("typed zero logs batch");
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);

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
        let legacy = validate_config(&zero_metrics).expect_err("legacy zero metric interval");
        let typed = validate_config_typed(&zero_metrics).expect_err("typed zero metric interval");
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
    }

    #[test]
    fn telemetry_config_builder_with_resource_preserves_attributes() {
        let service_name = ServiceName::new("demo").expect("service");
        let resource = ResourceAttributes {
            attributes: Map::from_iter([("service.version".to_string(), Value::from("1.0.0"))]),
        };

        let config = TelemetryConfigBuilder::new(service_name)
            .with_resource(resource.clone())
            .build()
            .expect("valid telemetry config");

        assert_eq!(config.resource, resource);
    }
}
