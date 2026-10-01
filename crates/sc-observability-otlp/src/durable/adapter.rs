//! Maps durable configuration into the existing validated transport contract.
use crate::config::{
    AuthHeader, ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, SyncHttpRetryPolicy,
};
use sc_observability_types::otlp::submission::{
    ExporterBackendId, Representation, Signal, TelemetryClientConfig, TelemetryConfigError,
    error_codes,
};

pub(super) fn invalid(field: &'static str) -> TelemetryConfigError {
    TelemetryConfigError::InvalidField {
        field,
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            &format!("invalid {field}; correct the telemetry configuration"),
        ),
    }
}
pub(super) fn invalid_reason(field: &'static str, reason: &str) -> TelemetryConfigError {
    TelemetryConfigError::InvalidField {
        field,
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            &format!("invalid {field}: {reason}"),
        ),
    }
}
pub(crate) fn otel_config_from(
    config: &TelemetryClientConfig,
) -> Result<OtelConfig, TelemetryConfigError> {
    config.validate()?;
    // No record exists at open: Logs/Log represent rejection of the entire backend.
    if config.backend != ExporterBackendId::SyncHttp {
        return Err(TelemetryConfigError::UnsupportedCombination {
            backend: config.backend,
            signal: Signal::Logs,
            representation: Representation::Log,
            context: super::context(
                error_codes::SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED,
                "durable submissions require the sync_http backend",
            ),
        });
    }
    let millis = u64::try_from(config.request_timeout.as_millis())
        .map_err(|_| invalid_reason("request_timeout", "milliseconds exceed u64"))?;
    let mut otel = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    otel.enabled = true;
    otel.endpoint = Some(
        OtlpEndpoint::new_typed(config.endpoint.clone())
            .map_err(|error| invalid_reason("endpoint", error.code().as_str()))?,
    );
    otel.auth_header = config
        .auth_header
        .as_ref()
        .map(|h| AuthHeader::new_typed(h.expose()).map_err(|_| invalid("auth_header")))
        .transpose()?;
    otel.timeout_ms = Some(millis.into());
    otel.sync_http_retry = config
        .sync_http_retry
        .as_ref()
        .map(|r| SyncHttpRetryPolicy {
            max_retries: r.max_retries,
            initial_backoff_ms: r.initial_backoff_ms.map(Into::into),
            max_backoff_ms: r.max_backoff_ms.map(Into::into),
            retry_sequence_timeout_ms: r.retry_sequence_timeout_ms.map(Into::into),
            retry_after_cap_ms: r.retry_after_cap_ms.map(Into::into),
            retry_jitter_percent: r.retry_jitter_percent,
        });
    Ok(otel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::otlp::submission::{
        ConfigOverrides, ConfigSources, Secret, SyncHttpRetryPolicyDto, resolve_config,
    };
    use std::time::Duration;
    #[test]
    fn maps_transport_and_all_six_retry_fields() {
        let mut overrides = ConfigOverrides::default();
        overrides.store_path = Some("adapter.db".into());
        overrides.endpoint = Some("http://localhost:4318".into());
        overrides.request_timeout = Some(Duration::from_millis(321));
        overrides.auth_header = Some(Secret::new("Bearer private".into()));
        let mut retry = SyncHttpRetryPolicyDto::default();
        retry.max_retries = Some(4);
        retry.initial_backoff_ms = Some(11);
        retry.max_backoff_ms = Some(99);
        retry.retry_sequence_timeout_ms = Some(4567);
        retry.retry_after_cap_ms = Some(1234);
        retry.retry_jitter_percent = Some(17);
        overrides.sync_http_retry = Some(retry);
        let config = resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap();
        let mapped = otel_config_from(&config).unwrap();
        assert!(mapped.enabled);
        assert_eq!(mapped.backend, ExporterBackend::SyncHttp);
        assert_eq!(mapped.protocol, OtlpProtocol::HttpJson);
        assert_eq!(mapped.endpoint.unwrap().as_str(), config.endpoint);
        assert_eq!(mapped.timeout_ms.unwrap().as_u64(), 321);
        assert_eq!(mapped.auth_header.unwrap().as_str(), "Bearer private");
        let retry = mapped.sync_http_retry.unwrap();
        assert_eq!(retry.max_retries, Some(4));
        assert_eq!(retry.initial_backoff_ms.unwrap().as_u64(), 11);
        assert_eq!(retry.max_backoff_ms.unwrap().as_u64(), 99);
        assert_eq!(retry.retry_sequence_timeout_ms.unwrap().as_u64(), 4567);
        assert_eq!(retry.retry_after_cap_ms.unwrap().as_u64(), 1234);
        assert_eq!(retry.retry_jitter_percent, Some(17));
        let mut bad = config;
        bad.sync_http_retry.as_mut().unwrap().retry_jitter_percent = Some(101);
        assert!(matches!(
            otel_config_from(&bad),
            Err(TelemetryConfigError::InvalidField {
                field: "sync_http_retry.retry_jitter_percent",
                ..
            })
        ));
    }
}
