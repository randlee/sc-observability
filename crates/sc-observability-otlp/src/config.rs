//! OTLP configuration types, builder defaults, and construction-time validation.
//!
//! This module defines the caller-facing telemetry config surface used to build
//! a `Telemetry` runtime, including transport options, per-signal batch
//! settings, and the eager validation rules enforced at initialization time.

use std::path::PathBuf;

use crate::{constants, error_codes};
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

/// Validated OTLP endpoint URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(String);

impl OtlpEndpoint {
    /// Creates a validated OTLP endpoint using the documented HTTP(S) schemes.
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_transport_value(
                "endpoint must not be empty",
                "set an explicit http:// or https:// OTLP endpoint",
            ));
        }
        if !(value.starts_with("http://") || value.starts_with("https://")) {
            return Err(invalid_transport_value(
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

/// Validated authorization header value for OTLP transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthHeader(String);

impl AuthHeader {
    /// Creates a validated non-empty authorization header value.
    pub fn new(value: impl Into<String>) -> Result<Self, InitError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(invalid_transport_value(
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

/// Transport-level OTLP configuration.
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

    /// Disables log export.
    #[expect(
        dead_code,
        reason = "builder keeps explicit crate-local disable toggles for test and internal composition paths"
    )]
    pub(crate) fn disable_logs(mut self) -> Self {
        self.logs = None;
        self
    }

    /// Enables trace export with the provided batch policy.
    pub fn enable_traces(mut self, config: TracesConfig) -> Self {
        self.traces = Some(config);
        self
    }

    /// Disables trace export.
    #[expect(
        dead_code,
        reason = "builder keeps explicit crate-local disable toggles for test and internal composition paths"
    )]
    pub(crate) fn disable_traces(mut self) -> Self {
        self.traces = None;
        self
    }

    /// Enables metric export with the provided batch policy.
    pub fn enable_metrics(mut self, config: MetricsConfig) -> Self {
        self.metrics = Some(config);
        self
    }

    /// Disables metric export.
    #[expect(
        dead_code,
        reason = "builder keeps explicit crate-local disable toggles for test and internal composition paths"
    )]
    pub(crate) fn disable_metrics(mut self) -> Self {
        self.metrics = None;
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
    pub fn build(self) -> Result<TelemetryConfig, InitError> {
        let config = TelemetryConfig {
            service_name: self.service_name,
            resource: self.resource,
            transport: self.transport,
            logs: self.logs,
            traces: self.traces,
            metrics: self.metrics,
        };
        validate_config(&config)?;
        Ok(config)
    }
}

pub(crate) fn validate_config(config: &TelemetryConfig) -> Result<(), InitError> {
    if config.transport.enabled && config.transport.endpoint.is_none() {
        return Err(InitError(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "enabled telemetry requires an endpoint",
            Remediation::recoverable(
                "set OtelConfig.endpoint before constructing Telemetry",
                ["disable telemetry for local-only runs if OTLP is not required"],
            ),
        ))));
    }
    if u64::from(config.transport.timeout_ms) == 0 {
        return Err(InitError(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "timeout_ms must be greater than zero",
            Remediation::recoverable(
                "set timeout_ms to a positive value",
                ["use documented defaults"],
            ),
        ))));
    }
    if config.transport.initial_backoff_ms > config.transport.max_backoff_ms {
        return Err(InitError(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "initial_backoff_ms must not exceed max_backoff_ms",
            Remediation::recoverable("fix the backoff configuration", ["use documented defaults"]),
        ))));
    }
    if config.transport.enabled
        && config.logs.is_none()
        && config.traces.is_none()
        && config.metrics.is_none()
    {
        return Err(InitError(Box::new(ErrorContext::new(
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
        return Err(InitError(Box::new(ErrorContext::new(
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

fn invalid_transport_value(message: &str, remediation: &str) -> InitError {
    InitError(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_INVALID_CONFIG,
        message,
        Remediation::recoverable(remediation, ["use the documented OTLP transport defaults"]),
    )))
}
