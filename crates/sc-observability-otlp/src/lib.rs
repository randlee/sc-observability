//! OTLP-backed telemetry layered on top of `sc-observe`.
//!
//! This crate owns telemetry configuration, span assembly, exporter contracts,
//! and the lifecycle/runtime behavior for OTLP-bound signals. It attaches to
//! routing through ordinary projector registration and keeps OpenTelemetry
//! transport concerns out of the lower crates.

mod assembly;
mod config;
mod projectors;

pub mod constants;
pub mod error_codes;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use config::validate_config;
use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, ErrorContext, ExportError, FlushError, InitError, LogEvent,
    MetricRecord, ObservabilityHealthProvider, Remediation, ShutdownError, SpanSignal,
    telemetry_health_provider_sealed,
};
#[doc(inline)]
pub use sc_observability_types::{
    ExporterHealth, ExporterHealthState, TelemetryError, TelemetryHealthReport,
    TelemetryHealthState,
};
use serde_json::Value;

#[doc(inline)]
pub use assembly::{CompleteSpan, SpanAssembler};
#[doc(inline)]
pub use config::{
    AuthHeader, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
    ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder, TracesConfig,
};
#[doc(inline)]
pub use projectors::TelemetryProjectors;

/// Exporter contract for projected log records.
pub trait LogExporter: Send + Sync {
    /// Exports one batch of log events.
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

/// Exporter contract for completed spans.
pub trait TraceExporter: Send + Sync {
    /// Exports one batch of completed spans.
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

/// Exporter contract for projected metrics.
pub trait MetricExporter: Send + Sync {
    /// Exports one batch of metric records.
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
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

#[derive(Debug, Clone, Copy)]
struct FlushOutcome {
    had_export_failure: bool,
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
    /// malformed or incomplete span is ever forwarded to the OTel backend.
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
                error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
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
            return Err(TelemetryError::ExportFailure(Box::new(context)));
        }
        if let Some(complete) = runtime.span_assembler.push(span.clone()).map_err(|err| {
            TelemetryError::ExportFailure(Box::new(error_context_from_diagnostic(err.diagnostic())))
        })? {
            runtime.span_buffer.push(complete);
        }
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
    pub fn flush(&self) -> Result<(), FlushError> {
        let _ = self.flush_outcome()?;
        Ok(())
    }

    fn flush_outcome(&self) -> Result<FlushOutcome, FlushError> {
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
        let mut had_export_failure = false;

        if !log_batch.is_empty() {
            match self.log_exporter.export_logs(&log_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Logs),
                Err(err) => {
                    had_export_failure = true;
                    self.record_export_failure(ExporterKind::Logs, log_batch.len() as u64, err)
                }
            }
        }

        if !span_batch.is_empty() {
            match self.trace_exporter.export_spans(&span_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Traces),
                Err(err) => {
                    had_export_failure = true;
                    self.record_export_failure(ExporterKind::Traces, span_batch.len() as u64, err)
                }
            }
        }

        if !metric_batch.is_empty() {
            match self.metric_exporter.export_metrics(&metric_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Metrics),
                Err(err) => {
                    had_export_failure = true;
                    self.record_export_failure(
                        ExporterKind::Metrics,
                        metric_batch.len() as u64,
                        err,
                    )
                }
            }
        }

        Ok(FlushOutcome { had_export_failure })
    }

    /// Flushes buffers, drops incomplete spans, and transitions the runtime to shutdown.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned while
    /// flushing, dropping incomplete spans, or constructing the final shutdown
    /// error state.
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let flush_outcome = self.flush_outcome().map_err(shutdown_flush_error)?;
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let dropped = runtime.span_assembler.flush_incomplete() as u64;
        if dropped > 0 {
            self.dropped_exports_total
                .fetch_add(dropped, Ordering::SeqCst);
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

        if flush_outcome.had_export_failure {
            return Err(shutdown_export_failure(runtime.last_error.clone()));
        }

        Ok(())
    }

    /// Returns the current telemetry health view.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned.
    pub fn health(&self) -> TelemetryHealthReport {
        let runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let exporter_statuses = vec![
            ExporterHealth {
                name: sc_observability_types::SinkName::new("logs")
                    .expect("logs exporter name is valid"),
                state: runtime.log_status.state,
                last_error: runtime.log_status.last_error.clone(),
            },
            ExporterHealth {
                name: sc_observability_types::SinkName::new("traces")
                    .expect("traces exporter name is valid"),
                state: runtime.trace_status.state,
                last_error: runtime.trace_status.last_error.clone(),
            },
            ExporterHealth {
                name: sc_observability_types::SinkName::new("metrics")
                    .expect("metrics exporter name is valid"),
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

    fn record_export_success(&self, exporter_kind: ExporterKind) {
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let status = exporter_kind.status_mut(&mut runtime);
        status.state = ExporterHealthState::Healthy;
        status.last_error = None;
    }

    fn record_export_failure(&self, exporter_kind: ExporterKind, dropped: u64, error: ExportError) {
        self.dropped_exports_total
            .fetch_add(dropped, Ordering::SeqCst);
        let summary = DiagnosticSummary::from(error.diagnostic());
        let mut runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        runtime.last_error = Some(summary.clone());

        let status = exporter_kind.status_mut(&mut runtime);
        status.state = ExporterHealthState::Degraded;
        status.last_error = Some(summary);
    }
}

impl telemetry_health_provider_sealed::Sealed for Telemetry {
    fn token(&self) -> telemetry_health_provider_sealed::Token {
        telemetry_health_provider_sealed::TOKEN
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
    TelemetryError::ExportFailure(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_EXPORT_FAILED,
        message,
        Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
    )))
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

fn shutdown_flush_error(error: FlushError) -> ShutdownError {
    ShutdownError(Box::new(
        ErrorContext::new(
            error_codes::TELEMETRY_FLUSH_FAILED,
            "failed to flush telemetry during shutdown",
            Remediation::recoverable(
                "inspect telemetry health and retry shutdown after the exporter recovers",
                ["retry shutdown"],
            ),
        )
        .source(Box::new(error)),
    ))
}

fn shutdown_export_failure(summary: Option<DiagnosticSummary>) -> ShutdownError {
    let mut context = ErrorContext::new(
        error_codes::TELEMETRY_FLUSH_FAILED,
        "failed to flush telemetry during shutdown",
        Remediation::recoverable(
            "inspect telemetry health and retry shutdown after the exporter recovers",
            ["retry shutdown"],
        ),
    );
    if let Some(summary) = summary {
        context = context.cause(summary.message);
        if let Some(code) = summary.code {
            context = context.detail(
                "exporter_error_code",
                Value::String(code.as_str().to_owned()),
            );
        }
    }
    ShutdownError(Box::new(context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::{
        ActionName, Diagnostic, DurationMs, ErrorCode, Level, LogEvent, MetricKind, MetricName,
        ProcessIdentity, ServiceName, SpanEvent, SpanId, SpanRecord, SpanStarted, StateTransition,
        TargetCategory, Timestamp, TraceContext, TraceId,
    };
    use serde_json::{Map, json};

    use crate::assembly::span_key;

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

    fn schema_version() -> sc_observability_types::SchemaVersion {
        sc_observability_types::SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version")
    }

    fn outcome_label(value: &str) -> sc_observability_types::OutcomeLabel {
        sc_observability_types::OutcomeLabel::new(value).expect("valid outcome label")
    }

    fn telemetry_config() -> TelemetryConfig {
        TelemetryConfigBuilder::new(service_name())
            .enable_logs(LogsConfig::default())
            .enable_traces(TracesConfig::default())
            .enable_metrics(MetricsConfig::default())
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: Some(
                    OtlpEndpoint::new("https://otel.example.internal")
                        .expect("valid OTLP endpoint"),
                ),
                ..OtelConfig::default()
            })
            .build()
            .expect("valid telemetry config")
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
            version: schema_version(),
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
            outcome: Some(outcome_label("ok")),
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
                entity_kind: TargetCategory::new("agent").expect("valid target"),
                entity_id: Some("agent-123".to_string()),
                from_state: sc_observability_types::StateName::new("idle").expect("valid state"),
                to_state: sc_observability_types::StateName::new("running").expect("valid state"),
                reason: None,
                trigger: None,
            }),
            fields: Map::from_iter([("kind".to_string(), json!(message))]),
        }
    }

    #[test]
    fn telemetry_config_builder_defaults() {
        // TelemetryConfig is constructed independently of ObservabilityConfig (OTLP-018).
        let config = TelemetryConfigBuilder::new(service_name())
            .build()
            .expect("valid config");

        assert!(config.logs.is_none());
        assert!(config.traces.is_none());
        assert!(config.metrics.is_none());
        assert!(!config.transport.enabled);
        assert_eq!(config.transport.protocol, OtlpProtocol::HttpBinary);
    }

    #[test]
    fn invalid_config_is_rejected_eagerly() {
        let result = TelemetryConfigBuilder::new(service_name())
            .with_transport(OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn all_signals_disabled_rejects_at_construction() {
        let config = TelemetryConfigBuilder::new(service_name())
            .with_transport(OtelConfig {
                enabled: false,
                endpoint: None,
                ..OtelConfig::default()
            })
            .build()
            .expect("valid config");

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
        assert_eq!(complete.record.duration_ms(), Some(DurationMs::from(42)));
    }

    #[test]
    fn span_assembler_reports_missing_event_buffer_explicitly() {
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
        let key = span_key(trace.trace_id.as_str(), trace.span_id.as_str());
        assembler.remove_event_buffer(&key);

        let error = assembler
            .push(SpanSignal::Ended(ended))
            .expect_err("missing event buffer should be explicit");
        assert_eq!(
            error.diagnostic().message,
            "missing span event buffer for a started span"
        );
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
    fn orphaned_ended_span_returns_export_failure_and_is_counted() {
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

        assert!(matches!(
            telemetry.emit_span(&SpanSignal::Ended(ended)),
            Err(TelemetryError::ExportFailure(_))
        ));
        let health = telemetry.health();
        assert_eq!(health.malformed_spans_total, 1);
        assert_eq!(
            health.last_error.and_then(|summary| summary.code),
            Some(error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED)
        );
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
    fn shutdown_propagates_flush_failures_and_records_them_in_health() {
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

        let error = telemetry
            .shutdown()
            .expect_err("shutdown should surface flush failures");
        assert_eq!(
            error.diagnostic().message,
            "failed to flush telemetry during shutdown"
        );

        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Unavailable);
        assert_eq!(health.dropped_exports_total, 1);
        assert_eq!(
            health.exporter_statuses[0].state,
            ExporterHealthState::Degraded
        );
    }
}
