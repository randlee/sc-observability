//! OTLP-backed telemetry layered on top of `sc-observe`.
//!
//! This crate owns telemetry configuration, span assembly, exporter contracts,
//! and the lifecycle/runtime behavior for OTLP-bound signals. It attaches to
//! routing through ordinary projector registration and keeps OpenTelemetry
//! transport concerns out of the lower crates.
#![expect(
    clippy::missing_errors_doc,
    reason = "telemetry-facade error behavior is documented centrally in workspace docs, and repeating it on every wrapper method adds low-signal boilerplate"
)]

mod assembly;
mod compat;
mod config;
#[cfg(test)]
mod contract_tests;
mod contracts;
#[cfg(test)]
#[allow(
    deprecated,
    reason = "telemetry compatibility tests exercise retained lifecycle and error wrappers"
)]
mod facade_tests;
mod legacy_projection;
mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
mod projectors;
mod testing;

#[cfg(feature = "legacy-http-json")]
mod legacy_http_json;
#[cfg(feature = "otlp-sdk")]
mod sdk;

pub mod constants;
pub mod error_codes;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use config::{
    BackendTransportBounds, TelemetryConfig as RuntimeTelemetryConfig, ValidatedBackendConnection,
    ValidatedTransportBounds, prepared_backend_connection, validated_telemetry_bounds,
};
#[cfg(test)]
use config::{validate_config_typed, validated_transport_bounds};
use sc_observability_types::typed::{EventFailure, FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
use sc_observability_types::v2::{ConfigFailure, EventError as CanonicalEventError, ExportError};
use sc_observability_types::v2::{
    MetricRecord as CanonicalMetricRecord, SpanSignal as CanonicalSpanSignal,
};
use sc_observability_types::{
    DiagnosticSummary, ErrorContext, LogEvent, MetricKind, MetricRecord,
    ObservabilityHealthProvider, Remediation, SinkName, SpanSignal,
    telemetry_health_provider_sealed,
};
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryError, TelemetryHealthReport,
    TelemetryHealthState,
};
use serde_json::Value;

#[doc(inline)]
pub use assembly::{CompleteSpan, SpanAssembler, SpanAssemblyLoss};
#[doc(inline)]
pub use compat::{
    AuthHeader, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry, TelemetryConfig,
    TelemetryConfigBuilder, TelemetryProjectors,
};
#[doc(inline)]
pub use config::{
    ExporterBackend, LegacyRetryPolicy, LogsConfig, MetricsConfig, ResourceAttributes, TracesConfig,
};
#[doc(inline)]
pub use projectors::V2TelemetryProjectors;

/// Opt-in canonical OTLP facade for the compatible 1.x transition.
///
/// This namespace re-exports the existing telemetry implementation and does
/// not introduce a second backend, configuration authority, or lifecycle.
pub mod v2 {
    #[doc(inline)]
    pub use crate::RuntimeTelemetry as Telemetry;
    #[doc(inline)]
    pub use crate::V2TelemetryProjectors as TelemetryProjectors;
    #[doc(inline)]
    pub use crate::config::{
        AuthHeader, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
        ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
    };
    #[doc(inline)]
    pub use crate::{ExporterBackend, LegacyRetryPolicy};
    #[doc(inline)]
    pub use sc_observability_types::v2::{
        ConfigFailure, EventError, FlushError, InitError, ShutdownError, TelemetryError,
    };
}
#[cfg(feature = "sdk-test-support")]
#[doc(inline)]
pub use sdk::SdkFixture;

use assembly::V2SpanAssembler;
use contracts::{
    ExportRecord, ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, LogRecord,
    MetricExporter, TraceExporter,
};
#[cfg(test)]
use legacy_projection::trace_context;
#[allow(
    unused_imports,
    reason = "transport construction failures are mapped only by enabled backends"
)]
use legacy_projection::transport_construction_failure;
use lifecycle::{LifecycleHealth, LifecycleState, SignalKind};

/// Metric admitted to the shared canonical buffer.
enum BufferedMetric {
    /// Canonical record, exported with its full aggregation.
    Canonical(Box<ExportRecord<CanonicalMetricRecord>>),
    /// Released scalar record, kept in its released shape until flush, where
    /// the released facade has always reported conversion failures.
    Released(MetricRecord),
}

/// OTLP-backed telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "telemetry owns exporter trait objects and runtime state that are intentionally not exposed through a stable Debug contract"
)]
pub struct RuntimeTelemetry {
    config: RuntimeTelemetryConfig,
    exporters: ExporterSet,
    // MUTEX: exporter flush/shutdown paths mutate buffers and per-signal runtime health together;
    // Mutex keeps the buffered state and last_error snapshot consistent, and RwLock would not help
    // because these operations are write-heavy critical sections.
    runtime: Mutex<TelemetryRuntime>,
    dropped_exports_total: AtomicU64,
    bounded_assembly_drops_total: AtomicU64,
    malformed_spans_total: AtomicU64,
}

#[derive(Debug)]
struct FlushOutcome {
    /// The final exporter failure in deterministic flush order. Health retains
    /// summaries for every failing exporter, while shutdown keeps this owned
    /// value so callers can traverse its native source chain.
    export_failure: Option<ExportError>,
}

#[derive(Default)]
struct TelemetryRuntime {
    span_assembler: V2SpanAssembler,
    log_buffer: Vec<ExportRecord<LogRecord>>,
    span_buffer: Vec<ExportRecord<contracts::CompleteSpan>>,
    metric_buffer: Vec<BufferedMetric>,
    log_status: ExporterRuntime,
    trace_status: ExporterRuntime,
    metric_status: ExporterRuntime,
    last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone)]
struct ExporterRuntime {
    state: ExporterHealthState,
    last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone, Copy)]
enum ExporterKind {
    Logs,
    Traces,
    Metrics,
}

impl ExporterKind {
    fn status_mut(self, runtime: &mut TelemetryRuntime) -> &mut ExporterRuntime {
        match self {
            Self::Logs => &mut runtime.log_status,
            Self::Traces => &mut runtime.trace_status,
            Self::Metrics => &mut runtime.metric_status,
        }
    }
}

impl Default for ExporterRuntime {
    fn default() -> Self {
        Self {
            state: ExporterHealthState::Healthy,
            last_error: None,
        }
    }
}

/// Merges facade-local assembly state with the lifecycle core's terminal loss ownership.
fn merged_lifecycle_status(
    runtime: &ExporterRuntime,
    lifecycle: Option<&LifecycleHealth>,
    signal: SignalKind,
) -> ExporterRuntime {
    let mut status = runtime.clone();
    if lifecycle.is_some_and(|health| health.dropped_for(signal) > 0) {
        status.state = ExporterHealthState::Degraded;
        if status.last_error.is_none() {
            status.last_error = lifecycle.and_then(|health| health.last_error.clone());
        }
    }
    status
}

static LOGS_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("logs").expect("logs exporter name is valid"));
static TRACES_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("traces").expect("traces exporter name is valid"));
static METRICS_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("metrics").expect("metrics exporter name is valid"));

/// Explicit disabled-transport exporters. They are never selected for an
/// enabled backend; the factory rejects enabled selections until D.6-D.8
/// supply their concrete exporter sets.
struct DisabledLogExporter;
struct DisabledTraceExporter;
struct DisabledMetricExporter;
struct DisabledLifecycle {
    shutdown: AtomicBool,
}

impl LogExporter for DisabledLogExporter {
    fn export_logs(&self, _batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl TraceExporter for DisabledTraceExporter {
    fn export_spans(
        &self,
        _batch: &[ExportRecord<contracts::CompleteSpan>],
    ) -> Result<(), ExportError> {
        Ok(())
    }
}

impl MetricExporter for DisabledMetricExporter {
    fn export_metrics(
        &self,
        _batch: &[ExportRecord<CanonicalMetricRecord>],
    ) -> Result<(), ExportError> {
        Ok(())
    }
}

impl ExporterLifecycle for DisabledLifecycle {
    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.shutdown.store(true, Ordering::Release);
        Box::pin(async { Ok(()) })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.shutdown.store(true, Ordering::Release);
        Ok(())
    }
}

/// Consumes only fully validated transport bounds before selecting one common
/// exporter shape. Protocol, feature, and caller-runtime availability are
/// deliberately checked here, after the configuration's normative ordered
/// validation, so an unavailable backend cannot mask a malformed config.
#[cfg(test)]
fn exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
) -> Result<ExporterSet, ConfigFailure> {
    exporter_factory_prepared(config, bounds)
}

fn exporter_factory_prepared(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
) -> Result<ExporterSet, ConfigFailure> {
    match bounds.backend() {
        BackendTransportBounds::Disabled => Ok(ExporterSet {
            logs: Arc::new(DisabledLogExporter),
            traces: Arc::new(DisabledTraceExporter),
            metrics: Arc::new(DisabledMetricExporter),
            lifecycle: Arc::new(DisabledLifecycle {
                shutdown: AtomicBool::new(false),
            }),
        }),
        BackendTransportBounds::Sdk => {
            let connection = prepared_backend_connection(&config.transport, bounds)?;
            sdk_exporter_factory(config, bounds, &connection)
        }
        BackendTransportBounds::Legacy(_) => {
            let connection = prepared_backend_connection(&config.transport, bounds)?;
            legacy_exporter_factory(config, bounds, &connection)
        }
    }
}

#[allow(unused_variables)]
fn sdk_exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
    connection: &ValidatedBackendConnection,
) -> Result<ExporterSet, ConfigFailure> {
    if !matches!(
        bounds.protocol(),
        config::OtlpProtocol::Grpc | config::OtlpProtocol::HttpBinary
    ) {
        return Err(unsupported_protocol(
            config::ExporterBackend::OpenTelemetrySdk,
            bounds.protocol(),
            "Grpc, HttpBinary",
        ));
    }

    #[cfg(feature = "otlp-sdk")]
    {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(ConfigFailure::TokioRuntimeRequired {
                context: Box::new(
                    ErrorContext::new(
                        sc_observability_types::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED,
                        "the OpenTelemetry SDK backend must be constructed inside a Tokio runtime",
                        Remediation::recoverable(
                            "construct telemetry from the host Tokio runtime",
                            ["enable the otlp-sdk feature", "enter a Tokio runtime first"],
                        ),
                    )
                    .detail(
                        "backend",
                        Value::String(
                            config::ExporterBackend::OpenTelemetrySdk
                                .stable_name()
                                .to_owned(),
                        ),
                    )
                    .detail("feature", Value::String("otlp-sdk".to_owned()))
                    .detail("runtime", Value::String("caller-tokio".to_owned())),
                ),
            });
        }
        sdk::build_exporter_set(connection, bounds)
            .map(|adapter| adapter.exporters)
            .map_err(transport_construction_failure)
    }

    #[cfg(not(feature = "otlp-sdk"))]
    Err(unsupported_backend(
        config::ExporterBackend::OpenTelemetrySdk,
        "otlp-sdk",
        "the otlp-sdk feature is disabled",
    ))
}

#[allow(unused_variables)]
fn legacy_exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
    connection: &ValidatedBackendConnection,
) -> Result<ExporterSet, ConfigFailure> {
    if bounds.protocol() != config::OtlpProtocol::HttpJson {
        return Err(unsupported_protocol(
            config::ExporterBackend::LegacyHttpJson,
            bounds.protocol(),
            "HttpJson",
        ));
    }

    #[cfg(feature = "legacy-http-json")]
    {
        legacy_http_json::build_exporter_set(connection, bounds)
            .map_err(transport_construction_failure)
    }

    #[cfg(not(feature = "legacy-http-json"))]
    Err(unsupported_backend(
        config::ExporterBackend::LegacyHttpJson,
        "legacy-http-json",
        "the legacy-http-json feature is disabled",
    ))
}

#[allow(dead_code)]
fn unsupported_protocol(
    backend: config::ExporterBackend,
    protocol: config::OtlpProtocol,
    supported_protocols: &str,
) -> ConfigFailure {
    ConfigFailure::UnsupportedProtocol {
        context: Box::new(
            ErrorContext::new(
                sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL,
                "the selected exporter backend does not support the configured protocol",
                Remediation::recoverable(
                    "select a protocol supported by the selected exporter backend",
                    ["select a documented backend/protocol combination"],
                ),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned()))
            .detail("protocol", Value::String(protocol.stable_name().to_owned()))
            .detail(
                "supported_protocols",
                Value::String(supported_protocols.to_owned()),
            ),
        ),
    }
}

#[cfg(any(not(feature = "otlp-sdk"), not(feature = "legacy-http-json")))]
fn unsupported_backend(
    backend: config::ExporterBackend,
    feature: &str,
    availability: &str,
) -> ConfigFailure {
    ConfigFailure::UnsupportedBackend {
        context: Box::new(
            ErrorContext::new(
                sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND,
                "enabled exporter backend is unavailable",
                Remediation::recoverable(
                    "enable the selected backend feature or select disabled telemetry",
                    ["enable the named feature", "disable telemetry"],
                ),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned()))
            .detail("feature", Value::String(feature.to_owned()))
            .detail("availability", Value::String(availability.to_owned())),
        ),
    }
}

impl RuntimeTelemetry {
    /// Creates a telemetry runtime through the validated exporter factory.
    pub fn new(config: RuntimeTelemetryConfig) -> Result<Self, InitFailure> {
        Self::new_typed(config)
    }

    /// Creates a telemetry runtime with neutral initialization failures.
    pub fn new_typed(config: RuntimeTelemetryConfig) -> Result<Self, InitFailure> {
        let bounds = validated_telemetry_bounds(&config)?;
        Self::new_prepared(config, &bounds)
    }

    /// Construct from checked bounds; released conversion has already performed
    /// its ordered validation and canonical construction uses its strict validator.
    fn new_prepared(
        config: RuntimeTelemetryConfig,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, InitFailure> {
        let exporters = exporter_factory_prepared(&config, bounds)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        Ok(Self::new_with_validated_exporter_set(config, exporters))
    }

    #[cfg(test)]
    fn new_with_exporters(
        config: RuntimeTelemetryConfig,
        log_exporter: Arc<dyn LogExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
        metric_exporter: Arc<dyn MetricExporter>,
    ) -> Result<Self, InitFailure> {
        Self::new_with_exporters_typed(config, log_exporter, trace_exporter, metric_exporter)
    }

    #[cfg(test)]
    fn new_with_exporters_typed(
        config: RuntimeTelemetryConfig,
        log_exporter: Arc<dyn LogExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
        metric_exporter: Arc<dyn MetricExporter>,
    ) -> Result<Self, InitFailure> {
        Self::new_with_exporter_set_typed(
            config,
            ExporterSet {
                logs: log_exporter,
                traces: trace_exporter,
                metrics: metric_exporter,
                lifecycle: Arc::new(testing::RecordingLifecycle::default()),
            },
        )
    }

    #[cfg(test)]
    fn new_with_exporter_set_typed(
        config: RuntimeTelemetryConfig,
        exporters: ExporterSet,
    ) -> Result<Self, InitFailure> {
        validate_config_typed(&config)?;
        Ok(Self::new_with_validated_exporter_set(config, exporters))
    }

    fn new_with_validated_exporter_set(
        config: RuntimeTelemetryConfig,
        exporters: ExporterSet,
    ) -> Self {
        Self {
            config,
            exporters,
            runtime: Mutex::new(TelemetryRuntime::default()),
            dropped_exports_total: AtomicU64::new(0),
            bounded_assembly_drops_total: AtomicU64::new(0),
            malformed_spans_total: AtomicU64::new(0),
        }
    }

    /// Buffers one projected log event for later export.
    ///
    /// # Errors
    ///
    /// Returns [`CanonicalTelemetryError::Shutdown`] if telemetry has shut down.
    /// Returns [`CanonicalTelemetryError::Event`] when enabled telemetry receives
    /// an invalid state-transition entity identifier. Disabled logs or transport
    /// return successfully before entity validation.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_log(&self, event: &LogEvent) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.logs.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        validate_entity_id(event)?;
        self.buffer_log(event);
        Ok(())
    }

    /// Released root-facade log admission: exact 1.4.1 acceptance, no entity check.
    pub(crate) fn emit_log_released(
        &self,
        event: &LogEvent,
    ) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.logs.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        self.buffer_log(event);
        Ok(())
    }

    fn buffer_log(&self, event: &LogEvent) {
        let record = legacy_projection::log_record(event);
        self.runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .log_buffer
            .push(record);
    }

    /// Buffers one canonical span signal for later export.
    ///
    /// Signals pass through the bounded span assembler; a completed span is
    /// exported with its kind, links, parent, trace flags, status, timing and
    /// events. An `Ended` signal without a prior `Started` signal is counted in
    /// `malformed_spans_total` and returned as a structured export failure. No
    /// malformed or incomplete span is ever forwarded to the exporter backend.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_span(&self, span: &CanonicalSpanSignal) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.traces.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        self.admit_span(span.clone())
    }

    /// Released root-facade span admission: the root signal is converted to
    /// the canonical model and enters the same bounded assembler.
    pub(crate) fn emit_span_released(
        &self,
        span: &SpanSignal,
    ) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.traces.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        let span =
            legacy_projection::span_signal(span).map_err(CanonicalTelemetryError::ExportFailure)?;
        self.admit_span(span)
    }

    fn admit_span(&self, span: CanonicalSpanSignal) -> Result<(), CanonicalTelemetryError> {
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        if let CanonicalSpanSignal::Ended(record) = &span
            && !runtime
                .span_assembler
                .has_started(&record.trace().trace_id, &record.trace().span_id)
        {
            self.malformed_spans_total.fetch_add(1, Ordering::SeqCst);
            let context = ErrorContext::new(
                error_codes::OTLP_SPAN_ASSEMBLY_FAILED,
                "received ended span without a matching started span",
                Remediation::not_recoverable(
                    "emit the started span before the matching ended span",
                ),
            )
            .detail("trace_id", record.trace().trace_id.as_str().into())
            .detail("span_id", record.trace().span_id.as_str().into());
            let summary = DiagnosticSummary::from(context.diagnostic());
            runtime.last_error = Some(summary.clone());
            runtime.trace_status.last_error = Some(summary);
            return Err(CanonicalTelemetryError::ExportFailure(
                ExportError::Transport {
                    context: Box::new(context),
                },
            ));
        }
        if let Some(complete) = runtime
            .span_assembler
            .push(span)
            .map_err(export_failure_from_canonical_event)?
        {
            let resource = legacy_projection::resource(complete.record.service());
            runtime.span_buffer.push(ExportRecord {
                resource,
                scope: contracts::InstrumentationScope::default(),
                record: complete,
            });
        }
        let loss = runtime.span_assembler.take_loss();
        self.record_span_assembly_loss(&mut runtime, loss);
        Ok(())
    }

    /// Buffers one canonical metric record for later export.
    ///
    /// The record was validated at construction, so gauges, sums and
    /// histograms, including their bucket distribution and interval, are
    /// exported unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_metric(
        &self,
        metric: &CanonicalMetricRecord,
    ) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.metrics.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        let record = ExportRecord {
            resource: legacy_projection::resource(metric.service()),
            scope: contracts::InstrumentationScope::default(),
            record: metric.clone(),
        };
        self.runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .metric_buffer
            .push(BufferedMetric::Canonical(Box::new(record)));
        Ok(())
    }

    /// Released root-facade metric admission with the 1.4.1 acceptance rules.
    ///
    /// A released scalar histogram is accepted and, as in 1.4.1, never
    /// transported: it carries no bucket distribution to export and none is
    /// fabricated. Other released records keep their released shape until
    /// flush.
    pub(crate) fn emit_metric_released(
        &self,
        metric: &MetricRecord,
    ) -> Result<(), CanonicalTelemetryError> {
        self.ensure_active()?;
        if self.config.metrics.is_none()
            || !self.config.transport.enabled
            || metric.kind == MetricKind::Histogram
        {
            return Ok(());
        }
        self.runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .metric_buffer
            .push(BufferedMetric::Released(metric.clone()));
        Ok(())
    }

    /// Flushes buffered logs, spans, and metrics through the configured exporters.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn flush(&self) -> Result<(), FlushFailure> {
        self.flush_typed()
    }

    /// Flushes telemetry with a neutral flush failure while retaining fail-open
    /// exporter semantics.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        self.exporters
            .lifecycle
            .blocking_lifecycle_preflight()
            .map_err(flush_lifecycle_failure)?;
        let _ = self.flush_outcome();
        self.exporters
            .lifecycle
            .flush_blocking()
            .map_err(flush_lifecycle_failure)
    }

    /// Flushes telemetry and awaits the shared backend lifecycle barrier.
    ///
    /// SDK callers must use this method from their entered runtime; it never
    /// blocks that runtime thread to emulate legacy HTTP behavior.
    pub async fn flush_async_typed(&self) -> Result<(), FlushFailure> {
        let _ = self.flush_outcome();
        self.exporters
            .lifecycle
            .flush_async()
            .await
            .map_err(flush_lifecycle_failure)
    }

    fn flush_outcome(&self) -> FlushOutcome {
        let (log_batch, span_batch, metric_batch) = {
            let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
            let log_batch = if self.config.logs.is_some() {
                std::mem::take(&mut runtime.log_buffer)
            } else {
                Vec::new()
            };
            let span_batch = if self.config.traces.is_some() {
                std::mem::take(&mut runtime.span_buffer)
            } else {
                Vec::new()
            };
            let metric_batch = if self.config.metrics.is_some() {
                std::mem::take(&mut runtime.metric_buffer)
            } else {
                Vec::new()
            };
            (log_batch, span_batch, metric_batch)
        };
        let mut export_failure = None;

        if !log_batch.is_empty() {
            match self.exporters.logs.export_logs(&log_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Logs),
                Err(err) => {
                    self.record_export_failure(ExporterKind::Logs, log_batch.len() as u64, &err);
                    export_failure = Some(err);
                }
            }
        }

        if !span_batch.is_empty() {
            match self.exporters.traces.export_spans(&span_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Traces),
                Err(err) => {
                    self.record_export_failure(ExporterKind::Traces, span_batch.len() as u64, &err);
                    export_failure = Some(err);
                }
            }
        }

        if !metric_batch.is_empty() {
            let batch_len = metric_batch.len() as u64;
            let exported = metric_batch
                .into_iter()
                .map(|metric| match metric {
                    BufferedMetric::Canonical(record) => Ok(*record),
                    BufferedMetric::Released(metric) => legacy_projection::metric_record(&metric),
                })
                .collect::<Result<Vec<_>, _>>()
                .and_then(|batch| self.exporters.metrics.export_metrics(&batch));
            match exported {
                Ok(()) => self.record_export_success(ExporterKind::Metrics),
                Err(err) => {
                    self.record_export_failure(ExporterKind::Metrics, batch_len, &err);
                    export_failure = Some(err);
                }
            }
        }

        FlushOutcome { export_failure }
    }

    /// Flushes buffers, drops incomplete spans, and transitions the runtime to shutdown.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned while
    /// flushing, dropping incomplete spans, or constructing the final shutdown
    /// error state.
    pub fn shutdown(&self) -> Result<(), ShutdownFailure> {
        self.shutdown_typed()
    }

    /// Flushes buffers and transitions telemetry to shutdown with a neutral
    /// shutdown failure.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned while
    /// flushing, dropping incomplete spans, or constructing final state.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        if self.exporters.lifecycle.is_shutdown() {
            return Ok(());
        }

        if let Err(error) = self.exporters.lifecycle.blocking_lifecycle_preflight() {
            let last_error = self
                .runtime
                .lock()
                .expect("telemetry runtime poisoned")
                .last_error
                .clone();
            return Err(shutdown_export_failure_typed(error, last_error));
        }

        let flush_outcome = self.flush_outcome();
        let lifecycle_result = self.exporters.lifecycle.shutdown_blocking();
        self.finish_shutdown(flush_outcome, lifecycle_result)
    }

    /// Shuts telemetry down and awaits the shared backend lifecycle barrier.
    ///
    /// SDK callers use this method to await admitted RPC completion. Legacy
    /// callers keep using [`RuntimeTelemetry::shutdown_typed`], whose backend owns a
    /// bounded blocking worker shutdown.
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownFailure> {
        if self.exporters.lifecycle.is_shutdown() {
            return Ok(());
        }

        let flush_outcome = self.flush_outcome();
        let lifecycle_result = self.exporters.lifecycle.shutdown_async().await;
        self.finish_shutdown(flush_outcome, lifecycle_result)
    }

    fn finish_shutdown(
        &self,
        flush_outcome: FlushOutcome,
        lifecycle_result: Result<(), ExportError>,
    ) -> Result<(), ShutdownFailure> {
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let dropped = runtime.span_assembler.flush_incomplete() as u64;
        if dropped > 0 {
            self.dropped_exports_total
                .fetch_add(dropped, Ordering::SeqCst);
            self.bounded_assembly_drops_total
                .fetch_add(dropped, Ordering::SeqCst);
            let context = ErrorContext::new(
                error_codes::OTLP_INCOMPLETE_SPAN_DROPPED,
                "dropped incomplete spans during shutdown",
                Remediation::recoverable(
                    "ensure all started spans receive matching ended signals before shutdown",
                    ["flush the routing runtime before shutting telemetry down"],
                ),
            )
            .detail("dropped_spans", Value::from(dropped));
            let summary = DiagnosticSummary::from(context.diagnostic());
            runtime.trace_status.state = ExporterHealthState::Degraded;
            runtime.trace_status.last_error = Some(summary.clone());
            runtime.last_error = Some(summary);
        }

        if let Err(export_failure) = lifecycle_result {
            return Err(shutdown_export_failure_typed(
                export_failure,
                runtime.last_error.clone(),
            ));
        }

        if let Some(export_failure) = flush_outcome.export_failure {
            return Err(shutdown_export_failure_typed(
                export_failure,
                runtime.last_error.clone(),
            ));
        }

        Ok(())
    }

    /// Returns the current telemetry health view.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn health(&self) -> TelemetryHealthReport {
        let lifecycle_health = self.exporters.lifecycle.lifecycle_health();
        let runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let log_status = merged_lifecycle_status(
            &runtime.log_status,
            lifecycle_health.as_ref(),
            SignalKind::Logs,
        );
        let trace_status = merged_lifecycle_status(
            &runtime.trace_status,
            lifecycle_health.as_ref(),
            SignalKind::Traces,
        );
        let metric_status = merged_lifecycle_status(
            &runtime.metric_status,
            lifecycle_health.as_ref(),
            SignalKind::Metrics,
        );
        let exporter_statuses = vec![
            ExporterHealth {
                name: LOGS_EXPORTER_NAME.clone(),
                state: log_status.state,
                last_error: log_status.last_error,
            },
            ExporterHealth {
                name: TRACES_EXPORTER_NAME.clone(),
                state: trace_status.state,
                last_error: trace_status.last_error,
            },
            ExporterHealth {
                name: METRICS_EXPORTER_NAME.clone(),
                state: metric_status.state,
                last_error: metric_status.last_error,
            },
        ];

        let lifecycle_is_terminal = lifecycle_health
            .as_ref()
            .is_some_and(|health| health.phase != LifecycleState::Open);
        let state = if lifecycle_is_terminal || self.exporters.lifecycle.is_shutdown() {
            TelemetryHealthState::Unavailable
        } else if !self.config.transport.enabled {
            TelemetryHealthState::Disabled
        } else if lifecycle_health
            .as_ref()
            .is_some_and(|health| health.degraded)
            || exporter_statuses
                .iter()
                .any(|status| status.state != ExporterHealthState::Healthy)
        {
            TelemetryHealthState::Degraded
        } else {
            TelemetryHealthState::Healthy
        };

        TelemetryHealthReport {
            state,
            dropped_exports_total: lifecycle_health.as_ref().map_or_else(
                || self.dropped_exports_total.load(Ordering::SeqCst),
                |health| {
                    health.dropped_total()
                        + self.bounded_assembly_drops_total.load(Ordering::SeqCst)
                },
            ),
            malformed_spans_total: self.malformed_spans_total.load(Ordering::SeqCst),
            exporter_statuses,
            last_error: runtime
                .last_error
                .clone()
                .or_else(|| lifecycle_health.and_then(|health| health.last_error)),
        }
    }

    fn ensure_active(&self) -> Result<(), CanonicalTelemetryError> {
        if self.exporters.lifecycle.is_shutdown() {
            return Err(CanonicalTelemetryError::Shutdown {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_TELEMETRY_SHUTDOWN,
                    "telemetry runtime is shut down",
                    Remediation::not_recoverable("do not emit telemetry after shutdown"),
                )),
            });
        }
        Ok(())
    }

    fn record_export_success(&self, exporter_kind: ExporterKind) {
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let status = exporter_kind.status_mut(&mut runtime);
        status.state = ExporterHealthState::Healthy;
        status.last_error = None;
    }

    fn record_export_failure(
        &self,
        exporter_kind: ExporterKind,
        dropped: u64,
        error: &ExportError,
    ) {
        self.dropped_exports_total
            .fetch_add(dropped, Ordering::SeqCst);
        let summary = DiagnosticSummary::from(error.diagnostic());
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        runtime.last_error = Some(summary.clone());

        let status = exporter_kind.status_mut(&mut runtime);
        status.state = ExporterHealthState::Degraded;
        status.last_error = Some(summary);
    }

    /// Records deterministic bounded-assembly eviction in the health surface.
    fn record_span_assembly_loss(&self, runtime: &mut TelemetryRuntime, loss: SpanAssemblyLoss) {
        if loss.total() == 0 {
            return;
        }
        self.dropped_exports_total
            .fetch_add(loss.total(), Ordering::SeqCst);
        self.bounded_assembly_drops_total
            .fetch_add(loss.total(), Ordering::SeqCst);
        let context = ErrorContext::new(
            error_codes::OTLP_INCOMPLETE_SPAN_DROPPED,
            "evicted bounded live span assembly state",
            Remediation::recoverable(
                "complete spans before the configured live-span or event capacity is exhausted",
                [
                    "reduce concurrent spans",
                    "flush completed spans more frequently",
                ],
            ),
        )
        .detail("evicted_spans", Value::from(loss.evicted_spans))
        .detail("evicted_events", Value::from(loss.evicted_events));
        let summary = DiagnosticSummary::from(context.diagnostic());
        runtime.trace_status.state = ExporterHealthState::Degraded;
        runtime.trace_status.last_error = Some(summary.clone());
        runtime.last_error = Some(summary);
    }
}

impl telemetry_health_provider_sealed::Sealed for RuntimeTelemetry {
    fn token(&self) -> telemetry_health_provider_sealed::Token {
        telemetry_health_provider_sealed::workspace_token()
    }
}
impl ObservabilityHealthProvider for RuntimeTelemetry {
    fn telemetry_health(&self) -> TelemetryHealthReport {
        self.health()
    }
}

mod sealed_emitters {
    pub trait Sealed {}
}

#[expect(
    dead_code,
    reason = "crate-local span emitter trait is intentionally retained for direct telemetry injection"
)]
pub(crate) trait SpanEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_span(&self, span: CanonicalSpanSignal) -> Result<(), CanonicalTelemetryError>;
}

#[expect(
    dead_code,
    reason = "crate-local metric emitter trait is intentionally retained for direct telemetry injection"
)]
pub(crate) trait MetricEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_metric(&self, metric: CanonicalMetricRecord) -> Result<(), CanonicalTelemetryError>;
}

impl sealed_emitters::Sealed for RuntimeTelemetry {}

impl SpanEmitter for RuntimeTelemetry {
    fn emit_span(&self, span: CanonicalSpanSignal) -> Result<(), CanonicalTelemetryError> {
        RuntimeTelemetry::emit_span(self, &span)
    }
}

impl MetricEmitter for RuntimeTelemetry {
    fn emit_metric(&self, metric: CanonicalMetricRecord) -> Result<(), CanonicalTelemetryError> {
        RuntimeTelemetry::emit_metric(self, &metric)
    }
}

/// Canonical admission check for the state-transition `entity_id`.
fn validate_entity_id(event: &LogEvent) -> Result<(), CanonicalTelemetryError> {
    let Some(entity_id) = event
        .state_transition
        .as_ref()
        .and_then(|transition| transition.entity_id.as_deref())
    else {
        return Ok(());
    };
    sc_observability_types::EntityId::new(entity_id)
        .map(|_| ())
        .map_err(|error| {
            let failure = EventFailure::invalid_event(
                "log event state transition entity_id is invalid",
                Remediation::recoverable(
                    "emit a valid entity_id or omit it",
                    ["rebuild the state transition before emitting"],
                ),
            )
            .source(Box::new(error));
            CanonicalTelemetryError::Event(CanonicalEventError::Validation {
                context: failure.into_context(),
            })
        })
}

/// Builds a telemetry export failure with the crate-local error code.
#[expect(
    dead_code,
    reason = "crate-local export failure helper is retained for internal construction sites"
)]
pub(crate) fn export_failure(message: impl Into<String>) -> CanonicalTelemetryError {
    CanonicalTelemetryError::ExportFailure(ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
        )),
    })
}

/// Converts a canonical span-assembly failure into a telemetry export failure,
/// moving the original error context unchanged.
fn export_failure_from_canonical_event(err: CanonicalEventError) -> CanonicalTelemetryError {
    CanonicalTelemetryError::ExportFailure(ExportError::Transport {
        context: err.into_context(),
    })
}

/// Preserves a shared-lifecycle failure as the source of a facade flush error.
fn flush_lifecycle_failure(error: ExportError) -> FlushFailure {
    FlushFailure::telemetry_flush(
        "the shared telemetry lifecycle did not complete its flush barrier",
        Remediation::recoverable(
            "inspect the exporter lifecycle and retry after it recovers",
            ["retry flush"],
        ),
    )
    .source(Box::new(error))
}

/// Converts a flush failure into a shutdown failure, chaining the flush
/// failure as the shutdown context's native source.
///
/// `flush_outcome` never currently returns `Err` (it is intentionally
/// `Result`-shaped so shutdown can propagate real flush failures without a
/// public-signature change later; see its own `unnecessary_wraps` rationale),
/// so this conversion is unreachable at runtime today. It is kept, rather
/// than deleted, for that future propagation path, and is covered directly
/// by `shutdown_flush_failure_preserves_flush_context_as_native_source`
/// below so a regression in its error-context/source chaining is still
/// caught even while the call site is dormant.
#[cfg_attr(not(test), allow(dead_code))]
fn shutdown_flush_failure(error: FlushFailure) -> ShutdownFailure {
    ShutdownFailure::from_context(Box::new(
        ErrorContext::new(
            error_codes::OTLP_FLUSH_FAILED,
            "failed to flush telemetry during shutdown",
            Remediation::recoverable(
                "inspect telemetry health and retry shutdown after the exporter recovers",
                ["retry shutdown"],
            ),
        )
        .source(Box::new(error)),
    ))
}

fn shutdown_export_failure_typed(
    error: ExportError,
    diagnostic_summary: Option<DiagnosticSummary>,
) -> ShutdownFailure {
    // The legacy shutdown path selected `runtime.last_error` after incomplete
    // span accounting. Preserve that diagnostic selection exactly, while the
    // typed source chain keeps the actual exporter failure available to callers.
    let summary = diagnostic_summary.unwrap_or_else(|| DiagnosticSummary::from(error.diagnostic()));
    let mut context = ErrorContext::new(
        error_codes::OTLP_FLUSH_FAILED,
        "failed to flush telemetry during shutdown",
        Remediation::recoverable(
            "inspect telemetry health and retry shutdown after the exporter recovers",
            ["retry shutdown"],
        ),
    );
    context = context.cause(summary.message);
    if let Some(code) = summary.code {
        context = context.detail(
            "exporter_error_code",
            Value::String(code.as_str().to_owned()),
        );
    }
    ShutdownFailure::from_context(Box::new(context.source(Box::new(error))))
}
