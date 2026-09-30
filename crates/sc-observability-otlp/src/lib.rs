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

mod constants;
mod error_codes;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use config::{BackendTransportBounds, ValidatedTransportBounds, validated_telemetry_bounds};
#[cfg(test)]
use config::{validate_config_typed, validated_transport_bounds};
use sc_observability_types::typed::{EventFailure, FlushFailure, InitFailure, ShutdownFailure};
#[doc(inline)]
pub use sc_observability_types::v2::TelemetryError;
use sc_observability_types::v2::{ConfigFailure, ExportError};
use sc_observability_types::{
    DiagnosticSummary, ErrorContext, LogEvent, MetricRecord, ObservabilityHealthProvider,
    Remediation, SinkName, SpanSignal, telemetry_health_provider_sealed,
};
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryHealthReport, TelemetryHealthState,
};
use serde_json::Value;

#[doc(inline)]
pub use assembly::{CompleteSpan, SpanAssembler, SpanAssemblyLoss};
#[doc(inline)]
pub use config::{
    AuthHeader, ExporterBackend, LegacyRetryPolicy, LogsConfig, MetricsConfig, OtelConfig,
    OtlpEndpoint, OtlpProtocol, ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder,
    TracesConfig,
};
#[doc(inline)]
pub use projectors::TelemetryProjectors;

/// Opt-in canonical OTLP facade for the compatible 1.x transition.
///
/// This namespace re-exports the existing telemetry implementation and does
/// not introduce a second backend, configuration authority, or lifecycle.
pub mod v2 {
    #[doc(inline)]
    pub use crate::{
        AuthHeader, OtelConfig, OtlpEndpoint, Telemetry, TelemetryConfig, TelemetryConfigBuilder,
        TelemetryProjectors,
    };
    #[doc(inline)]
    pub use sc_observability_types::v2::{
        ConfigFailure, EventError, FlushError, InitError, ShutdownError,
    };
}
#[cfg(feature = "sdk-test-support")]
#[doc(inline)]
pub use sdk::SdkFixture;

use contracts::{ExporterLifecycle, LifecycleFuture, LogExporter, MetricExporter, TraceExporter};
#[cfg(test)]
use legacy_projection::trace_context;
#[allow(
    unused_imports,
    reason = "legacy projection is selected only when the optional legacy backend is enabled"
)]
use legacy_projection::{raw_exporter_set, transport_construction_failure};
use lifecycle::{LifecycleHealth, LifecycleState, SignalKind};

// Temporary root-facade specialization. D.18 can remove this alias when the
// facade composition decision is made; backend adapters use the v2 defaults.
type ExporterSet = contracts::ExporterSet<LogEvent, CompleteSpan, MetricRecord>;

/// OTLP-backed telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "telemetry owns exporter trait objects and runtime state that are intentionally not exposed through a stable Debug contract"
)]
pub struct Telemetry {
    config: TelemetryConfig,
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
    span_assembler: SpanAssembler,
    log_buffer: Vec<LogEvent>,
    span_buffer: Vec<CompleteSpan>,
    metric_buffer: Vec<MetricRecord>,
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

impl LogExporter<LogEvent> for DisabledLogExporter {
    fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl TraceExporter<CompleteSpan> for DisabledTraceExporter {
    fn export_spans(&self, _batch: &[CompleteSpan]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl MetricExporter<MetricRecord> for DisabledMetricExporter {
    fn export_metrics(&self, _batch: &[MetricRecord]) -> Result<(), ExportError> {
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
fn exporter_factory(
    config: &TelemetryConfig,
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
        BackendTransportBounds::Sdk => sdk_exporter_factory(config, bounds),
        BackendTransportBounds::Legacy(_) => legacy_exporter_factory(config, bounds),
    }
}

#[allow(unused_variables)]
fn sdk_exporter_factory(
    config: &TelemetryConfig,
    bounds: &ValidatedTransportBounds,
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
        let connection = config::validated_backend_connection(&config.transport)?;
        sdk::build_exporter_set(&connection, bounds)
            .map(|adapter| raw_exporter_set(adapter.exporters))
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
    config: &TelemetryConfig,
    bounds: &ValidatedTransportBounds,
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
        legacy_http_json::build_exporter_set(&config.transport)
            .map(raw_exporter_set)
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

impl Telemetry {
    /// Creates a telemetry runtime through the validated exporter factory.
    pub fn new(config: TelemetryConfig) -> Result<Self, InitFailure> {
        Self::new_typed(config)
    }

    /// Creates a telemetry runtime with neutral initialization failures.
    pub fn new_typed(config: TelemetryConfig) -> Result<Self, InitFailure> {
        let bounds = validated_telemetry_bounds(&config)?;
        let exporters = exporter_factory(&config, &bounds)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
        Ok(Self::new_with_validated_exporter_set(config, exporters))
    }

    #[cfg(test)]
    fn new_with_exporters(
        config: TelemetryConfig,
        log_exporter: Arc<dyn LogExporter<LogEvent>>,
        trace_exporter: Arc<dyn TraceExporter<CompleteSpan>>,
        metric_exporter: Arc<dyn MetricExporter<MetricRecord>>,
    ) -> Result<Self, InitFailure> {
        Self::new_with_exporters_typed(config, log_exporter, trace_exporter, metric_exporter)
    }

    #[cfg(test)]
    fn new_with_exporters_typed(
        config: TelemetryConfig,
        log_exporter: Arc<dyn LogExporter<LogEvent>>,
        trace_exporter: Arc<dyn TraceExporter<CompleteSpan>>,
        metric_exporter: Arc<dyn MetricExporter<MetricRecord>>,
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
        config: TelemetryConfig,
        exporters: ExporterSet,
    ) -> Result<Self, InitFailure> {
        validate_config_typed(&config)?;
        Ok(Self::new_with_validated_exporter_set(config, exporters))
    }

    fn new_with_validated_exporter_set(config: TelemetryConfig, exporters: ExporterSet) -> Self {
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
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_log(&self, event: &LogEvent) -> Result<(), TelemetryError> {
        self.ensure_active()?;
        if self.config.logs.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        self.runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .log_buffer
            .push(event.clone());
        Ok(())
    }

    /// Buffers one projected span signal for later export.
    ///
    /// An `Ended` signal without a prior `Started` signal is counted in
    /// `malformed_spans_total` and returned as a structured export failure. No
    /// malformed or incomplete span is ever forwarded to the `OTel` backend.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_span(&self, span: &SpanSignal) -> Result<(), TelemetryError> {
        self.ensure_active()?;
        if self.config.traces.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        if let SpanSignal::Ended(record) = span
            && !runtime.span_assembler.has_started(
                record.trace().trace_id.as_str(),
                record.trace().span_id.as_str(),
            )
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
            return Err(TelemetryError::ExportFailure(ExportError::Transport {
                context: Box::new(context),
            }));
        }
        if let Some(complete) = runtime
            .span_assembler
            .push_typed(span.clone())
            .map_err(export_failure_from_event)?
        {
            runtime.span_buffer.push(complete);
        }
        let loss = runtime.span_assembler.take_loss();
        self.record_span_assembly_loss(&mut runtime, loss);
        Ok(())
    }

    /// Buffers one projected metric record for later export.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn emit_metric(&self, metric: &MetricRecord) -> Result<(), TelemetryError> {
        self.ensure_active()?;
        if self.config.metrics.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        self.runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .metric_buffer
            .push(metric.clone());
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
            match self.exporters.metrics.export_metrics(&metric_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Metrics),
                Err(err) => {
                    self.record_export_failure(
                        ExporterKind::Metrics,
                        metric_batch.len() as u64,
                        &err,
                    );
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
    /// callers keep using [`Telemetry::shutdown_typed`], whose backend owns a
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

    fn ensure_active(&self) -> Result<(), TelemetryError> {
        if self.exporters.lifecycle.is_shutdown() {
            return Err(TelemetryError::Shutdown {
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

impl telemetry_health_provider_sealed::Sealed for Telemetry {
    fn token(&self) -> telemetry_health_provider_sealed::Token {
        telemetry_health_provider_sealed::workspace_token()
    }
}
impl ObservabilityHealthProvider for Telemetry {
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
    fn emit_span(&self, span: SpanSignal) -> Result<(), TelemetryError>;
}

#[expect(
    dead_code,
    reason = "crate-local metric emitter trait is intentionally retained for direct telemetry injection"
)]
pub(crate) trait MetricEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_metric(&self, metric: MetricRecord) -> Result<(), TelemetryError>;
}

impl sealed_emitters::Sealed for Telemetry {}

impl SpanEmitter for Telemetry {
    fn emit_span(&self, span: SpanSignal) -> Result<(), TelemetryError> {
        Telemetry::emit_span(self, &span)
    }
}

impl MetricEmitter for Telemetry {
    fn emit_metric(&self, metric: MetricRecord) -> Result<(), TelemetryError> {
        Telemetry::emit_metric(self, &metric)
    }
}

/// Builds a telemetry export failure with the crate-local error code.
#[expect(
    dead_code,
    reason = "crate-local export failure helper is retained for internal construction sites"
)]
pub(crate) fn export_failure(message: impl Into<String>) -> TelemetryError {
    TelemetryError::ExportFailure(ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
        )),
    })
}

/// Converts a span-assembly event failure into a telemetry export failure.
///
/// Moves the original `Box<ErrorContext>` unchanged via `into_context()`
/// rather than reconstructing a new one from its diagnostic fields, so the
/// original timestamp, backtrace, and any attached source survive intact.
fn export_failure_from_event(err: EventFailure) -> TelemetryError {
    TelemetryError::ExportFailure(ExportError::Transport {
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
