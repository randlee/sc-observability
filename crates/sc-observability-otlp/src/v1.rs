//! Deprecated 1.x facade retained only for the Phase F migration window.
//!
//! These owners translate the released root API into the canonical v2
//! configuration and runtime. They contain no exporter or lifecycle logic.
#![expect(
    clippy::missing_errors_doc,
    reason = "compatibility errors follow the released OTLP API documentation"
)]
#![allow(
    deprecated,
    reason = "v1 intentionally contains the deprecated migration path"
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

#[deprecated(note = "removed; see docs/migration/phase-f.md")]
pub use crate::assembly::CompleteSpan;
#[deprecated(note = "removed; see docs/migration/phase-f.md")]
pub use crate::assembly::SpanAssembler;
use crate::config::{ExporterBackend, SyncHttpRetryPolicy, TelemetryConfig as RuntimeConfig};
use crate::projectors::V2TelemetryProjectors;
use crate::{RuntimeTelemetry, constants, error_codes};
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::{
    FlushError, LogProjector, MetricProjector, MetricRecord, ObservationFilter,
    ProjectionRegistration, ShutdownError, SpanProjector,
    TelemetryError as CanonicalTelemetryError,
};
#[allow(deprecated)]
use sc_observability_types::{
    DurationMs, ErrorContext, EventError, InitError, LogEvent, Observable, Remediation,
    ServiceName, SpanSignal,
};

/// Released root telemetry error alias retained for the 1.x migration window.
#[deprecated(note = "use sc_observability_otlp::v2::TelemetryError")]
pub type TelemetryError = sc_observability_types::TelemetryError;

/// The released 1.4.1 OTLP protocol set.
#[deprecated(note = "use sc_observability_otlp::v2::OtlpProtocol")]
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
#[deprecated(note = "use sc_observability_otlp::v2::OtlpEndpoint")]
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
#[deprecated(note = "use sc_observability_otlp::v2::AuthHeader")]
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
#[deprecated(note = "use sc_observability_otlp::v2::OtelConfig")]
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
            OtlpProtocol::HttpJson => ExporterBackend::SyncHttp,
        };
        let budget = DurationMs::from(
            u64::from(self.timeout_ms).max(constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS),
        );
        let retry = SyncHttpRetryPolicy {
            max_retries: Some(self.max_retries),
            initial_backoff_ms: Some(self.initial_backoff_ms),
            max_backoff_ms: Some(self.max_backoff_ms),
            retry_sequence_timeout_ms: Some(budget),
            ..SyncHttpRetryPolicy::default()
        };
        let sync_http_retry =
            (self.enabled && backend == ExporterBackend::SyncHttp).then_some(retry);
        // Canonical retry values apply only to the enabled HTTP/JSON synchronous
        // backend. Released retry bounds are validated before this projection,
        // for every protocol and enabled state, as required by 1.4.1.
        crate::config::OtelConfig {
            enabled: self.enabled,
            backend,
            endpoint: self.endpoint.map(|endpoint| endpoint.0),
            protocol: self.protocol.into(),
            auth_header: self.auth_header.map(|header| header.0),
            ca_file: self.ca_file,
            // The released flag was accepted but inert in 1.4.1.  Compatibility
            // construction must retain that behavior; canonical construction
            // continues to reject an explicit insecure TLS configuration.
            insecure_skip_verify: false,
            timeout_ms: Some(self.timeout_ms),
            lifecycle_flush_timeout_ms: Some(budget),
            lifecycle_shutdown_timeout_ms: Some(budget),
            debug_local_export: self.debug_local_export,
            sync_http_retry,
            ..crate::config::OtelConfig::default()
        }
    }
}

/// Released telemetry configuration literal.
#[deprecated(note = "use sc_observability_otlp::v2::TelemetryConfig")]
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
#[deprecated(note = "use sc_observability_otlp::v2::TelemetryConfigBuilder")]
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
        validate_released_config(&config)?;
        Ok(config)
    }
}

/// Validates the published 1.4.1 configuration in its original failure order.
///
/// This intentionally precedes backend projection: released callers must see
/// their released diagnostic rather than a canonical transport diagnostic.
fn validate_released_config(config: &TelemetryConfig) -> Result<(), InitFailure> {
    let transport = &config.transport;
    if transport.enabled && transport.endpoint.is_none() {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::TELEMETRY_INVALID_CONFIG,
            "enabled telemetry requires an endpoint",
            Remediation::recoverable(
                "set OtelConfig.endpoint before constructing Telemetry",
                ["disable telemetry for local-only runs if OTLP is not required"],
            ),
        ))));
    }
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
    if transport.enabled
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
    if config.logs.is_some_and(|value| value.batch_size == 0)
        || config.traces.is_some_and(|value| value.batch_size == 0)
        || config
            .metrics
            .is_some_and(|value| value.batch_size == 0 || u64::from(value.export_interval_ms) == 0)
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

/// Released root facade over the single canonical telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "the runtime state has no stable Debug contract"
)]
#[deprecated(note = "use sc_observability_otlp::v2::Telemetry")]
pub struct Telemetry {
    inner: Arc<RuntimeTelemetry>,
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
        validate_released_config(&config)?;
        let config = config.into_runtime();
        let bounds = crate::config::validated_telemetry_bounds(&config)?;
        Ok(
            RuntimeTelemetry::new_prepared(config, &bounds).map(|inner| Self {
                inner: Arc::new(inner),
            })?,
        )
    }

    /// Buffers one log event for export.
    pub fn emit_log(&self, event: &LogEvent) -> Result<(), sc_observability_types::TelemetryError> {
        self.inner.emit_log(event).map_err(legacy_telemetry_error)
    }

    /// Buffers one span signal for export.
    pub fn emit_span(
        &self,
        span: &SpanSignal,
    ) -> Result<(), sc_observability_types::TelemetryError> {
        let span = v1_span_signal(span).map_err(|error| {
            legacy_telemetry_error(CanonicalTelemetryError::ExportFailure(error))
        })?;
        self.inner.emit_span(&span).map_err(legacy_telemetry_error)
    }

    /// Buffers one metric record for export.
    pub fn emit_metric(&self, metric: &MetricRecord) -> Result<(), CanonicalTelemetryError> {
        self.inner.emit_metric(metric)
    }

    /// Flushes all configured exporters with the released error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observability_otlp::v2::Telemetry::flush; see docs/migration/phase-f.md"
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_typed()
    }

    /// Flushes all configured exporters with the typed error.
    #[deprecated(
        note = "use sc_observability_otlp::v2::Telemetry::flush; see docs/migration/phase-f.md"
    )]
    pub fn flush_typed(&self) -> Result<(), FlushError> {
        self.inner.flush()
    }

    /// Awaits the canonical lifecycle flush barrier.
    #[deprecated(
        note = "use sc_observability_otlp::v2::Telemetry::flush_async; see docs/migration/phase-f.md"
    )]
    pub async fn flush_async_typed(&self) -> Result<(), FlushError> {
        self.inner.flush_async().await
    }

    /// Shuts down all configured exporters with the released error type.
    #[allow(deprecated)]
    #[deprecated(
        since = "1.4.0",
        note = "use sc_observability_otlp::v2::Telemetry::shutdown; see docs/migration/phase-f.md"
    )]
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_typed()
    }

    /// Shuts down all configured exporters with the typed error.
    #[deprecated(
        note = "use sc_observability_otlp::v2::Telemetry::shutdown; see docs/migration/phase-f.md"
    )]
    pub fn shutdown_typed(&self) -> Result<(), ShutdownError> {
        self.inner.shutdown()
    }

    /// Awaits canonical graceful shutdown of all configured exporters.
    #[deprecated(
        note = "use sc_observability_otlp::v2::Telemetry::shutdown_async; see docs/migration/phase-f.md"
    )]
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownError> {
        self.inner.shutdown_async().await
    }

    /// Returns the current exporter health report.
    pub fn health(&self) -> crate::TelemetryHealthReport {
        self.inner.health()
    }
}

fn v1_span_signal(
    signal: &SpanSignal,
) -> Result<sc_observability_types::v2::SpanSignal, sc_observability_types::v2::ExportError> {
    use sc_observability_types::v2::SpanSignal as Canonical;
    Ok(match signal {
        SpanSignal::Started(record) => Canonical::Started(v1_started_record(record)),
        SpanSignal::Event(event) => Canonical::Event(sc_observability_types::v2::SpanEvent {
            timestamp: event.timestamp,
            trace: v1_trace_context(&event.trace),
            name: event.name.clone(),
            attributes: v1_attributes(&event.attributes),
            diagnostic: event.diagnostic.clone(),
        }),
        SpanSignal::Ended(record) => {
            let duration = record
                .duration_ms()
                .ok_or_else(|| v1_transport_error("completed span has no duration"))?;
            Canonical::Ended(
                v1_started_record(record).end(v1_span_status(record.status()), duration),
            )
        }
    })
}

fn v1_started_record<S>(
    record: &sc_observability_types::SpanRecord<S>,
) -> sc_observability_types::v2::SpanRecord<sc_observability_types::v2::SpanStarted> {
    let started = sc_observability_types::v2::SpanRecord::new(
        record.timestamp(),
        record.service().clone(),
        record.name().clone(),
        v1_trace_context(record.trace()),
        v1_attributes(record.attributes()),
    );
    match record.diagnostic().cloned() {
        Some(diagnostic) => started.with_diagnostic(diagnostic),
        None => started,
    }
}

fn v1_span_status(
    status: sc_observability_types::SpanStatus,
) -> sc_observability_types::v2::SpanStatus {
    match status {
        sc_observability_types::SpanStatus::Ok => sc_observability_types::v2::SpanStatus::Ok,
        sc_observability_types::SpanStatus::Error => sc_observability_types::v2::SpanStatus::Error,
        sc_observability_types::SpanStatus::Unset => sc_observability_types::v2::SpanStatus::Unset,
    }
}

fn v1_trace_context(
    trace: &sc_observability_types::TraceContext,
) -> sc_observability_types::v2::TraceContext {
    let context = sc_observability_types::v2::TraceContext::new(
        trace.trace_id.clone(),
        trace.span_id.clone(),
        sc_observability_types::v2::TraceFlags::default(),
    );
    match trace.parent_span_id.clone() {
        Some(parent) => context.with_parent(parent),
        None => context,
    }
}

fn v1_attributes(
    values: &serde_json::Map<String, serde_json::Value>,
) -> sc_observability_types::v2::Attributes {
    values
        .iter()
        .map(|(key, value)| (key.clone(), v1_attribute(value)))
        .collect()
}

fn v1_attribute(value: &serde_json::Value) -> sc_observability_types::v2::AttributeValue {
    use sc_observability_types::v2::{AttributeValue, FiniteF64};
    match value {
        serde_json::Value::Null => AttributeValue::Null,
        serde_json::Value::Bool(value) => AttributeValue::Bool(*value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(AttributeValue::Int)
            .or_else(|| value.as_u64().map(AttributeValue::UInt))
            .or_else(|| {
                value
                    .as_f64()
                    .and_then(|value| FiniteF64::new(value).ok())
                    .map(AttributeValue::Float)
            })
            .unwrap_or(AttributeValue::Null),
        serde_json::Value::String(value) => AttributeValue::String(value.clone()),
        serde_json::Value::Array(values) => {
            AttributeValue::Array(values.iter().map(v1_attribute).collect())
        }
        serde_json::Value::Object(values) => AttributeValue::Object(v1_attributes(values)),
    }
}

fn v1_transport_error(message: &str) -> sc_observability_types::v2::ExportError {
    sc_observability_types::v2::ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            sc_observability_types::error_codes::otlp::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("correct the signal before exporting"),
        )),
    }
}

/// Public helper for attaching the released telemetry API to observation projectors.
#[expect(
    missing_debug_implementations,
    reason = "the helper stores trait-object projectors and filters whose internal state is not part of the public debug contract"
)]
#[deprecated(note = "use sc_observability_otlp::v2::TelemetryProjectors")]
pub struct TelemetryProjectors<T>
where
    T: Observable,
{
    inner: V2TelemetryProjectors<T>,
}

impl<T> TelemetryProjectors<T>
where
    T: Observable,
{
    /// Starts a wrapped projector set for one observation payload type.
    pub fn new(telemetry: &Arc<Telemetry>) -> Self {
        Self {
            inner: V2TelemetryProjectors::new(Arc::clone(&telemetry.inner)),
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
        CanonicalTelemetryError::Event(error) => {
            TelemetryError::ExportFailure(error.into_context())
        }
        _ => TelemetryError::ExportFailure(error.into_context()),
    }
}

impl crate::SpanAssembler {
    /// Pushes one lifecycle signal through the assembler.
    #[allow(
        deprecated,
        reason = "retained compatibility assembler method keeps the published EventError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use SpanAssembler::push_typed(); see migrate-error-api.md."
    )]
    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError> {
        self.push_typed(signal).map_err(Into::into)
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

    // These private value-level guards check selected backend, timeout, and retry/backoff
    // projection: `released_http_json_uses_the_nested_sync_http_retry_policy` and
    // `released_sdk_and_disabled_transports_discard_sync_http_retry_settings`. The external
    // `released_config_translation` tests cover released-bound validation, not direct
    // inspection of this private projection or live collector/runtime behavior.
    #[test]
    fn released_http_json_uses_the_nested_sync_http_retry_policy() {
        let runtime = released_transport(OtlpProtocol::HttpJson, true).into_runtime();

        assert_eq!(runtime.backend, ExporterBackend::SyncHttp);
        assert_eq!(runtime.timeout_ms, Some(DurationMs::from(750)));
        assert_eq!(
            runtime.sync_http_retry,
            Some(SyncHttpRetryPolicy {
                max_retries: Some(7),
                initial_backoff_ms: Some(DurationMs::from(125)),
                max_backoff_ms: Some(DurationMs::from(875)),
                retry_sequence_timeout_ms: Some(DurationMs::from(30_000)),
                ..SyncHttpRetryPolicy::default()
            })
        );
    }

    #[test]
    fn released_projection_retains_zero_and_large_timeout_with_inert_tls() {
        for timeout in [750_u64, 60_001] {
            let mut transport = released_transport(OtlpProtocol::HttpJson, true);
            transport.endpoint = Some(OtlpEndpoint::new_typed("https://localhost:4318").unwrap());
            transport.timeout_ms = timeout.into();
            transport.initial_backoff_ms = 0_u64.into();
            transport.max_backoff_ms = 0_u64.into();
            transport.insecure_skip_verify = true;
            let runtime = TelemetryConfig {
                service_name: ServiceName::new("released-projection").unwrap(),
                resource: crate::ResourceAttributes::default(),
                transport,
                logs: Some(crate::LogsConfig::default()),
                traces: None,
                metrics: None,
            }
            .into_runtime();
            assert!(!runtime.transport.insecure_skip_verify);
            let bounds = crate::config::validated_test_telemetry_bounds(&runtime).unwrap();
            let crate::config::BackendTransportBounds::SyncHttp(retry) = bounds.backend() else {
                panic!("enabled released HttpJson has retry bounds");
            };
            let budget = std::time::Duration::from_millis(
                timeout.max(constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS),
            );
            assert_eq!(
                bounds.request_timeout().get(),
                std::time::Duration::from_millis(timeout)
            );
            assert_eq!(bounds.lifecycle().flush().get(), budget);
            assert_eq!(bounds.lifecycle().shutdown().get(), budget);
            assert_eq!(retry.sequence_timeout().get(), budget);
            assert_eq!(retry.initial_backoff().get(), std::time::Duration::ZERO);
            assert_eq!(retry.max_backoff().get(), std::time::Duration::ZERO);
            assert!(crate::config::validated_telemetry_bounds(&runtime).is_err());
        }
    }

    #[test]
    fn released_sdk_and_disabled_transports_discard_sync_http_retry_settings() {
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
                ExporterBackend::SyncHttp,
            ),
        ] {
            let runtime = transport.into_runtime();

            assert_eq!(runtime.backend, backend);
            assert_eq!(runtime.timeout_ms, Some(DurationMs::from(750)));
            assert_eq!(runtime.sync_http_retry, None);
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
            if enabled {
                transport.endpoint = Some(
                    OtlpEndpoint::new_typed("http://127.0.0.1:4318")
                        .expect("valid released endpoint"),
                );
            }
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
    fn released_constructor_validates_literals_before_runtime_projection() {
        for (protocol, enabled, timeout_ms, initial_backoff_ms, max_backoff_ms, message) in [
            (
                OtlpProtocol::HttpBinary,
                true,
                750_u64,
                300_u64,
                200_u64,
                "initial_backoff_ms must not exceed max_backoff_ms",
            ),
            (
                OtlpProtocol::HttpJson,
                false,
                0_u64,
                125_u64,
                875_u64,
                "timeout_ms must be greater than zero",
            ),
        ] {
            let mut transport = released_transport(protocol, enabled);
            if enabled {
                transport.endpoint = Some(
                    OtlpEndpoint::new_typed("http://127.0.0.1:4318")
                        .expect("valid released endpoint"),
                );
            }
            transport.timeout_ms = DurationMs::from(timeout_ms);
            transport.initial_backoff_ms = DurationMs::from(initial_backoff_ms);
            transport.max_backoff_ms = DurationMs::from(max_backoff_ms);

            let result = Telemetry::new_typed(TelemetryConfig {
                service_name: ServiceName::new("released-constructor-validation")
                    .expect("valid service name"),
                resource: crate::ResourceAttributes::default(),
                transport,
                logs: Some(crate::LogsConfig::default()),
                traces: None,
                metrics: None,
            });
            let Err(error) = result else {
                panic!("released constructor must validate literal configuration");
            };

            assert_eq!(
                error.diagnostic().code,
                error_codes::TELEMETRY_INVALID_CONFIG
            );
            assert_eq!(error.diagnostic().message, message);
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
        assert!(runtime.sync_http_retry.is_none());
    }
}
