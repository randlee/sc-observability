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

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::{constants, error_codes};
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::ConfigFailure;
#[allow(
    deprecated,
    reason = "OTLP config retains InitError in its published compatibility signatures"
)]
use sc_observability_types::{DurationMs, ErrorContext, InitError, Remediation, ServiceName};
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

/// A named transport field used in deterministic validation diagnostics.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OtlpConfigField {
    /// OTLP collector endpoint.
    Endpoint,
    /// Authorization or credential header.
    Header,
    /// Per-request timeout.
    Timeout,
    /// Lifecycle flush timeout.
    LifecycleFlushTimeout,
    /// Lifecycle shutdown timeout.
    LifecycleShutdownTimeout,
    /// Record admission capacity.
    QueueCapacity,
    /// Aggregate byte admission capacity.
    QueueByteCapacity,
    /// Legacy maximum retries.
    MaxRetries,
    /// Legacy initial retry backoff.
    InitialBackoff,
    /// Legacy maximum retry backoff.
    MaxBackoff,
    /// Legacy complete retry-sequence timeout.
    RetrySequenceTimeout,
    /// Legacy Retry-After cap.
    RetryAfterCap,
    /// Legacy jitter percentage.
    RetryJitterPercent,
    /// TLS certificate-verification override.
    InsecureSkipVerify,
}

impl OtlpConfigField {
    const fn stable_name(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Header => "auth_header",
            Self::Timeout => "timeout_ms",
            Self::LifecycleFlushTimeout => "lifecycle_flush_timeout_ms",
            Self::LifecycleShutdownTimeout => "lifecycle_shutdown_timeout_ms",
            Self::QueueCapacity => "queue_capacity",
            Self::QueueByteCapacity => "queue_byte_capacity",
            Self::MaxRetries => "legacy_retry.max_retries",
            Self::InitialBackoff => "legacy_retry.initial_backoff_ms",
            Self::MaxBackoff => "legacy_retry.max_backoff_ms",
            Self::RetrySequenceTimeout => "legacy_retry.retry_sequence_timeout_ms",
            Self::RetryAfterCap => "legacy_retry.retry_after_cap_ms",
            Self::RetryJitterPercent => "legacy_retry.retry_jitter_percent",
            Self::InsecureSkipVerify => "insecure_skip_verify",
        }
    }
}

/// Whether a resolved transport value came from the caller or the contract default.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueOrigin {
    /// Contract default.
    Default,
    /// Caller-supplied value.
    Explicit,
}

impl ValueOrigin {
    const fn stable_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Explicit => "explicit",
        }
    }
}

/// A resolved transport value with its stable field identity and source.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedField<T> {
    /// Field represented by this value.
    pub(crate) field: OtlpConfigField,
    /// Resolved value.
    pub(crate) value: T,
    /// Whether the value was explicit or defaulted.
    pub(crate) origin: ValueOrigin,
}

/// The target at which an inapplicable field was rejected.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OtlpConfigTarget {
    /// Transport is disabled and no backend is constructed.
    Disabled,
    /// A specific enabled backend was selected.
    Backend(ExporterBackend),
}

impl OtlpConfigTarget {
    fn stable_name(self) -> String {
        match self {
            Self::Disabled => "disabled".to_owned(),
            Self::Backend(backend) => format!("backend:{}", backend.stable_name()),
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
            .map_err(Into::into)
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
            .map_err(Into::into)
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
            .map_err(Into::into)
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
            .map_err(Into::into)
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
        self.build_typed().map_err(Into::into)
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

#[cfg(test)]
#[allow(
    deprecated,
    reason = "OTLP config compatibility tests exercise retained constructors and builder"
)]
pub(crate) fn validate_config(config: &TelemetryConfig) -> Result<(), InitError> {
    validate_config_typed(config).map_err(Into::into)
}

pub(crate) fn validate_config_typed(config: &TelemetryConfig) -> Result<(), InitFailure> {
    validated_telemetry_bounds(config).map(|_| ())
}

/// Validates a complete telemetry configuration once and returns its checked
/// transport bounds for factory construction.
pub(crate) fn validated_telemetry_bounds(
    config: &TelemetryConfig,
) -> Result<ValidatedTransportBounds, InitFailure> {
    let bounds =
        validated_transport_bounds(&config.transport).map_err(config_failure_to_init_failure)?;
    if config.transport.enabled && config.transport.endpoint.is_none() {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "enabled telemetry requires an endpoint",
            Remediation::recoverable(
                "set OtelConfig.endpoint before constructing Telemetry",
                ["disable telemetry for local-only runs if OTLP is not required"],
            ),
        ))));
    }
    if config.transport.enabled
        && config.logs.is_none()
        && config.traces.is_none()
        && config.metrics.is_none()
    {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "at least one telemetry signal must be enabled",
            Remediation::recoverable(
                "enable logs, traces, or metrics before constructing Telemetry",
                ["disable the OTLP layer entirely if telemetry is not needed"],
            ),
        ))));
    }
    if config.logs.is_some_and(|cfg| cfg.batch_size == 0)
        || config.traces.is_some_and(|cfg| cfg.batch_size == 0)
        || config
            .metrics
            .is_some_and(|cfg| cfg.batch_size == 0 || u64::from(cfg.export_interval_ms) == 0)
    {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "telemetry batch sizing and export intervals must be positive",
            Remediation::recoverable(
                "set batch sizes and export intervals above zero",
                ["use documented defaults"],
            ),
        ))));
    }
    Ok(bounds)
}

/// A duration checked as strictly positive by ordered config validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    pub(crate) const fn get(self) -> Duration {
        self.0
    }
}

/// Checked maximum number of simultaneously admitted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueCapacity(usize);

impl QueueCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Checked aggregate serialized-byte budget for admitted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueByteCapacity(usize);

impl QueueByteCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// A percentage validated within zero through one hundred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundedPercent(u8);

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "D.21 checked contract consumed by D.6-D.8")
)]
impl BoundedPercent {
    pub(crate) const fn get(self) -> u8 {
        self.0
    }
}

/// Positive lifecycle deadlines, both at least the request timeout.
#[derive(Debug)]
pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

impl LifecycleBounds {
    pub(crate) const fn flush(&self) -> PositiveDuration {
        self.flush
    }
    pub(crate) const fn shutdown(&self) -> PositiveDuration {
        self.shutdown
    }
}

/// Checked transport bounds; private fields prohibit unchecked factory construction.
#[derive(Debug)]
pub(crate) struct ValidatedTransportBounds {
    protocol: OtlpProtocol,
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

/// Connection values admitted by the same validation path as transport
/// bounds. Backend constructors consume this view instead of consulting
/// ambient `OTEL_*` configuration.
#[derive(Debug, Clone)]
pub(crate) struct ValidatedBackendConnection {
    endpoint: OtlpEndpoint,
    auth_header: Option<AuthHeader>,
    ca_file: Option<PathBuf>,
}

impl ValidatedBackendConnection {
    pub(crate) fn endpoint(&self) -> &OtlpEndpoint {
        &self.endpoint
    }

    pub(crate) fn auth_header(&self) -> Option<&AuthHeader> {
        self.auth_header.as_ref()
    }

    pub(crate) fn ca_file(&self) -> Option<&PathBuf> {
        self.ca_file.as_ref()
    }
}

impl ValidatedTransportBounds {
    pub(crate) const fn protocol(&self) -> OtlpProtocol {
        self.protocol
    }
    pub(crate) const fn queue_capacity(&self) -> QueueCapacity {
        self.queue_capacity
    }
    pub(crate) const fn queue_byte_capacity(&self) -> QueueByteCapacity {
        self.queue_byte_capacity
    }
    pub(crate) const fn request_timeout(&self) -> PositiveDuration {
        self.request_timeout
    }
    pub(crate) const fn lifecycle(&self) -> &LifecycleBounds {
        &self.lifecycle
    }
    pub(crate) const fn backend(&self) -> &BackendTransportBounds {
        &self.backend
    }
}

/// Backend-specific state; SDK and disabled transports cannot carry retry policy.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "D.21 checked contract consumed by D.6-D.8")
)]
#[derive(Debug)]
pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

/// Checked legacy retry policy produced only by ordered config validation.
#[derive(Debug)]
pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: PositiveDuration,
    max_backoff: PositiveDuration,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "D.21 checked contract consumed by D.6-D.8")
)]
impl RetryPolicy {
    pub(crate) const fn max_retries(&self) -> u32 {
        self.max_retries
    }
    pub(crate) const fn initial_backoff(&self) -> PositiveDuration {
        self.initial_backoff
    }
    pub(crate) const fn max_backoff(&self) -> PositiveDuration {
        self.max_backoff
    }
    pub(crate) const fn sequence_timeout(&self) -> PositiveDuration {
        self.sequence_timeout
    }
    pub(crate) const fn retry_after_cap(&self) -> PositiveDuration {
        self.retry_after_cap
    }
    pub(crate) const fn jitter(&self) -> BoundedPercent {
        self.jitter
    }
}

/// Resolves defaults and validates a transport in the documented first-failure
/// order. This is crate-visible for backend factories and contract tests.
#[expect(
    clippy::too_many_lines,
    reason = "the normative validation order is intentionally visible and linear"
)]
pub(crate) fn validated_transport_bounds(
    config: &OtelConfig,
) -> Result<ValidatedTransportBounds, ConfigFailure> {
    #[allow(deprecated)]
    let direct_legacy_fields = config.max_retries.is_some()
        || config.initial_backoff_ms.is_some()
        || config.max_backoff_ms.is_some();
    let legacy_retry_field = first_legacy_retry_field(config);
    #[allow(deprecated)]
    let direct_retry = LegacyRetryPolicy {
        max_retries: config.max_retries,
        initial_backoff_ms: config.initial_backoff_ms,
        max_backoff_ms: config.max_backoff_ms,
        ..LegacyRetryPolicy::default()
    };
    let timeout = resolve_duration(
        OtlpConfigField::Timeout,
        config.timeout_ms,
        constants::DEFAULT_OTLP_TIMEOUT_MS,
    );
    let flush = resolve_duration(
        OtlpConfigField::LifecycleFlushTimeout,
        config.lifecycle_flush_timeout_ms,
        constants::DEFAULT_OTLP_LIFECYCLE_FLUSH_TIMEOUT_MS,
    );
    let shutdown = resolve_duration(
        OtlpConfigField::LifecycleShutdownTimeout,
        config.lifecycle_shutdown_timeout_ms,
        constants::DEFAULT_OTLP_LIFECYCLE_SHUTDOWN_TIMEOUT_MS,
    );
    let queue_capacity = resolve_usize(
        OtlpConfigField::QueueCapacity,
        config.queue_capacity,
        constants::DEFAULT_OTLP_QUEUE_CAPACITY,
    );
    let queue_byte_capacity = resolve_usize(
        OtlpConfigField::QueueByteCapacity,
        config.queue_byte_capacity,
        constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY,
    );

    // The ordering below is normative: do not aggregate failures or move
    // checks without updating the D.21 contract tests.
    let request_timeout = checked_duration(&timeout)?;
    let lifecycle_flush_timeout = checked_duration(&flush)?;
    let lifecycle_shutdown_timeout = checked_duration(&shutdown)?;
    if shutdown.value < timeout.value {
        return Err(invalid_bound(&timeout, &shutdown));
    }
    if flush.value < timeout.value {
        return Err(invalid_bound(&timeout, &flush));
    }
    let legacy_retry =
        if config.enabled && matches!(config.backend, ExporterBackend::LegacyHttpJson) {
            Some(resolve_retry(
                config
                    .legacy_retry
                    .as_ref()
                    .or(direct_legacy_fields.then_some(&direct_retry)),
                &timeout,
            )?)
        } else {
            None
        };
    if !(1..=constants::MAX_OTLP_QUEUE_CAPACITY).contains(&queue_capacity.value) {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueCapacity,
            error_codes::OTLP_CONFIG_QUEUE_CAPACITY,
            format!(
                "queue capacity must be in 1..={}",
                constants::MAX_OTLP_QUEUE_CAPACITY
            ),
            queue_capacity.field,
            queue_capacity.origin,
        ));
    }
    if queue_byte_capacity.value == 0
        || queue_byte_capacity.value > constants::MAX_OTLP_QUEUE_BYTE_CAPACITY
    {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueByteCapacity,
            error_codes::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
            "queue byte capacity must be in 1..=64 MiB",
            queue_byte_capacity.field,
            queue_byte_capacity.origin,
        ));
    }

    let backend = if config.enabled {
        match config.backend {
            ExporterBackend::OpenTelemetrySdk => {
                if let Some(field) = legacy_retry_field {
                    return Err(not_applicable(
                        field,
                        OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk),
                    ));
                }
                BackendTransportBounds::Sdk
            }
            ExporterBackend::LegacyHttpJson => BackendTransportBounds::Legacy(
                legacy_retry.expect("legacy backend resolves its retry policy"),
            ),
        }
    } else {
        if let Some(field) = legacy_retry_field {
            return Err(not_applicable(field, OtlpConfigTarget::Disabled));
        }
        BackendTransportBounds::Disabled
    };

    if config.insecure_skip_verify && config.enabled {
        return Err(insecure_transport_rejected(config.backend));
    }

    Ok(ValidatedTransportBounds {
        protocol: config.protocol,
        queue_capacity: QueueCapacity(queue_capacity.value),
        queue_byte_capacity: QueueByteCapacity(queue_byte_capacity.value),
        request_timeout,
        lifecycle: LifecycleBounds {
            flush: lifecycle_flush_timeout,
            shutdown: lifecycle_shutdown_timeout,
        },
        backend,
    })
}

/// Returns the connection values only after the transport's ordinary ordered
/// validation has succeeded. Enabled factories need an explicit endpoint and
/// must never reconstruct it from environment defaults.
#[allow(
    dead_code,
    reason = "D.18 consumes the validated SDK connection view during facade composition"
)]
pub(crate) fn validated_backend_connection(
    config: &OtelConfig,
) -> Result<ValidatedBackendConnection, ConfigFailure> {
    let _ = validated_transport_bounds(config)?;
    let endpoint = config.endpoint.clone().ok_or_else(|| {
        invalid_endpoint(
            "enabled telemetry requires an endpoint",
            "set OtelConfig.endpoint before constructing the backend",
        )
    })?;
    Ok(ValidatedBackendConnection {
        endpoint,
        auth_header: config.auth_header.clone(),
        ca_file: config.ca_file.clone(),
    })
}

/// Returns the first legacy-only retry setting supplied by the caller.
///
/// The order is part of the deterministic validation contract. Retained
/// direct fields and their `legacy_retry` successors share the same identity,
/// so either representation reports the same first applicable field.
#[allow(
    deprecated,
    reason = "the selector preserves diagnostics for retained direct retry fields"
)]
fn first_legacy_retry_field(config: &OtelConfig) -> Option<OtlpConfigField> {
    let retry = config.legacy_retry.as_ref();
    if config.max_retries.is_some() || retry.is_some_and(|value| value.max_retries.is_some()) {
        return Some(OtlpConfigField::MaxRetries);
    }
    if config.initial_backoff_ms.is_some()
        || retry.is_some_and(|value| value.initial_backoff_ms.is_some())
    {
        return Some(OtlpConfigField::InitialBackoff);
    }
    if config.max_backoff_ms.is_some() || retry.is_some_and(|value| value.max_backoff_ms.is_some())
    {
        return Some(OtlpConfigField::MaxBackoff);
    }
    if retry.is_some_and(|value| value.retry_sequence_timeout_ms.is_some()) {
        return Some(OtlpConfigField::RetrySequenceTimeout);
    }
    if retry.is_some_and(|value| value.retry_after_cap_ms.is_some()) {
        return Some(OtlpConfigField::RetryAfterCap);
    }
    if retry.is_some_and(|value| value.retry_jitter_percent.is_some()) {
        return Some(OtlpConfigField::RetryJitterPercent);
    }

    // An explicitly supplied but empty compatibility block is still
    // inapplicable outside the legacy backend; retain the original field.
    config
        .legacy_retry
        .as_ref()
        .map(|_| OtlpConfigField::MaxRetries)
}

fn resolve_duration(
    field: OtlpConfigField,
    raw: Option<DurationMs>,
    default: u64,
) -> ResolvedField<u64> {
    ResolvedField {
        field,
        value: raw.map_or(default, u64::from),
        origin: if raw.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    }
}

fn resolve_usize(
    field: OtlpConfigField,
    raw: Option<usize>,
    default: usize,
) -> ResolvedField<usize> {
    ResolvedField {
        field,
        value: raw.unwrap_or(default),
        origin: if raw.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    }
}

fn checked_duration(value: &ResolvedField<u64>) -> Result<PositiveDuration, ConfigFailure> {
    if value.value == 0 {
        return Err(config_failure(
            ConfigFailureKind::ZeroDuration,
            error_codes::OTLP_CONFIG_ZERO_DURATION,
            "duration must be greater than zero",
            value.field,
            value.origin,
        ));
    }
    Ok(PositiveDuration(Duration::from_millis(value.value)))
}

fn resolve_retry(
    raw: Option<&LegacyRetryPolicy>,
    timeout: &ResolvedField<u64>,
) -> Result<RetryPolicy, ConfigFailure> {
    let raw = raw.cloned().unwrap_or_default();
    let initial = resolve_duration(
        OtlpConfigField::InitialBackoff,
        raw.initial_backoff_ms,
        constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS,
    );
    let maximum = resolve_duration(
        OtlpConfigField::MaxBackoff,
        raw.max_backoff_ms,
        constants::DEFAULT_OTLP_MAX_BACKOFF_MS,
    );
    let sequence = resolve_duration(
        OtlpConfigField::RetrySequenceTimeout,
        raw.retry_sequence_timeout_ms,
        constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS,
    );
    let after_cap = resolve_duration(
        OtlpConfigField::RetryAfterCap,
        raw.retry_after_cap_ms,
        constants::DEFAULT_OTLP_RETRY_AFTER_CAP_MS,
    );
    let jitter = ResolvedField {
        field: OtlpConfigField::RetryJitterPercent,
        value: raw
            .retry_jitter_percent
            .unwrap_or(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
        origin: if raw.retry_jitter_percent.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    };
    let initial_backoff = checked_duration(&initial)?;
    let max_backoff = checked_duration(&maximum)?;
    let sequence_timeout = checked_duration(&sequence)?;
    let retry_after_cap = checked_duration(&after_cap)?;
    if maximum.value < initial.value {
        return Err(invalid_bound(&initial, &maximum));
    }
    if sequence.value < timeout.value {
        return Err(invalid_bound(timeout, &sequence));
    }
    if after_cap.value > sequence.value {
        return Err(invalid_bound(&after_cap, &sequence));
    }
    if jitter.value > constants::MAX_OTLP_RETRY_JITTER_PERCENT {
        return Err(config_failure(
            ConfigFailureKind::InvalidJitterPercent,
            error_codes::OTLP_CONFIG_JITTER_PERCENT,
            format!(
                "retry jitter percent must be in 0..={}",
                constants::MAX_OTLP_RETRY_JITTER_PERCENT
            ),
            jitter.field,
            jitter.origin,
        ));
    }
    Ok(RetryPolicy {
        max_retries: raw
            .max_retries
            .unwrap_or(constants::DEFAULT_OTLP_MAX_RETRIES),
        initial_backoff,
        max_backoff,
        sequence_timeout,
        retry_after_cap,
        jitter: BoundedPercent(jitter.value),
    })
}

#[derive(Clone, Copy)]
enum ConfigFailureKind {
    ZeroDuration,
    InvalidJitterPercent,
    InvalidQueueCapacity,
    InvalidQueueByteCapacity,
}

fn invalid_bound(lower: &ResolvedField<u64>, upper: &ResolvedField<u64>) -> ConfigFailure {
    // Preserve both resolved fields in structured diagnostics without exposing
    // credentials or raw endpoint data.
    ConfigFailure::InvalidBoundOrdering {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_BOUND_ORDER,
                "resolved transport bounds are out of order",
                Remediation::recoverable(
                    "correct the named OTLP configuration fields",
                    ["use documented defaults"],
                ),
            )
            .detail("field", Value::String(lower.field.stable_name().to_owned()))
            .detail(
                "origin",
                Value::String(lower.origin.stable_name().to_owned()),
            )
            .detail("lower_value", Value::from(lower.value))
            .detail(
                "upper_field",
                Value::String(upper.field.stable_name().to_owned()),
            )
            .detail(
                "upper_origin",
                Value::String(upper.origin.stable_name().to_owned()),
            )
            .detail("upper_value", Value::from(upper.value)),
        ),
    }
}

fn not_applicable(field: OtlpConfigField, target: OtlpConfigTarget) -> ConfigFailure {
    ConfigFailure::ConfigFieldNotApplicable {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
                "configuration field is not applicable to the selected transport target",
                Remediation::recoverable(
                    "omit the field or choose an applicable backend",
                    ["use documented defaults"],
                ),
            )
            .detail("field", Value::String(field.stable_name().to_owned()))
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            )
            .detail("target", Value::String(target.stable_name())),
        ),
    }
}

fn config_failure(
    kind: ConfigFailureKind,
    code: sc_observability_types::ErrorCode,
    message: impl Into<String>,
    field: OtlpConfigField,
    origin: ValueOrigin,
) -> ConfigFailure {
    let context = Box::new(
        ErrorContext::new(
            code,
            message,
            Remediation::recoverable(
                "correct the named OTLP configuration field",
                ["use documented defaults"],
            ),
        )
        .detail("field", Value::String(field.stable_name().to_owned()))
        .detail("origin", Value::String(origin.stable_name().to_owned())),
    );
    match kind {
        ConfigFailureKind::ZeroDuration => ConfigFailure::ZeroDuration { context },
        ConfigFailureKind::InvalidJitterPercent => ConfigFailure::InvalidJitterPercent { context },
        ConfigFailureKind::InvalidQueueCapacity => ConfigFailure::InvalidQueueCapacity { context },
        ConfigFailureKind::InvalidQueueByteCapacity => {
            ConfigFailure::InvalidQueueByteCapacity { context }
        }
    }
}

fn insecure_transport_rejected(backend: ExporterBackend) -> ConfigFailure {
    ConfigFailure::InsecureTransportRejected {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
                "the selected backend does not support insecure certificate verification",
                Remediation::recoverable(
                    "leave certificate verification enabled",
                    ["use the documented TLS configuration"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::InsecureSkipVerify.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned())),
        ),
    }
}

fn config_failure_to_init_failure(error: ConfigFailure) -> InitFailure {
    InitFailure::from_context(error.into_context())
}

fn is_valid_http_endpoint(value: &str) -> bool {
    // Preserve the retained raw-value behavior for harmless trailing spaces,
    // while validating the URL-shaped portion and rejecting control bytes.
    if value.chars().any(char::is_control) {
        return false;
    }
    let value = value.trim_end();
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || authority.contains('\\')
        || authority.chars().any(char::is_whitespace)
    {
        return false;
    }

    if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((host, port)) = bracketed.split_once(']') else {
            return false;
        };
        return !host.is_empty()
            && (port.is_empty()
                || port.strip_prefix(':').is_some_and(|value| {
                    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
                }));
    }

    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        && port.is_none_or(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn invalid_endpoint(message: &str, remediation: &str) -> ConfigFailure {
    ConfigFailure::InvalidEndpoint {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INVALID_ENDPOINT,
                message,
                Remediation::recoverable(
                    remediation,
                    ["use the documented OTLP transport defaults"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::Endpoint.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            ),
        ),
    }
}

fn invalid_header(message: &str, remediation: &str) -> ConfigFailure {
    ConfigFailure::InvalidHeader {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INVALID_HEADER,
                message,
                Remediation::recoverable(
                    remediation,
                    ["use the documented OTLP transport defaults"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::Header.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            ),
        ),
    }
}

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
