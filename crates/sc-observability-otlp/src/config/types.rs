use std::fmt;
use std::path::PathBuf;

use super::validation::{
    config_failure_to_init_failure, invalid_endpoint, invalid_header, is_valid_http_endpoint,
    validate_config_typed,
};
use crate::constants;
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::{ConfigFailure, InitError};
use sc_observability_types::{DurationMs, ServiceName};
use serde_json::{Map, Value};

/// Supported OTLP transport protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    /// OTLP over HTTP with protobuf/binary payloads.
    HttpBinary,
    /// OTLP over HTTP with JSON payloads.
    HttpJson,
    /// OTLP over gRPC.
    Grpc,
}

impl OtlpProtocol {
    pub(crate) const fn stable_name(self) -> &'static str {
        match self {
            Self::HttpBinary => "http_binary",
            Self::HttpJson => "http_json",
            Self::Grpc => "grpc",
        }
    }
}

/// Backend selected for an enabled OTLP transport.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExporterBackend {
    /// The asynchronous OpenTelemetry SDK backend.
    OpenTelemetrySdk,
    /// The reserved blocking HTTP/JSON backend.
    LegacyHttpJson,
}

impl ExporterBackend {
    pub(crate) const fn stable_name(self) -> &'static str {
        match self {
            Self::OpenTelemetrySdk => "opentelemetry_sdk",
            Self::LegacyHttpJson => "legacy_http_json",
        }
    }
}

/// Legacy-only retry wire settings.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyRetryPolicy {
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Initial retry delay.
    pub initial_backoff_ms: Option<DurationMs>,
    /// Maximum retry delay.
    pub max_backoff_ms: Option<DurationMs>,
    /// Complete retry-sequence deadline.
    pub retry_sequence_timeout_ms: Option<DurationMs>,
    /// Upper bound for a server-supplied Retry-After delay.
    pub retry_after_cap_ms: Option<DurationMs>,
    /// Jitter percentage bounded by `MAX_OTLP_RETRY_JITTER_PERCENT`.
    pub retry_jitter_percent: Option<u8>,
}

/// Validated OTLP endpoint URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(String);

impl OtlpEndpoint {
    /// Creates a validated OTLP endpoint using the documented HTTP(S) subset.
    ///
    /// The endpoint grammar intentionally admits `http://` or `https://` URLs
    /// with an ASCII DNS host (letters, digits, `.` and `-`) or bracketed IPv6
    /// literal, each with an optional numeric port and path/query/fragment.
    /// It rejects URL userinfo, underscores, raw Unicode host names, and
    /// unbracketed IPv6 literals. Callers requiring broader URL support must
    /// normalize it before constructing this contract type.
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use OtlpEndpoint::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        Self::new_typed(value)
            .map_err(config_failure_to_init_failure)
            .map_err(|failure| InitError::Configuration {
                context: failure.into_context(),
            })
    }

    /// Creates a validated OTLP endpoint with a canonical configuration failure.
    ///
    /// Emptiness is checked against the trimmed value, but the original,
    /// untrimmed `value` is stored: this is intentional retained legacy
    /// behavior, not an oversight, and both the legacy [`OtlpEndpoint::new`]
    /// and this typed constructor preserve it identically. Callers that
    /// require a trimmed endpoint must trim before calling.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, ConfigFailure> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_endpoint(
                "endpoint must not be empty",
                "set an explicit http:// or https:// OTLP endpoint",
            ));
        }
        if !is_valid_http_endpoint(&value) {
            return Err(invalid_endpoint(
                "endpoint must be a valid http:// or https:// URL with a host",
                "set an OTLP endpoint with an explicit HTTP(S) scheme and host",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the validated endpoint as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
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
    #[allow(
        deprecated,
        reason = "TryFrom preserves the published InitError compatibility contract"
    )]
    type Error = InitError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new_typed(value)
            .map_err(config_failure_to_init_failure)
            .map_err(|failure| InitError::Configuration {
                context: failure.into_context(),
            })
    }
}

/// Validated authorization header value for OTLP transport.
///
/// `Debug` redacts the wrapped credential (`AuthHeader("<redacted>")`) so it
/// never leaks through `{:?}` formatting of this type or any config that
/// embeds it (for example [`OtelConfig`] and [`TelemetryConfig`]). The raw
/// value remains reachable only through the explicit, documented
/// [`AuthHeader::as_str`], `Display`, and `AsRef<str>` accessors — callers
/// that need the credential must opt in via one of those, not `{:?}`.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthHeader(String);

impl fmt::Debug for AuthHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuthHeader").field(&"<redacted>").finish()
    }
}

impl AuthHeader {
    /// Creates a validated non-empty authorization header value.
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use AuthHeader::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        Self::new_typed(value)
            .map_err(config_failure_to_init_failure)
            .map_err(|failure| InitError::Configuration {
                context: failure.into_context(),
            })
    }

    /// Creates a validated authorization header with a canonical configuration failure.
    ///
    /// Emptiness is checked against the trimmed value, but the original,
    /// untrimmed `value` is stored: this is intentional retained legacy
    /// behavior, not an oversight, and both the legacy [`AuthHeader::new`]
    /// and this typed constructor preserve it identically. Callers that
    /// require a trimmed header value must trim before calling.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, ConfigFailure> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_header(
                "auth header must not be empty",
                "set a non-empty authorization header or omit it entirely",
            ));
        }
        if value.chars().any(char::is_control) {
            return Err(invalid_header(
                "auth header must not contain control characters",
                "remove CR, LF, and other control characters from the authorization header",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the validated header as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
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
    #[allow(
        deprecated,
        reason = "TryFrom preserves the published InitError compatibility contract"
    )]
    type Error = InitError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new_typed(value)
            .map_err(config_failure_to_init_failure)
            .map_err(|failure| InitError::Configuration {
                context: failure.into_context(),
            })
    }
}

/// Transport-level OTLP configuration.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtelConfig {
    /// Whether transport/export is enabled.
    pub enabled: bool,
    /// Backend selected when transport is enabled.
    pub backend: ExporterBackend,
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
    /// Optional per-export timeout. `None` uses the documented default.
    pub timeout_ms: Option<DurationMs>,
    /// Time allowed for a lifecycle flush barrier.
    pub lifecycle_flush_timeout_ms: Option<DurationMs>,
    /// Time allowed for an ordered lifecycle shutdown.
    pub lifecycle_shutdown_timeout_ms: Option<DurationMs>,
    /// Bounded record admission capacity.
    pub queue_capacity: Option<usize>,
    /// Bounded aggregate serialized payload-byte admission capacity.
    pub queue_byte_capacity: Option<usize>,
    /// Whether local debug export output is enabled.
    pub debug_local_export: bool,
    /// Legacy-only retry settings. `None` means no legacy-only field was supplied.
    pub legacy_retry: Option<LegacyRetryPolicy>,
    /// Retained compatibility retry count. `None` means the field was not supplied.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.max_retries")]
    pub max_retries: Option<u32>,
    /// Retained compatibility initial backoff. `None` means the field was not supplied.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.initial_backoff_ms")]
    pub initial_backoff_ms: Option<DurationMs>,
    /// Retained compatibility maximum backoff. `None` means the field was not supplied.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.max_backoff_ms")]
    pub max_backoff_ms: Option<DurationMs>,
}

#[allow(
    deprecated,
    reason = "the retained compatibility fields must preserve their historical defaults until D.18"
)]
impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: ExporterBackend::OpenTelemetrySdk,
            endpoint: None,
            protocol: OtlpProtocol::HttpBinary,
            auth_header: None,
            ca_file: None,
            insecure_skip_verify: false,
            timeout_ms: None,
            lifecycle_flush_timeout_ms: None,
            lifecycle_shutdown_timeout_ms: None,
            queue_capacity: None,
            queue_byte_capacity: None,
            debug_local_export: false,
            legacy_retry: None,
            max_retries: None,
            initial_backoff_ms: None,
            max_backoff_ms: None,
        }
    }
}

impl OtelConfig {
    /// Starts an OTLP configuration for the selected backend and protocol.
    #[must_use]
    pub fn new(backend: ExporterBackend, protocol: OtlpProtocol) -> Self {
        Self {
            backend,
            protocol,
            ..Self::default()
        }
    }
}

/// Resource attributes attached to exported telemetry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceAttributes {
    /// Resource-level key/value attributes attached to exports.
    pub attributes: Map<String, Value>,
}

/// Log export batching configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogsConfig {
    /// Maximum logs per export batch.
    pub batch_size: usize,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            batch_size: constants::DEFAULT_LOG_BATCH_SIZE,
        }
    }
}

/// Trace export batching configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TracesConfig {
    /// Maximum complete spans per export batch.
    pub batch_size: usize,
}

impl Default for TracesConfig {
    fn default() -> Self {
        Self {
            batch_size: constants::DEFAULT_TRACE_BATCH_SIZE,
        }
    }
}

/// Metric export batching configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsConfig {
    /// Maximum metrics per export batch.
    pub batch_size: usize,
    /// Periodic export interval for metric flushes.
    pub export_interval_ms: DurationMs,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            batch_size: constants::DEFAULT_METRIC_BATCH_SIZE,
            export_interval_ms: constants::DEFAULT_METRIC_EXPORT_INTERVAL_MS.into(),
        }
    }
}

/// Application-owned telemetry configuration.
///
/// A configuration with `logs`, `traces`, and `metrics` all set to `None` is
/// valid for a disabled or not-yet-configured telemetry instance. When
/// `transport.enabled` is `false`, callers may construct `TelemetryConfig`
/// without enabling any signal exporters and still build `Telemetry`
/// successfully.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryConfig {
    /// Service name attached to all exported telemetry.
    pub service_name: ServiceName,
    /// Resource attributes attached to all exported telemetry.
    pub resource: ResourceAttributes,
    /// Transport-level OTLP configuration.
    pub transport: OtelConfig,
    /// Optional log export configuration.
    pub logs: Option<LogsConfig>,
    /// Optional trace export configuration.
    pub traces: Option<TracesConfig>,
    /// Optional metric export configuration.
    pub metrics: Option<MetricsConfig>,
}

/// Builder for documented v1 telemetry defaults.
#[allow(
    missing_debug_implementations,
    reason = "the builder stores partially configured runtime values, and a public Debug surface would not add meaningful API value"
)]
pub struct TelemetryConfigBuilder {
    service_name: ServiceName,
    resource: ResourceAttributes,
    transport: OtelConfig,
    logs: Option<LogsConfig>,
    traces: Option<TracesConfig>,
    metrics: Option<MetricsConfig>,
}

impl TelemetryConfigBuilder {
    /// Starts a builder from the required service name.
    pub fn new(service_name: ServiceName) -> Self {
        Self {
            service_name,
            resource: ResourceAttributes::default(),
            transport: OtelConfig::default(),
            logs: None,
            traces: None,
            metrics: None,
        }
    }

    /// Overrides the resource attributes attached to exports.
    pub fn with_resource(mut self, resource: ResourceAttributes) -> Self {
        self.resource = resource;
        self
    }

    /// Overrides the transport configuration.
    pub fn with_transport(mut self, transport: OtelConfig) -> Self {
        self.transport = transport;
        self
    }

    /// Enables log export with the provided batch policy.
    pub fn enable_logs(mut self, config: LogsConfig) -> Self {
        self.logs = Some(config);
        self
    }

    /// Enables trace export with the provided batch policy.
    pub fn enable_traces(mut self, config: TracesConfig) -> Self {
        self.traces = Some(config);
        self
    }

    /// Enables metric export with the provided batch policy.
    pub fn enable_metrics(mut self, config: MetricsConfig) -> Self {
        self.metrics = Some(config);
        self
    }

    /// Finalizes the telemetry configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_observability_otlp::TelemetryConfigBuilder;
    /// use sc_observability_types::ServiceName;
    ///
    /// let config = TelemetryConfigBuilder::new(
    ///     ServiceName::new("demo").expect("valid service"),
    /// )
    /// .build()
    /// .expect("valid telemetry config");
    ///
    /// assert_eq!(config.service_name.as_str(), "demo");
    /// ```
    #[allow(
        deprecated,
        reason = "retained compatibility builder keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use TelemetryConfigBuilder::build_typed(); see migrate-error-api.md."
    )]
    pub fn build(self) -> Result<TelemetryConfig, InitError> {
        self.build_typed()
            .map_err(|failure| InitError::Configuration {
                context: failure.into_context(),
            })
    }

    /// Finalizes the telemetry configuration with a neutral initialization failure.
    pub fn build_typed(self) -> Result<TelemetryConfig, InitFailure> {
        let config = TelemetryConfig {
            service_name: self.service_name,
            resource: self.resource,
            transport: self.transport,
            logs: self.logs,
            traces: self.traces,
            metrics: self.metrics,
        };
        validate_config_typed(&config)?;
        Ok(config)
    }
}
