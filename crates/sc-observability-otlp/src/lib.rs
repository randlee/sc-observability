//! OTLP-backed telemetry layered on top of `sc-observe`.
//!
//! This crate owns telemetry configuration, span assembly, exporter contracts,
//! and the lifecycle/runtime behavior for OTLP-bound signals. It attaches to
//! routing through ordinary projector registration and keeps OpenTelemetry
//! transport concerns out of the lower crates.

pub mod constants;
pub mod error_codes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, DurationMs, ErrorContext, EventError, ExportError,
    FlushError, InitError, LogEvent, LogProjector, MetricProjector, MetricRecord, Observable,
    Observation, ObservationFilter, ProjectionError, ProjectionRegistration, Remediation,
    ServiceName, ShutdownError, SpanEnded, SpanEvent, SpanProjector, SpanRecord, SpanSignal,
    SpanStarted, TelemetryHealthProvider, Timestamp, telemetry_health_provider_sealed,
};
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryError, TelemetryHealthReport,
    TelemetryHealthState,
};
use serde_json::{Map, Value};

/// Supported OTLP transport protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    HttpBinary,
    HttpJson,
    Grpc,
}

/// Transport-level OTLP configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub protocol: OtlpProtocol,
    pub auth_header: Option<String>,
    pub ca_file: Option<PathBuf>,
    pub insecure_skip_verify: bool,
    pub timeout_ms: DurationMs,
    pub debug_local_export: bool,
    pub max_retries: u32,
    pub initial_backoff_ms: DurationMs,
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
    pub attributes: Map<String, Value>,
}

/// Log export batching configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogsConfig {
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
    pub batch_size: usize,
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
    pub service_name: ServiceName,
    pub resource: ResourceAttributes,
    pub transport: OtelConfig,
    pub logs: Option<LogsConfig>,
    pub traces: Option<TracesConfig>,
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
    pub fn build(self) -> TelemetryConfig {
        TelemetryConfig {
            service_name: self.service_name,
            resource: self.resource,
            transport: self.transport,
            logs: self.logs,
            traces: self.traces,
            metrics: self.metrics,
        }
    }
}

/// Completed span assembled from a start/event/end stream.
#[derive(Debug, Clone, PartialEq)]
pub struct CompleteSpan {
    pub record: SpanRecord<SpanEnded>,
    pub events: Vec<SpanEvent>,
}

/// Public helper for attaching telemetry export to ordinary observation projection registration.
pub struct TelemetryProjectors<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    log_projector: Option<Arc<dyn LogProjector<T>>>,
    span_projector: Option<Arc<dyn SpanProjector<T>>>,
    metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

impl<T> TelemetryProjectors<T>
where
    T: Observable,
{
    /// Starts a wrapped projector set for one observation payload type.
    pub fn new(telemetry: Arc<Telemetry>) -> Self {
        Self {
            telemetry,
            log_projector: None,
            span_projector: None,
            metric_projector: None,
            filter: None,
        }
    }

    /// Attaches a log projector whose output is also forwarded into telemetry.
    pub fn with_log_projector(mut self, projector: Arc<dyn LogProjector<T>>) -> Self {
        self.log_projector = Some(projector);
        self
    }

    /// Attaches a span projector whose output is also forwarded into telemetry.
    pub fn with_span_projector(mut self, projector: Arc<dyn SpanProjector<T>>) -> Self {
        self.span_projector = Some(projector);
        self
    }

    /// Attaches a metric projector whose output is also forwarded into telemetry.
    pub fn with_metric_projector(mut self, projector: Arc<dyn MetricProjector<T>>) -> Self {
        self.metric_projector = Some(projector);
        self
    }

    /// Attaches the same observation filter the wrapped projector registration should honor.
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Converts the wrapped helper into ordinary sc-observe projection registration.
    pub fn into_registration(self) -> ProjectionRegistration<T> {
        ProjectionRegistration {
            log_projector: self.log_projector.map(|inner| {
                Arc::new(AttachedLogProjector {
                    telemetry: self.telemetry.clone(),
                    inner,
                }) as Arc<dyn LogProjector<T>>
            }),
            span_projector: self.span_projector.map(|inner| {
                Arc::new(AttachedSpanProjector {
                    telemetry: self.telemetry.clone(),
                    inner,
                }) as Arc<dyn SpanProjector<T>>
            }),
            metric_projector: self.metric_projector.map(|inner| {
                Arc::new(AttachedMetricProjector {
                    telemetry: self.telemetry,
                    inner,
                }) as Arc<dyn MetricProjector<T>>
            }),
            filter: self.filter,
        }
    }
}

struct AttachedLogProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn LogProjector<T>>,
}

impl<T> LogProjector<T> for AttachedLogProjector<T>
where
    T: Observable,
{
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
        let events = self.inner.project_logs(observation)?;
        for event in &events {
            self.telemetry
                .emit_log(event)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(events)
    }
}

struct AttachedSpanProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn SpanProjector<T>>,
}

impl<T> SpanProjector<T> for AttachedSpanProjector<T>
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        let spans = self.inner.project_spans(observation)?;
        for span in &spans {
            self.telemetry
                .emit_span(span)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(spans)
    }
}

struct AttachedMetricProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn MetricProjector<T>>,
}

impl<T> MetricProjector<T> for AttachedMetricProjector<T>
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        let metrics = self.inner.project_metrics(observation)?;
        for metric in &metrics {
            self.telemetry
                .emit_metric(metric)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(metrics)
    }
}

/// Exporter contract for projected log records.
pub trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

/// Exporter contract for completed spans.
pub trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

/// Exporter contract for projected metrics.
pub trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}

/// Stateful span assembler used by telemetry export.
pub struct SpanAssembler {
    started: HashMap<String, SpanRecord<SpanStarted>>,
    events: HashMap<String, Vec<SpanEvent>>,
}

impl SpanAssembler {
    /// Creates an empty assembler.
    pub fn new() -> Self {
        Self {
            started: HashMap::new(),
            events: HashMap::new(),
        }
    }

    /// Pushes one lifecycle signal through the assembler.
    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError> {
        match signal {
            SpanSignal::Started(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                self.started.insert(key, record);
                Ok(None)
            }
            SpanSignal::Event(event) => {
                let key = span_key(event.trace.trace_id.as_str(), event.trace.span_id.as_str());
                if !self.started.contains_key(&key) {
                    return Err(EventError(Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                        "received span event without a matching started span",
                        Remediation::not_recoverable(
                            "emit started, event, and ended span signals in order",
                        ),
                    ))));
                }
                self.events.entry(key).or_default().push(event);
                Ok(None)
            }
            SpanSignal::Ended(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                if self.started.remove(&key).is_none() {
                    return Err(EventError(Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                        "received ended span without a matching started span",
                        Remediation::not_recoverable(
                            "emit started and ended span signals with the same trace context",
                        ),
                    ))));
                }
                Ok(Some(CompleteSpan {
                    record,
                    events: self.events.remove(&key).unwrap_or_default(),
                }))
            }
        }
    }

    /// Drops any incomplete span state and returns the number of dropped spans.
    pub fn flush_incomplete(&mut self) -> usize {
        let dropped = self.started.len();
        self.started.clear();
        self.events.clear();
        dropped
    }
}

impl Default for SpanAssembler {
    fn default() -> Self {
        Self::new()
    }
}

/// OTLP-backed telemetry runtime.
pub struct Telemetry {
    config: TelemetryConfig,
    shutdown: AtomicBool,
    log_exporter: Arc<dyn LogExporter>,
    trace_exporter: Arc<dyn TraceExporter>,
    metric_exporter: Arc<dyn MetricExporter>,
    // MUTEX: exporter flush/shutdown paths mutate buffers and per-signal runtime health together;
    // Mutex keeps the buffered state and last_error snapshot consistent, and RwLock would not help
    // because these operations are write-heavy critical sections.
    runtime: Mutex<TelemetryRuntime>,
    dropped_exports_total: AtomicU64,
    malformed_spans_total: AtomicU64,
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

impl Default for ExporterRuntime {
    fn default() -> Self {
        Self {
            state: ExporterHealthState::Healthy,
            last_error: None,
        }
    }
}

struct NoopLogExporter;
struct NoopTraceExporter;
struct NoopMetricExporter;

impl LogExporter for NoopLogExporter {
    fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl TraceExporter for NoopTraceExporter {
    fn export_spans(&self, _batch: &[CompleteSpan]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl MetricExporter for NoopMetricExporter {
    fn export_metrics(&self, _batch: &[MetricRecord]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl Telemetry {
    /// Creates a telemetry runtime with the default no-op exporters.
    pub fn new(config: TelemetryConfig) -> Result<Self, InitError> {
        Self::new_with_exporters(
            config,
            Arc::new(NoopLogExporter),
            Arc::new(NoopTraceExporter),
            Arc::new(NoopMetricExporter),
        )
    }

    fn new_with_exporters(
        config: TelemetryConfig,
        log_exporter: Arc<dyn LogExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
        metric_exporter: Arc<dyn MetricExporter>,
    ) -> Result<Self, InitError> {
        validate_config(&config)?;
        Ok(Self {
            config,
            shutdown: AtomicBool::new(false),
            log_exporter,
            trace_exporter,
            metric_exporter,
            runtime: Mutex::new(TelemetryRuntime::default()),
            dropped_exports_total: AtomicU64::new(0),
            malformed_spans_total: AtomicU64::new(0),
        })
    }

    /// Buffers one projected log event for later export.
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
    /// An `Ended` signal without a prior `Started` signal is absorbed and
    /// counted in `malformed_spans_total`. No malformed or incomplete span is
    /// ever forwarded to the OTel backend.
    pub fn emit_span(&self, span: &SpanSignal) -> Result<(), TelemetryError> {
        self.ensure_active()?;
        if self.config.traces.is_none() || !self.config.transport.enabled {
            return Ok(());
        }
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        if let SpanSignal::Ended(record) = span {
            let key = span_key(
                record.trace().trace_id.as_str(),
                record.trace().span_id.as_str(),
            );
            if !runtime.span_assembler.started.contains_key(&key) {
                self.malformed_spans_total.fetch_add(1, Ordering::SeqCst);
                let summary = DiagnosticSummary {
                    code: Some(error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED),
                    message: "received ended span without a matching started span".to_string(),
                    at: span_timestamp(span),
                };
                runtime.last_error = Some(summary.clone());
                runtime.trace_status.last_error = Some(summary);
                return Ok(());
            }
        }
        if let Some(complete) = runtime.span_assembler.push(span.clone()).map_err(|err| {
            TelemetryError::ExportFailure(Box::new(error_context_from_diagnostic(err.diagnostic())))
        })? {
            runtime.span_buffer.push(complete);
        }
        Ok(())
    }

    /// Buffers one projected metric record for later export.
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
    pub fn flush(&self) -> Result<(), FlushError> {
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

        if !log_batch.is_empty() {
            match self.log_exporter.export_logs(&log_batch) {
                Ok(()) => self.record_export_success("logs"),
                Err(err) => self.record_export_failure("logs", log_batch.len() as u64, err),
            }
        }

        if !span_batch.is_empty() {
            match self.trace_exporter.export_spans(&span_batch) {
                Ok(()) => self.record_export_success("traces"),
                Err(err) => self.record_export_failure("traces", span_batch.len() as u64, err),
            }
        }

        if !metric_batch.is_empty() {
            match self.metric_exporter.export_metrics(&metric_batch) {
                Ok(()) => self.record_export_success("metrics"),
                Err(err) => self.record_export_failure("metrics", metric_batch.len() as u64, err),
            }
        }

        Ok(())
    }

    /// Flushes buffers, drops incomplete spans, and transitions the runtime to shutdown.
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        // Best-effort flush on shutdown: exporter failures are recorded in health,
        // but shutdown itself stays fail-open so callers can continue teardown.
        let _ = self.flush();
        let dropped = self
            .runtime
            .lock()
            .expect("telemetry runtime poisoned")
            .span_assembler
            .flush_incomplete() as u64;
        if dropped > 0 {
            self.dropped_exports_total
                .fetch_add(dropped, Ordering::SeqCst);
            let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
            let context = ErrorContext::new(
                error_codes::TELEMETRY_INCOMPLETE_SPAN_DROPPED,
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

        Ok(())
    }

    /// Returns the current telemetry health view.
    pub fn health(&self) -> TelemetryHealthReport {
        let runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let exporter_statuses = vec![
            ExporterHealth {
                name: "logs".to_string(),
                state: runtime.log_status.state,
                last_error: runtime.log_status.last_error.clone(),
            },
            ExporterHealth {
                name: "traces".to_string(),
                state: runtime.trace_status.state,
                last_error: runtime.trace_status.last_error.clone(),
            },
            ExporterHealth {
                name: "metrics".to_string(),
                state: runtime.metric_status.state,
                last_error: runtime.metric_status.last_error.clone(),
            },
        ];

        let state = if self.shutdown.load(Ordering::SeqCst) {
            TelemetryHealthState::Unavailable
        } else if !self.config.transport.enabled {
            TelemetryHealthState::Disabled
        } else if exporter_statuses
            .iter()
            .any(|status| status.state != ExporterHealthState::Healthy)
        {
            TelemetryHealthState::Degraded
        } else {
            TelemetryHealthState::Healthy
        };

        TelemetryHealthReport {
            state,
            dropped_exports_total: self.dropped_exports_total.load(Ordering::SeqCst),
            malformed_spans_total: self.malformed_spans_total.load(Ordering::SeqCst),
            exporter_statuses,
            last_error: runtime.last_error.clone(),
        }
    }

    fn ensure_active(&self) -> Result<(), TelemetryError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(TelemetryError::Shutdown);
        }
        Ok(())
    }

    fn record_export_success(&self, exporter_name: &str) {
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let status = match exporter_name {
            "logs" => &mut runtime.log_status,
            "traces" => &mut runtime.trace_status,
            "metrics" => &mut runtime.metric_status,
            _ => unreachable!("unknown exporter name"),
        };
        status.state = ExporterHealthState::Healthy;
        status.last_error = None;
    }

    fn record_export_failure(&self, exporter_name: &str, dropped: u64, error: ExportError) {
        self.dropped_exports_total
            .fetch_add(dropped, Ordering::SeqCst);
        let summary = DiagnosticSummary::from(error.diagnostic());
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        runtime.last_error = Some(summary.clone());

        let status = match exporter_name {
            "logs" => &mut runtime.log_status,
            "traces" => &mut runtime.trace_status,
            "metrics" => &mut runtime.metric_status,
            _ => unreachable!("unknown exporter name"),
        };
        status.state = ExporterHealthState::Degraded;
        status.last_error = Some(summary);
    }
}

impl telemetry_health_provider_sealed::Sealed for Telemetry {}

impl TelemetryHealthProvider for Telemetry {
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
    TelemetryError::ExportFailure(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_EXPORT_FAILED,
        message,
        Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
    )))
}

fn telemetry_to_projection_error(error: sc_observability_types::TelemetryError) -> ProjectionError {
    match error {
        sc_observability_types::TelemetryError::Shutdown => {
            ProjectionError(Box::new(ErrorContext::new(
                error_codes::TELEMETRY_EXPORT_FAILED,
                "telemetry runtime is shut down",
                Remediation::not_recoverable("do not project telemetry after shutdown"),
            )))
        }
        sc_observability_types::TelemetryError::ExportFailure(context) => ProjectionError(context),
    }
}

fn span_timestamp(span: &SpanSignal) -> Timestamp {
    match span {
        SpanSignal::Started(record) => record.timestamp(),
        SpanSignal::Event(event) => event.timestamp,
        SpanSignal::Ended(record) => record.timestamp(),
    }
}

fn validate_config(config: &TelemetryConfig) -> Result<(), InitError> {
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

fn error_context_from_diagnostic(diagnostic: &sc_observability_types::Diagnostic) -> ErrorContext {
    let mut context = ErrorContext::new(
        diagnostic.code.clone(),
        diagnostic.message.clone(),
        diagnostic.remediation.clone(),
    );
    if let Some(cause) = &diagnostic.cause {
        context = context.cause(cause.clone());
    }
    if let Some(docs) = &diagnostic.docs {
        context = context.docs(docs.clone());
    }
    for (key, value) in &diagnostic.details {
        context = context.detail(key.clone(), value.clone());
    }
    context
}

fn span_key(trace_id: &str, span_id: &str) -> String {
    format!("{trace_id}:{span_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::{
        ActionName, Diagnostic, ErrorCode, Level, LogEvent, MetricKind, MetricName,
        ProcessIdentity, ServiceName, SpanId, StateTransition, TargetCategory, Timestamp,
        TraceContext, TraceId,
    };
    use serde_json::json;

    #[derive(Default)]
    struct RecordingLogExporter {
        calls: Mutex<Vec<usize>>,
        fail: AtomicBool,
    }

    impl LogExporter for RecordingLogExporter {
        fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError> {
            self.calls.lock().expect("calls poisoned").push(batch.len());
            if self.fail.load(Ordering::SeqCst) {
                Err(ExportError(Box::new(ErrorContext::new(
                    error_codes::TELEMETRY_EXPORT_FAILED,
                    "log export failed",
                    Remediation::not_recoverable("test exporter failure"),
                ))))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct RecordingTraceExporter {
        calls: Mutex<Vec<usize>>,
        fail: AtomicBool,
    }

    impl TraceExporter for RecordingTraceExporter {
        fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError> {
            self.calls.lock().expect("calls poisoned").push(batch.len());
            if self.fail.load(Ordering::SeqCst) {
                Err(ExportError(Box::new(ErrorContext::new(
                    error_codes::TELEMETRY_EXPORT_FAILED,
                    "trace export failed",
                    Remediation::not_recoverable("test exporter failure"),
                ))))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct RecordingMetricExporter {
        calls: Mutex<Vec<usize>>,
        fail: AtomicBool,
    }

    impl MetricExporter for RecordingMetricExporter {
        fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError> {
            self.calls.lock().expect("calls poisoned").push(batch.len());
            if self.fail.load(Ordering::SeqCst) {
                Err(ExportError(Box::new(ErrorContext::new(
                    error_codes::TELEMETRY_EXPORT_FAILED,
                    "metric export failed",
                    Remediation::not_recoverable("test exporter failure"),
                ))))
            } else {
                Ok(())
            }
        }
    }

    fn service_name() -> ServiceName {
        ServiceName::new("test-service").expect("valid service")
    }

    fn telemetry_config() -> TelemetryConfig {
        TelemetryConfigBuilder::new(service_name())
            .enable_logs(LogsConfig::default())
            .enable_traces(TracesConfig::default())
            .enable_metrics(MetricsConfig::default())
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: Some("https://otel.example.internal".to_string()),
                ..OtelConfig::default()
            })
            .build()
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: None,
        }
    }

    fn log_event(service: ServiceName, message: &str) -> LogEvent {
        LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service,
            target: TargetCategory::new("test.agent").expect("valid target"),
            action: ActionName::new("agent.observe").expect("valid action"),
            message: Some(message.to_string()),
            identity: ProcessIdentity::default(),
            trace: Some(trace_context()),
            request_id: None,
            correlation_id: None,
            outcome: Some("ok".to_string()),
            diagnostic: Some(Diagnostic {
                timestamp: Timestamp::UNIX_EPOCH,
                code: ErrorCode::new_static("SC_TEST"),
                message: "projected".to_string(),
                cause: None,
                remediation: Remediation::recoverable("retry", ["inspect telemetry"]),
                docs: None,
                details: Map::new(),
            }),
            state_transition: Some(StateTransition {
                entity_kind: "agent".to_string(),
                entity_id: Some("agent-123".to_string()),
                from_state: "idle".to_string(),
                to_state: "running".to_string(),
                reason: None,
                trigger: None,
            }),
            fields: Map::from_iter([("kind".to_string(), json!(message))]),
        }
    }

    #[test]
    fn telemetry_config_builder_defaults() {
        // TelemetryConfig is constructed independently of ObservabilityConfig (OTLP-018).
        let config = TelemetryConfigBuilder::new(service_name()).build();

        assert!(config.logs.is_none());
        assert!(config.traces.is_none());
        assert!(config.metrics.is_none());
        assert!(!config.transport.enabled);
        assert_eq!(config.transport.protocol, OtlpProtocol::HttpBinary);
    }

    #[test]
    fn invalid_config_is_rejected_eagerly() {
        let config = TelemetryConfigBuilder::new(service_name())
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build();

        assert!(Telemetry::new(config).is_err());
    }

    #[test]
    fn all_signals_disabled_rejects_at_construction() {
        let config = TelemetryConfigBuilder::new(service_name())
            .with_transport(OtelConfig {
                enabled: false,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build();

        assert!(Telemetry::new(config).is_ok());
    }

    #[test]
    fn span_assembler_emits_complete_span_from_lifecycle() {
        let trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace.clone(),
            Map::new(),
        );
        let ended = started
            .clone()
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42));
        let mut assembler = SpanAssembler::new();

        assert!(
            assembler
                .push(SpanSignal::Started(started))
                .expect("started")
                .is_none()
        );
        assert!(
            assembler
                .push(SpanSignal::Event(SpanEvent {
                    timestamp: Timestamp::UNIX_EPOCH,
                    trace: trace.clone(),
                    name: ActionName::new("tool.call").expect("valid name"),
                    attributes: Map::new(),
                    diagnostic: None,
                }))
                .expect("event")
                .is_none()
        );
        let complete = assembler
            .push(SpanSignal::Ended(ended))
            .expect("ended")
            .expect("complete span");

        assert_eq!(complete.events.len(), 1);
        assert_eq!(complete.record.duration_ms(), DurationMs::from(42));
    }

    #[test]
    fn incomplete_span_drop_accounting_is_tracked() {
        let trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace,
            Map::new(),
        );
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");

        telemetry
            .emit_span(&SpanSignal::Started(started))
            .expect("emit started");
        telemetry.shutdown().expect("shutdown");

        let health = telemetry.health();
        assert_eq!(health.dropped_exports_total, 1);
        assert_eq!(health.state, TelemetryHealthState::Unavailable);
    }

    #[test]
    fn orphaned_ended_span_is_absorbed_and_counted() {
        let trace = trace_context();
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");
        let ended = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace,
            Map::new(),
        )
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));

        assert!(telemetry.emit_span(&SpanSignal::Ended(ended)).is_ok());
        assert_eq!(telemetry.health().malformed_spans_total, 1);
    }

    #[test]
    fn exporter_failure_accounting_is_tracked() {
        let log_exporter = Arc::new(RecordingLogExporter::default());
        log_exporter.fail.store(true, Ordering::SeqCst);
        let telemetry = Telemetry::new_with_exporters(
            telemetry_config(),
            log_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("telemetry");

        telemetry
            .emit_log(&log_event(service_name(), "export"))
            .expect("emit");
        let result = telemetry.flush();

        assert!(result.is_ok());
        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Degraded);
        assert_eq!(health.dropped_exports_total, 1);
        assert_eq!(
            health.exporter_statuses[0].state,
            ExporterHealthState::Degraded
        );
    }

    #[test]
    fn exporter_health_recovers_after_a_successful_flush() {
        let log_exporter = Arc::new(RecordingLogExporter::default());
        log_exporter.fail.store(true, Ordering::SeqCst);
        let telemetry = Telemetry::new_with_exporters(
            telemetry_config(),
            log_exporter.clone(),
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("telemetry");

        telemetry
            .emit_log(&log_event(service_name(), "first"))
            .expect("emit first");
        telemetry.flush().expect("first flush remains fail-open");
        assert_eq!(telemetry.health().state, TelemetryHealthState::Degraded);

        log_exporter.fail.store(false, Ordering::SeqCst);
        telemetry
            .emit_log(&log_event(service_name(), "second"))
            .expect("emit second");
        telemetry.flush().expect("second flush");

        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Healthy);
        assert_eq!(
            health.exporter_statuses[0].state,
            ExporterHealthState::Healthy
        );
        assert!(health.exporter_statuses[0].last_error.is_none());
    }

    #[test]
    fn post_shutdown_returns_shutdown_error() {
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");
        telemetry.shutdown().expect("shutdown");

        assert!(matches!(
            telemetry.emit_log(&log_event(service_name(), "after-shutdown")),
            Err(TelemetryError::Shutdown)
        ));
    }

    #[test]
    fn emit_methods_return_shutdown_after_shutdown() {
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");
        let trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace,
            Map::new(),
        );
        let metric = MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: service_name(),
            name: MetricName::new("agent.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some("1".to_string()),
            attributes: Map::new(),
        };

        telemetry.shutdown().expect("shutdown");

        assert!(matches!(
            telemetry.emit_log(&log_event(service_name(), "after-shutdown")),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            telemetry.emit_span(&SpanSignal::Started(started)),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            telemetry.emit_metric(&metric),
            Err(TelemetryError::Shutdown)
        ));
    }

    #[test]
    fn shutdown_flushes_complete_spans_and_counts_incomplete_ones() {
        let trace_exporter = Arc::new(RecordingTraceExporter::default());
        let telemetry = Telemetry::new_with_exporters(
            telemetry_config(),
            Arc::new(RecordingLogExporter::default()),
            trace_exporter.clone(),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("telemetry");

        let complete_trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("complete.run").expect("valid action"),
            complete_trace.clone(),
            Map::new(),
        );
        let ended = started
            .clone()
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));
        telemetry
            .emit_span(&SpanSignal::Started(started))
            .expect("started");
        telemetry
            .emit_span(&SpanSignal::Ended(ended))
            .expect("ended");

        let incomplete_trace = TraceContext {
            trace_id: TraceId::new("abcdefabcdefabcdefabcdefabcdefab").expect("valid trace"),
            span_id: SpanId::new("abcdefabcdefabcd").expect("valid span"),
            parent_span_id: None,
        };
        telemetry
            .emit_span(&SpanSignal::Started(SpanRecord::<SpanStarted>::new(
                Timestamp::UNIX_EPOCH,
                service_name(),
                ActionName::new("incomplete.run").expect("valid action"),
                incomplete_trace,
                Map::new(),
            )))
            .expect("incomplete");

        telemetry.shutdown().expect("shutdown");

        assert_eq!(
            *trace_exporter.calls.lock().expect("calls poisoned"),
            vec![1]
        );
        assert_eq!(telemetry.health().dropped_exports_total, 1);
    }

    #[test]
    fn repeated_shutdown_is_idempotent() {
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");

        telemetry.shutdown().expect("first shutdown");
        telemetry.shutdown().expect("second shutdown");
    }

    #[test]
    fn shutdown_absorbs_export_failures_and_records_them_in_health() {
        let log_exporter = Arc::new(RecordingLogExporter::default());
        log_exporter.fail.store(true, Ordering::SeqCst);
        let telemetry = Telemetry::new_with_exporters(
            telemetry_config(),
            log_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("telemetry");

        telemetry
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("emit");

        telemetry.shutdown().expect("shutdown remains fail-open");

        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Unavailable);
        assert_eq!(health.dropped_exports_total, 1);
        assert_eq!(
            health.exporter_statuses[0].state,
            ExporterHealthState::Degraded
        );
    }
}
