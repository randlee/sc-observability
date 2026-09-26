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
use sc_observability_types::error_codes::otlp as otlp_error_codes;
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

/// Backend selected for an enabled OTLP transport.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExporterBackend {
    /// The asynchronous OpenTelemetry SDK backend.
    OpenTelemetrySdk,
    /// The reserved blocking HTTP/JSON backend.
    LegacyHttpJson,
}

/// A named transport field used in deterministic validation diagnostics.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpConfigField {
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
}

/// Whether a resolved transport value came from the caller or the contract default.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOrigin {
    /// Contract default.
    Default,
    /// Caller-supplied value.
    Explicit,
}

/// A resolved transport value with its stable field identity and source.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedField<T> {
    /// Field represented by this value.
    pub field: OtlpConfigField,
    /// Resolved value.
    pub value: T,
    /// Whether the value was explicit or defaulted.
    pub origin: ValueOrigin,
}

/// The target at which an inapplicable field was rejected.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpConfigTarget {
    /// Transport is disabled and no backend is constructed.
    Disabled,
    /// A specific enabled backend was selected.
    Backend(ExporterBackend),
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
    /// Jitter percentage in `0..=100`.
    pub retry_jitter_percent: Option<u8>,
}

/// Validated OTLP endpoint URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(String);

impl OtlpEndpoint {
    /// Creates a validated OTLP endpoint using the documented HTTP(S) schemes.
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use OtlpEndpoint::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        Self::new_typed(value).map_err(Into::into)
    }

    /// Creates a validated OTLP endpoint with a neutral initialization failure.
    ///
    /// Emptiness is checked against the trimmed value, but the original,
    /// untrimmed `value` is stored: this is intentional retained legacy
    /// behavior, not an oversight, and both the legacy [`OtlpEndpoint::new`]
    /// and this typed constructor preserve it identically. Callers that
    /// require a trimmed endpoint must trim before calling.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_transport_value_typed(
                "endpoint must not be empty",
                "set an explicit http:// or https:// OTLP endpoint",
            ));
        }
        if !(value.starts_with("http://") || value.starts_with("https://")) {
            return Err(invalid_transport_value_typed(
                "endpoint must start with http:// or https://",
                "set an OTLP endpoint with an explicit HTTP(S) scheme",
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
        Self::new_typed(value).map_err(Into::into)
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
        Self::new_typed(value).map_err(Into::into)
    }

    /// Creates a validated authorization header with a neutral initialization failure.
    ///
    /// Emptiness is checked against the trimmed value, but the original,
    /// untrimmed `value` is stored: this is intentional retained legacy
    /// behavior, not an oversight, and both the legacy [`AuthHeader::new`]
    /// and this typed constructor preserve it identically. Callers that
    /// require a trimmed header value must trim before calling.
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_transport_value_typed(
                "auth header must not be empty",
                "set a non-empty authorization header or omit it entirely",
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
        Self::new_typed(value).map_err(Into::into)
    }
}

/// Transport-level OTLP configuration.
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
    /// Per-export timeout.
    pub timeout_ms: DurationMs,
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
    /// Retained compatibility retry count; D.18 migrates callers to `legacy_retry`.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.max_retries")]
    pub max_retries: u32,
    /// Retained compatibility initial backoff; D.18 migrates callers to `legacy_retry`.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.initial_backoff_ms")]
    pub initial_backoff_ms: DurationMs,
    /// Retained compatibility maximum backoff; D.18 migrates callers to `legacy_retry`.
    #[deprecated(since = "2.0.0", note = "Use legacy_retry.max_backoff_ms")]
    pub max_backoff_ms: DurationMs,
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
            timeout_ms: constants::DEFAULT_OTLP_TIMEOUT_MS.into(),
            lifecycle_flush_timeout_ms: None,
            lifecycle_shutdown_timeout_ms: None,
            queue_capacity: None,
            queue_byte_capacity: None,
            debug_local_export: false,
            legacy_retry: None,
            max_retries: constants::DEFAULT_OTLP_MAX_RETRIES,
            initial_backoff_ms: constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS.into(),
            max_backoff_ms: constants::DEFAULT_OTLP_MAX_BACKOFF_MS.into(),
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
    validated_transport_bounds(&config.transport).map_err(config_failure_to_init_failure)?;
    if config.transport.enabled && config.transport.endpoint.is_none() {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
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
            error_codes::TELEMETRY_INVALID_CONFIG,
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
            error_codes::TELEMETRY_INVALID_CONFIG,
            "telemetry batch sizing and export intervals must be positive",
            Remediation::recoverable(
                "set batch sizes and export intervals above zero",
                ["use documented defaults"],
            ),
        ))));
    }
    Ok(())
}

/// Checked, backend-neutral transport bounds. Backend factories receive this
/// value rather than raw configuration so no adapter can reinterpret a wire
/// field or bypass the ordered validation contract.
#[allow(
    dead_code,
    reason = "D.21 validates these factory-only bounds before D.6-D.8 consume them"
)]
#[derive(Debug)]
pub(crate) struct ValidatedTransportBounds {
    pub(crate) queue_capacity: usize,
    pub(crate) queue_byte_capacity: usize,
    pub(crate) request_timeout: Duration,
    pub(crate) lifecycle_flush_timeout: Duration,
    pub(crate) lifecycle_shutdown_timeout: Duration,
    pub(crate) backend: BackendTransportBounds,
}

#[allow(
    dead_code,
    reason = "D.21 stages backend-neutral bounds before backend factories consume them"
)]
#[derive(Debug)]
pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(ValidatedRetryPolicy),
}

#[allow(
    dead_code,
    reason = "D.21 stages checked legacy policy values for the D.8 factory"
)]
#[derive(Debug)]
pub(crate) struct ValidatedRetryPolicy {
    pub(crate) max_retries: u32,
    pub(crate) initial_backoff: Duration,
    pub(crate) max_backoff: Duration,
    pub(crate) sequence_timeout: Duration,
    pub(crate) retry_after_cap: Duration,
    pub(crate) jitter_percent: u8,
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
    let direct_legacy_fields = config.max_retries != constants::DEFAULT_OTLP_MAX_RETRIES
        || u64::from(config.initial_backoff_ms) != constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS
        || u64::from(config.max_backoff_ms) != constants::DEFAULT_OTLP_MAX_BACKOFF_MS;
    let timeout = resolve_duration(
        OtlpConfigField::Timeout,
        Some(config.timeout_ms),
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
    if !(1..=constants::MAX_OTLP_QUEUE_CAPACITY).contains(&queue_capacity.value) {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueCapacity,
            otlp_error_codes::OTLP_CONFIG_QUEUE_CAPACITY,
            "queue capacity must be in 1..=65_536",
            queue_capacity.field,
            queue_capacity.origin,
        ));
    }
    if queue_byte_capacity.value == 0
        || queue_byte_capacity.value > constants::MAX_OTLP_QUEUE_BYTE_CAPACITY
    {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueByteCapacity,
            otlp_error_codes::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
            "queue byte capacity must be in 1..=64 MiB",
            queue_byte_capacity.field,
            queue_byte_capacity.origin,
        ));
    }

    let backend = if config.enabled {
        match config.backend {
            ExporterBackend::OpenTelemetrySdk => {
                if config.legacy_retry.is_some() || direct_legacy_fields {
                    return Err(not_applicable(
                        OtlpConfigField::MaxRetries,
                        OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk),
                    ));
                }
                BackendTransportBounds::Sdk
            }
            ExporterBackend::LegacyHttpJson => {
                #[allow(deprecated)]
                let direct_retry = LegacyRetryPolicy {
                    max_retries: Some(config.max_retries),
                    initial_backoff_ms: Some(config.initial_backoff_ms),
                    max_backoff_ms: Some(config.max_backoff_ms),
                    ..LegacyRetryPolicy::default()
                };
                let retry = resolve_retry(
                    config
                        .legacy_retry
                        .as_ref()
                        .or(direct_legacy_fields.then_some(&direct_retry)),
                    &timeout,
                )?;
                BackendTransportBounds::Legacy(retry)
            }
        }
    } else {
        if config.legacy_retry.is_some() || direct_legacy_fields {
            return Err(not_applicable(
                OtlpConfigField::MaxRetries,
                OtlpConfigTarget::Disabled,
            ));
        }
        BackendTransportBounds::Disabled
    };

    if config.insecure_skip_verify && config.enabled {
        return Err(config_failure(
            ConfigFailureKind::InsecureTransportRejected,
            otlp_error_codes::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
            "the selected backend does not support insecure certificate verification",
            OtlpConfigField::Endpoint,
            ValueOrigin::Explicit,
        ));
    }

    Ok(ValidatedTransportBounds {
        queue_capacity: queue_capacity.value,
        queue_byte_capacity: queue_byte_capacity.value,
        request_timeout,
        lifecycle_flush_timeout,
        lifecycle_shutdown_timeout,
        backend,
    })
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

fn checked_duration(value: &ResolvedField<u64>) -> Result<Duration, ConfigFailure> {
    if value.value == 0 {
        return Err(config_failure(
            ConfigFailureKind::ZeroDuration,
            otlp_error_codes::OTLP_CONFIG_ZERO_DURATION,
            "duration must be greater than zero",
            value.field,
            value.origin,
        ));
    }
    // `Duration::from_millis` is total for u64 inputs, but this explicit
    // checked conversion protects the contract if the representation changes.
    let seconds = value.value / 1_000;
    let nanos = (value.value % 1_000)
        .checked_mul(1_000_000)
        .ok_or_else(|| {
            config_failure(
                ConfigFailureKind::DurationOverflow,
                otlp_error_codes::OTLP_CONFIG_DURATION_OVERFLOW,
                "duration milliseconds overflow nanosecond conversion",
                value.field,
                value.origin,
            )
        })?;
    let nanos = u32::try_from(nanos).map_err(|_| {
        config_failure(
            ConfigFailureKind::DurationOverflow,
            otlp_error_codes::OTLP_CONFIG_DURATION_OVERFLOW,
            "duration milliseconds overflow nanosecond conversion",
            value.field,
            value.origin,
        )
    })?;
    Ok(Duration::new(seconds, nanos))
}

fn resolve_retry(
    raw: Option<&LegacyRetryPolicy>,
    timeout: &ResolvedField<u64>,
) -> Result<ValidatedRetryPolicy, ConfigFailure> {
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
    if jitter.value > 100 {
        return Err(config_failure(
            ConfigFailureKind::InvalidJitterPercent,
            otlp_error_codes::OTLP_CONFIG_JITTER_PERCENT,
            "retry jitter percent must be in 0..=100",
            jitter.field,
            jitter.origin,
        ));
    }
    Ok(ValidatedRetryPolicy {
        max_retries: raw
            .max_retries
            .unwrap_or(constants::DEFAULT_OTLP_MAX_RETRIES),
        initial_backoff,
        max_backoff,
        sequence_timeout,
        retry_after_cap,
        jitter_percent: jitter.value,
    })
}

#[derive(Clone, Copy)]
enum ConfigFailureKind {
    ZeroDuration,
    DurationOverflow,
    InvalidJitterPercent,
    InvalidQueueCapacity,
    InvalidQueueByteCapacity,
    InsecureTransportRejected,
}

fn invalid_bound(lower: &ResolvedField<u64>, upper: &ResolvedField<u64>) -> ConfigFailure {
    // Preserve both resolved fields in structured diagnostics without exposing
    // credentials or raw endpoint data.
    ConfigFailure::InvalidBoundOrdering {
        context: Box::new(
            ErrorContext::new(
                otlp_error_codes::OTLP_CONFIG_BOUND_ORDER,
                "resolved transport bounds are out of order",
                Remediation::recoverable(
                    "correct the named OTLP configuration fields",
                    ["use documented defaults"],
                ),
            )
            .detail(
                "field",
                Value::String(format!("{field:?}", field = lower.field)),
            )
            .detail(
                "origin",
                Value::String(format!("{origin:?}", origin = lower.origin)),
            )
            .detail("lower_value", Value::from(lower.value))
            .detail(
                "upper_field",
                Value::String(format!("{field:?}", field = upper.field)),
            )
            .detail(
                "upper_origin",
                Value::String(format!("{origin:?}", origin = upper.origin)),
            )
            .detail("upper_value", Value::from(upper.value)),
        ),
    }
}

fn not_applicable(field: OtlpConfigField, target: OtlpConfigTarget) -> ConfigFailure {
    ConfigFailure::ConfigFieldNotApplicable {
        context: Box::new(
            ErrorContext::new(
                otlp_error_codes::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
                "configuration field is not applicable to the selected transport target",
                Remediation::recoverable(
                    "omit the field or choose an applicable backend",
                    ["use documented defaults"],
                ),
            )
            .detail("field", Value::String(format!("{field:?}")))
            .detail("origin", Value::String("Explicit".to_owned()))
            .detail("target", Value::String(format!("{target:?}"))),
        ),
    }
}

fn config_failure(
    kind: ConfigFailureKind,
    code: sc_observability_types::ErrorCode,
    message: &str,
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
        .detail("field", Value::String(format!("{field:?}")))
        .detail("origin", Value::String(format!("{origin:?}"))),
    );
    match kind {
        ConfigFailureKind::ZeroDuration => ConfigFailure::ZeroDuration { context },
        ConfigFailureKind::DurationOverflow => ConfigFailure::DurationOverflow { context },
        ConfigFailureKind::InvalidJitterPercent => ConfigFailure::InvalidJitterPercent { context },
        ConfigFailureKind::InvalidQueueCapacity => ConfigFailure::InvalidQueueCapacity { context },
        ConfigFailureKind::InvalidQueueByteCapacity => {
            ConfigFailure::InvalidQueueByteCapacity { context }
        }
        ConfigFailureKind::InsecureTransportRejected => {
            ConfigFailure::InsecureTransportRejected { context }
        }
    }
}

fn config_failure_to_init_failure(error: ConfigFailure) -> InitFailure {
    InitFailure::from_context(error.into_context())
}

fn invalid_transport_value_typed(message: &str, remediation: &str) -> InitFailure {
    InitFailure::from_context(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_INVALID_CONFIG,
        message,
        Remediation::recoverable(remediation, ["use the documented OTLP transport defaults"]),
    )))
}

#[cfg(test)]
#[allow(
    deprecated,
    reason = "OTLP config compatibility tests exercise retained constructors and builder"
)]
mod tests {
    use super::*;
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

        assert_eq!(https.as_ref(), "https://otel.example.internal");
        assert_eq!(https.to_string(), "https://otel.example.internal");
        assert_eq!(http.as_str(), "http://localhost:4318");
    }

    #[test]
    fn otlp_endpoint_rejects_empty_or_scheme_less_values() {
        assert!(OtlpEndpoint::new("").is_err());
        assert!(OtlpEndpoint::new("otel.example.internal").is_err());
    }

    #[test]
    fn auth_header_rejects_empty_values() {
        assert!(AuthHeader::new("").is_err());
        assert!(AuthHeader::new("   ").is_err());
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
                timeout_ms: 0_u64.into(),
                ..transport()
            })
            .enable_logs(LogsConfig::default())
            .build()
            .expect_err("legacy zero timeout");
        let typed = TelemetryConfigBuilder::new(ServiceName::new("demo").expect("service"))
            .with_transport(OtelConfig {
                timeout_ms: 0_u64.into(),
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
                timeout_ms: 0_u64.into(),
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
                timeout_ms: 0_u64.into(),
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
            otlp_error_codes::OTLP_CONFIG_ZERO_DURATION
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
