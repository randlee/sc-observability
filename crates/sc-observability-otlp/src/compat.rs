//! Removable facade for the released 1.4.1 OTLP surface.
//!
//! These owners translate the released root API into the canonical v2
//! configuration and runtime. They contain no exporter or lifecycle logic.
#![expect(
    clippy::missing_errors_doc,
    reason = "compatibility errors follow the released OTLP API documentation"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "the released accessors and builder methods intentionally keep the original annotations"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "the released builder methods intentionally keep the original annotations"
)]

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::{self, ExporterBackend, LegacyRetryPolicy, TelemetryConfig as RuntimeConfig};
use crate::projectors::{ProjectorSet, TelemetryEmit};
use crate::{RuntimeTelemetry, constants, error_codes};
use sc_observability_types::typed::{FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
#[allow(deprecated)]
use sc_observability_types::{
    DurationMs, ErrorContext, FlushError, InitError, LogEvent, LogProjector, MetricProjector,
    MetricRecord, Observable, ObservationFilter, ProjectionRegistration, Remediation, ServiceName,
    ShutdownError, SpanProjector, SpanSignal, TelemetryError,
};

/// The released 1.4.1 OTLP protocol set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    /// OTLP over HTTP with protobuf/binary payloads.
    HttpBinary,
    /// OTLP over HTTP with JSON payloads.
    HttpJson,
    /// OTLP over gRPC.
    Grpc,
}

impl From<OtlpProtocol> for crate::config::OtlpProtocol {
    fn from(value: OtlpProtocol) -> Self {
        match value {
            OtlpProtocol::HttpBinary => Self::HttpBinary,
            OtlpProtocol::HttpJson => Self::HttpJson,
            OtlpProtocol::Grpc => Self::Grpc,
        }
    }
}

/// Released validated endpoint owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(crate::config::OtlpEndpoint);

impl OtlpEndpoint {
    /// Creates a validated endpoint with the released initialization error.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use OtlpEndpoint::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        Self::new_typed(value).map_err(Into::into)
    }

    /// Creates a validated endpoint with the released typed initialization error.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure> {
        crate::config::OtlpEndpoint::new_typed(value)
            .map(Self)
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Returns the endpoint string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for OtlpEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for OtlpEndpoint {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for OtlpEndpoint {
    #[allow(deprecated)]
    type Error = InitError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new_typed(value).map_err(Into::into)
    }
}

/// Released validated authorization-header owner.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthHeader(crate::config::AuthHeader);

impl fmt::Debug for AuthHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuthHeader").field(&"<redacted>").finish()
    }
}

impl AuthHeader {
    /// Creates a validated header with the released initialization error.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use AuthHeader::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        Self::new_typed(value).map_err(Into::into)
    }

    /// Creates a validated header with the released typed initialization error.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure> {
        crate::config::AuthHeader::new_typed(value)
            .map(Self)
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Returns the raw header value.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for AuthHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for AuthHeader {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for AuthHeader {
    #[allow(deprecated)]
    type Error = InitError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new_typed(value).map_err(Into::into)
    }
}

/// Released 1.4.1 transport configuration literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtelConfig {
    /// Whether transport/export is enabled.
    pub enabled: bool,
    /// Optional OTLP endpoint URL.
    pub endpoint: Option<OtlpEndpoint>,
    /// Transport protocol to use.
    pub protocol: OtlpProtocol,
    /// Optional authorization header value.
    pub auth_header: Option<AuthHeader>,
    /// Optional CA bundle path for TLS validation.
    pub ca_file: Option<PathBuf>,
    /// Whether TLS certificate verification is skipped.
    pub insecure_skip_verify: bool,
    /// Per-export timeout.
    pub timeout_ms: DurationMs,
    /// Whether local debug export output is enabled.
    pub debug_local_export: bool,
    /// Maximum export retry attempts.
    pub max_retries: u32,
    /// Initial retry backoff.
    pub initial_backoff_ms: DurationMs,
    /// Maximum retry backoff.
    pub max_backoff_ms: DurationMs,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            protocol: OtlpProtocol::HttpBinary,
            auth_header: None,
            ca_file: None,
            insecure_skip_verify: false,
            timeout_ms: constants::DEFAULT_OTLP_TIMEOUT_MS.into(),
            debug_local_export: false,
            max_retries: constants::DEFAULT_OTLP_MAX_RETRIES,
            initial_backoff_ms: constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS.into(),
            max_backoff_ms: constants::DEFAULT_OTLP_MAX_BACKOFF_MS.into(),
        }
    }
}

impl OtelConfig {
    fn into_runtime(self) -> crate::config::OtelConfig {
        let backend = match self.protocol {
            OtlpProtocol::HttpBinary | OtlpProtocol::Grpc => ExporterBackend::OpenTelemetrySdk,
            OtlpProtocol::HttpJson => ExporterBackend::LegacyHttpJson,
        };
        let retry = LegacyRetryPolicy {
            max_retries: Some(self.max_retries),
            initial_backoff_ms: Some(self.initial_backoff_ms),
            max_backoff_ms: Some(self.max_backoff_ms),
            ..LegacyRetryPolicy::default()
        };
        let legacy_retry =
            (self.enabled && backend == ExporterBackend::LegacyHttpJson).then_some(retry);
        // Canonical retry values apply only to the enabled HTTP/JSON legacy
        // backend. Released retry bounds are validated before this projection,
        // for every protocol and enabled state, as required by 1.4.1.
        crate::config::OtelConfig {
            enabled: self.enabled,
            backend,
            endpoint: self.endpoint.map(|endpoint| endpoint.0),
            protocol: self.protocol.into(),
            auth_header: self.auth_header.map(|header| header.0),
            ca_file: self.ca_file,
            insecure_skip_verify: self.insecure_skip_verify,
            timeout_ms: Some(self.timeout_ms),
            debug_local_export: self.debug_local_export,
            legacy_retry,
            ..crate::config::OtelConfig::default()
        }
    }
}

/// Released telemetry configuration literal.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryConfig {
    /// Service name attached to all exported telemetry.
    pub service_name: ServiceName,
    /// Resource attributes attached to all exported telemetry.
    pub resource: crate::ResourceAttributes,
    /// Transport-level OTLP configuration.
    pub transport: OtelConfig,
    /// Optional log export configuration.
    pub logs: Option<crate::LogsConfig>,
    /// Optional trace export configuration.
    pub traces: Option<crate::TracesConfig>,
    /// Optional metric export configuration.
    pub metrics: Option<crate::MetricsConfig>,
}

impl TelemetryConfig {
    fn into_runtime(self) -> RuntimeConfig {
        RuntimeConfig {
            service_name: self.service_name,
            resource: self.resource,
            transport: self.transport.into_runtime(),
            logs: self.logs,
            traces: self.traces,
            metrics: self.metrics,
        }
    }
}

/// Builder retaining the published 1.4.1 construction and validation surface.
#[expect(
    missing_debug_implementations,
    reason = "the released builder never exposed Debug"
)]
pub struct TelemetryConfigBuilder {
    service_name: ServiceName,
    resource: crate::ResourceAttributes,
    transport: OtelConfig,
    logs: Option<crate::LogsConfig>,
    traces: Option<crate::TracesConfig>,
    metrics: Option<crate::MetricsConfig>,
}

impl TelemetryConfigBuilder {
    /// Starts a builder from the required service name.
    pub fn new(service_name: ServiceName) -> Self {
        Self {
            service_name,
            resource: crate::ResourceAttributes::default(),
            transport: OtelConfig::default(),
            logs: None,
            traces: None,
            metrics: None,
        }
    }

    /// Overrides the resource attributes attached to exports.
    pub fn with_resource(mut self, resource: crate::ResourceAttributes) -> Self {
        self.resource = resource;
        self
    }

    /// Overrides the released transport configuration.
    pub fn with_transport(mut self, transport: OtelConfig) -> Self {
        self.transport = transport;
        self
    }

    /// Enables log export with the provided batch policy.
    pub fn enable_logs(mut self, config: crate::LogsConfig) -> Self {
        self.logs = Some(config);
        self
    }

    /// Enables trace export with the provided batch policy.
    pub fn enable_traces(mut self, config: crate::TracesConfig) -> Self {
        self.traces = Some(config);
        self
    }

    /// Enables metric export with the provided batch policy.
    pub fn enable_metrics(mut self, config: crate::MetricsConfig) -> Self {
        self.metrics = Some(config);
        self
    }

    /// Finalizes the released configuration with its deprecated error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use TelemetryConfigBuilder::build_typed(); see migrate-error-api.md."
    )]
    pub fn build(self) -> Result<TelemetryConfig, InitError> {
        self.build_typed().map_err(Into::into)
    }

    /// Finalizes and validates the released configuration.
    pub fn build_typed(self) -> Result<TelemetryConfig, InitFailure> {
        let config = TelemetryConfig {
            service_name: self.service_name,
            resource: self.resource,
            transport: self.transport,
            logs: self.logs,
            traces: self.traces,
            metrics: self.metrics,
        };
        validate_released_transport(&config.transport)?;
        config::validate_config_typed(&config.clone().into_runtime())?;
        Ok(config)
    }
}

/// Validates released transport fields before backend-specific projection.
fn validate_released_transport(transport: &OtelConfig) -> Result<(), InitFailure> {
    if u64::from(transport.timeout_ms) == 0 {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "timeout_ms must be greater than zero",
            Remediation::recoverable(
                "set timeout_ms to a positive value",
                ["use documented defaults"],
            ),
        ))));
    }
    if transport.initial_backoff_ms > transport.max_backoff_ms {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "initial_backoff_ms must not exceed max_backoff_ms",
            Remediation::recoverable("fix the backoff configuration", ["use documented defaults"]),
        ))));
    }
    Ok(())
}

/// Released root facade over the single canonical telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "the runtime state has no stable Debug contract"
)]
pub struct Telemetry {
    inner: RuntimeTelemetry,
}

impl Telemetry {
    /// Creates telemetry with the released initialization error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(config: TelemetryConfig) -> Result<Self, InitError> {
        Self::new_typed(config).map_err(Into::into)
    }

    /// Creates telemetry with the released typed initialization error.
    pub fn new_typed(config: TelemetryConfig) -> Result<Self, InitFailure> {
        RuntimeTelemetry::new_typed(config.into_runtime()).map(|inner| Self { inner })
    }

    pub(crate) fn runtime(&self) -> &RuntimeTelemetry {
        &self.inner
    }

    /// Buffers one log event for export.
    pub fn emit_log(&self, event: &LogEvent) -> Result<(), TelemetryError> {
        self.inner.emit_log(event).map_err(legacy_telemetry_error)
    }

    /// Buffers one span signal for export.
    pub fn emit_span(&self, span: &SpanSignal) -> Result<(), TelemetryError> {
        self.inner.emit_span(span).map_err(legacy_telemetry_error)
    }

    /// Buffers one metric record for export.
    pub fn emit_metric(&self, metric: &MetricRecord) -> Result<(), TelemetryError> {
        self.inner
            .emit_metric(metric)
            .map_err(legacy_telemetry_error)
    }

    /// Flushes all configured exporters with the released error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::flush_typed(); see migrate-error-api.md."
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_typed().map_err(Into::into)
    }

    /// Flushes all configured exporters with the typed error.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.inner.flush_typed()
    }

    /// Awaits the canonical lifecycle flush barrier.
    pub async fn flush_async_typed(&self) -> Result<(), FlushFailure> {
        self.inner.flush_async_typed().await
    }

    /// Shuts down all configured exporters with the released error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::shutdown_typed(); see migrate-error-api.md."
    )]
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_typed().map_err(Into::into)
    }

    /// Shuts down all configured exporters with the typed error.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        self.inner.shutdown_typed()
    }

    /// Awaits canonical graceful shutdown of all configured exporters.
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownFailure> {
        self.inner.shutdown_async_typed().await
    }

    /// Returns the current exporter health report.
    pub fn health(&self) -> crate::TelemetryHealthReport {
        self.inner.health()
    }
}

impl TelemetryEmit for Telemetry {
    fn emit_log(&self, event: &LogEvent) -> Result<(), CanonicalTelemetryError> {
        self.runtime().emit_log(event)
    }

    fn emit_span(&self, span: &SpanSignal) -> Result<(), CanonicalTelemetryError> {
        self.runtime().emit_span(span)
    }

    fn emit_metric(&self, metric: &MetricRecord) -> Result<(), CanonicalTelemetryError> {
        self.runtime().emit_metric(metric)
    }
}

/// Public helper for attaching the released telemetry API to observation projectors.
#[expect(
    missing_debug_implementations,
    reason = "the helper stores trait-object projectors and filters whose internal state is not part of the public debug contract"
)]
pub struct TelemetryProjectors<T>
where
    T: Observable,
{
    inner: ProjectorSet<T, Telemetry>,
}

impl<T> TelemetryProjectors<T>
where
    T: Observable,
{
    /// Starts a wrapped projector set for one observation payload type.
    pub fn new(telemetry: Arc<Telemetry>) -> Self {
        Self {
            inner: ProjectorSet::new(telemetry),
        }
    }

    /// Attaches a log projector whose output is also forwarded into telemetry.
    pub fn with_log_projector(mut self, projector: Arc<dyn LogProjector<T>>) -> Self {
        self.inner = self.inner.with_log_projector(projector);
        self
    }

    /// Attaches a span projector whose output is also forwarded into telemetry.
    pub fn with_span_projector(mut self, projector: Arc<dyn SpanProjector<T>>) -> Self {
        self.inner = self.inner.with_span_projector(projector);
        self
    }

    /// Attaches a metric projector whose output is also forwarded into telemetry.
    pub fn with_metric_projector(mut self, projector: Arc<dyn MetricProjector<T>>) -> Self {
        self.inner = self.inner.with_metric_projector(projector);
        self
    }

    /// Attaches the filter the wrapped projector registration should honor.
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.inner = self.inner.with_filter(filter);
        self
    }

    /// Converts the wrapped helper into ordinary observation registration.
    pub fn into_registration(self) -> ProjectionRegistration<T> {
        self.inner.into_registration()
    }
}

#[allow(deprecated)]
fn legacy_telemetry_error(error: CanonicalTelemetryError) -> TelemetryError {
    #[allow(deprecated)]
    match error {
        CanonicalTelemetryError::Shutdown { .. } => TelemetryError::Shutdown,
        CanonicalTelemetryError::ExportFailure(error) => {
            TelemetryError::ExportFailure(error.into_context())
        }
        _ => TelemetryError::ExportFailure(error.into_context()),
    }
}

impl crate::telemetry_health_provider_sealed::Sealed for Telemetry {
    fn token(&self) -> crate::telemetry_health_provider_sealed::Token {
        crate::telemetry_health_provider_sealed::workspace_token()
    }
}

impl sc_observability_types::ObservabilityHealthProvider for Telemetry {
    fn telemetry_health(&self) -> crate::TelemetryHealthReport {
        self.health()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::DiagnosticInfo;

    fn released_transport(protocol: OtlpProtocol, enabled: bool) -> OtelConfig {
        OtelConfig {
            enabled,
            protocol,
            timeout_ms: DurationMs::from(750),
            max_retries: 7,
            initial_backoff_ms: DurationMs::from(125),
            max_backoff_ms: DurationMs::from(875),
            ..OtelConfig::default()
        }
    }

    #[test]
    fn released_http_json_uses_the_nested_legacy_retry_policy() {
        let runtime = released_transport(OtlpProtocol::HttpJson, true).into_runtime();

        assert_eq!(runtime.backend, ExporterBackend::LegacyHttpJson);
        assert_eq!(runtime.timeout_ms, Some(DurationMs::from(750)));
        assert_eq!(
            runtime.legacy_retry,
            Some(LegacyRetryPolicy {
                max_retries: Some(7),
                initial_backoff_ms: Some(DurationMs::from(125)),
                max_backoff_ms: Some(DurationMs::from(875)),
                ..LegacyRetryPolicy::default()
            })
        );
    }

    #[test]
    fn released_sdk_and_disabled_transports_discard_legacy_retry_settings() {
        for (transport, backend) in [
            (
                released_transport(OtlpProtocol::HttpBinary, true),
                ExporterBackend::OpenTelemetrySdk,
            ),
            (
                released_transport(OtlpProtocol::Grpc, true),
                ExporterBackend::OpenTelemetrySdk,
            ),
            (
                released_transport(OtlpProtocol::HttpJson, false),
                ExporterBackend::LegacyHttpJson,
            ),
        ] {
            let runtime = transport.into_runtime();

            assert_eq!(runtime.backend, backend);
            assert_eq!(runtime.timeout_ms, Some(DurationMs::from(750)));
            assert_eq!(runtime.legacy_retry, None);
        }
    }

    #[test]
    fn released_builder_validates_bounds_before_backend_specific_retry_projection() {
        for (protocol, enabled) in [
            (OtlpProtocol::HttpBinary, true),
            (OtlpProtocol::Grpc, true),
            (OtlpProtocol::HttpJson, true),
            (OtlpProtocol::HttpBinary, false),
            (OtlpProtocol::HttpJson, false),
        ] {
            let mut transport = released_transport(protocol, enabled);
            transport.initial_backoff_ms = DurationMs::from(300);
            transport.max_backoff_ms = DurationMs::from(200);

            let error = TelemetryConfigBuilder::new(
                ServiceName::new("released-config-validation").expect("valid service name"),
            )
            .with_transport(transport)
            .enable_logs(crate::LogsConfig::default())
            .build_typed()
            .expect_err("released bounds must be checked before retry projection");

            assert_eq!(
                error.diagnostic().code,
                error_codes::TELEMETRY_INVALID_CONFIG
            );
            assert_eq!(
                error.diagnostic().message,
                "initial_backoff_ms must not exceed max_backoff_ms"
            );
        }
    }

    #[test]
    fn released_grpc_protocol_uses_the_canonical_sdk_adapter() {
        let runtime = OtelConfig {
            protocol: OtlpProtocol::Grpc,
            ..OtelConfig::default()
        }
        .into_runtime();

        assert_eq!(runtime.backend, ExporterBackend::OpenTelemetrySdk);
        assert_eq!(runtime.protocol, crate::config::OtlpProtocol::Grpc);
        assert!(runtime.legacy_retry.is_none());
    }
}
