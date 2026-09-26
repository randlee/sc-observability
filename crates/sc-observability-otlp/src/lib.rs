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
mod lifecycle;
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

use config::validate_config_typed;
use sc_observability_types::typed::{EventFailure, FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::ExportError;
#[allow(
    deprecated,
    reason = "telemetry retains legacy error names in its published compatibility signatures"
)]
use sc_observability_types::{
    DiagnosticSummary, ErrorContext, FlushError, InitError, LogEvent, MetricRecord,
    ObservabilityHealthProvider, Remediation, ShutdownError, SinkName, SpanSignal,
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
    AuthHeader, ExporterBackend, LegacyRetryPolicy, LogsConfig, MetricsConfig, OtelConfig,
    OtlpConfigField, OtlpConfigTarget, OtlpEndpoint, OtlpProtocol, ResolvedField,
    ResourceAttributes, TelemetryConfig, TelemetryConfigBuilder, TracesConfig, ValueOrigin,
};
#[doc(inline)]
pub use projectors::TelemetryProjectors;

use contracts::{LogExporter, MetricExporter, TraceExporter};

/// OTLP-backed telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "telemetry owns exporter trait objects and runtime state that are intentionally not exposed through a stable Debug contract"
)]
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

static LOGS_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("logs").expect("logs exporter name is valid"));
static TRACES_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("traces").expect("traces exporter name is valid"));
static METRICS_EXPORTER_NAME: LazyLock<SinkName> =
    LazyLock::new(|| SinkName::new("metrics").expect("metrics exporter name is valid"));

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
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(config: TelemetryConfig) -> Result<Self, InitError> {
        Self::new_typed(config).map_err(Into::into)
    }

    /// Creates a telemetry runtime with neutral initialization failures.
    pub fn new_typed(config: TelemetryConfig) -> Result<Self, InitFailure> {
        Self::new_with_exporters_typed(
            config,
            Arc::new(NoopLogExporter),
            Arc::new(NoopTraceExporter),
            Arc::new(NoopMetricExporter),
        )
    }

    #[cfg(test)]
    #[allow(
        deprecated,
        reason = "test exporter injection retains the legacy InitError comparison boundary"
    )]
    fn new_with_exporters(
        config: TelemetryConfig,
        log_exporter: Arc<dyn LogExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
        metric_exporter: Arc<dyn MetricExporter>,
    ) -> Result<Self, InitError> {
        Self::new_with_exporters_typed(config, log_exporter, trace_exporter, metric_exporter)
            .map_err(Into::into)
    }

    fn new_with_exporters_typed(
        config: TelemetryConfig,
        log_exporter: Arc<dyn LogExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
        metric_exporter: Arc<dyn MetricExporter>,
    ) -> Result<Self, InitFailure> {
        validate_config_typed(&config)?;
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
        if let Some(complete) = runtime
            .span_assembler
            .push_typed(span.clone())
            .map_err(export_failure_from_event)?
        {
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
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published FlushError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::flush_typed(); see migrate-error-api.md."
    )]
    pub fn flush(&self) -> Result<(), FlushError> {
        self.flush_typed().map_err(Into::into)
    }

    /// Flushes telemetry with a neutral flush failure while retaining fail-open
    /// exporter semantics.
    pub fn flush_typed(&self) -> Result<(), FlushFailure> {
        let _ = self.flush_outcome().map_err(FlushFailure::from)?;
        Ok(())
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the helper preserves a Result-shaped internal API so flush behavior can grow direct error propagation without reshaping public callers"
    )]
    #[allow(
        deprecated,
        reason = "flush_outcome retains the legacy internal result while flush_typed is the public path"
    )]
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
        let mut export_failure = None;

        if !log_batch.is_empty() {
            match self.log_exporter.export_logs(&log_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Logs),
                Err(err) => {
                    self.record_export_failure(ExporterKind::Logs, log_batch.len() as u64, &err);
                    export_failure = Some(err);
                }
            }
        }

        if !span_batch.is_empty() {
            match self.trace_exporter.export_spans(&span_batch) {
                Ok(()) => self.record_export_success(ExporterKind::Traces),
                Err(err) => {
                    self.record_export_failure(ExporterKind::Traces, span_batch.len() as u64, &err);
                    export_failure = Some(err);
                }
            }
        }

        if !metric_batch.is_empty() {
            match self.metric_exporter.export_metrics(&metric_batch) {
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

        Ok(FlushOutcome { export_failure })
    }

    /// Flushes buffers, drops incomplete spans, and transitions the runtime to shutdown.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned while
    /// flushing, dropping incomplete spans, or constructing the final shutdown
    /// error state.
    #[allow(
        deprecated,
        reason = "retained compatibility lifecycle method keeps the published ShutdownError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use Telemetry::shutdown_typed(); see migrate-error-api.md."
    )]
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_typed().map_err(Into::into)
    }

    /// Flushes buffers and transitions telemetry to shutdown with a neutral
    /// shutdown failure.
    ///
    /// # Panics
    ///
    /// Panics if the internal telemetry runtime mutex has been poisoned while
    /// flushing, dropping incomplete spans, or constructing final state.
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let flush_outcome = self
            .flush_outcome()
            .map_err(FlushFailure::from)
            .map_err(shutdown_flush_failure)?;
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
        let runtime = self.runtime.lock().expect("telemetry runtime poisoned");
        let exporter_statuses = vec![
            ExporterHealth {
                name: LOGS_EXPORTER_NAME.clone(),
                state: runtime.log_status.state,
                last_error: runtime.log_status.last_error.clone(),
            },
            ExporterHealth {
                name: TRACES_EXPORTER_NAME.clone(),
                state: runtime.trace_status.state,
                last_error: runtime.trace_status.last_error.clone(),
            },
            ExporterHealth {
                name: METRICS_EXPORTER_NAME.clone(),
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
    TelemetryError::ExportFailure(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_EXPORT_FAILED,
        message,
        Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
    )))
}

/// Converts a span-assembly event failure into a telemetry export failure.
///
/// Moves the original `Box<ErrorContext>` unchanged via `into_context()`
/// rather than reconstructing a new one from its diagnostic fields, so the
/// original timestamp, backtrace, and any attached source survive intact.
fn export_failure_from_event(err: EventFailure) -> TelemetryError {
    TelemetryError::ExportFailure(err.into_context())
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
fn shutdown_flush_failure(error: FlushFailure) -> ShutdownFailure {
    ShutdownFailure::from_context(Box::new(
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

fn shutdown_export_failure_typed(
    error: ExportError,
    diagnostic_summary: Option<DiagnosticSummary>,
) -> ShutdownFailure {
    // The legacy shutdown path selected `runtime.last_error` after incomplete
    // span accounting. Preserve that diagnostic selection exactly, while the
    // typed source chain keeps the actual exporter failure available to callers.
    let summary = diagnostic_summary.unwrap_or_else(|| DiagnosticSummary::from(error.diagnostic()));
    let mut context = ErrorContext::new(
        error_codes::TELEMETRY_FLUSH_FAILED,
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

#[cfg(test)]
#[allow(
    deprecated,
    reason = "telemetry compatibility tests exercise retained lifecycle and error wrappers"
)]
mod tests {
    use super::*;
    use sc_observability_types::DiagnosticInfo;
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
                Err(ExportError::Transport {
                    context: Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_EXPORT_FAILED,
                        "log export failed",
                        Remediation::not_recoverable("test exporter failure"),
                    )),
                })
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
                Err(ExportError::Transport {
                    context: Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_EXPORT_FAILED,
                        "trace export failed",
                        Remediation::not_recoverable("test exporter failure"),
                    )),
                })
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
                Err(ExportError::Transport {
                    context: Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_EXPORT_FAILED,
                        "metric export failed",
                        Remediation::not_recoverable("test exporter failure"),
                    )),
                })
            } else {
                Ok(())
            }
        }
    }

    struct SourcePreservingLogExporter;

    impl LogExporter for SourcePreservingLogExporter {
        fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportError> {
            Err(ExportError::Transport {
                context: Box::new(
                    ErrorContext::new(
                        ErrorCode::new_static("SC_TEST_CUSTOM_EXPORT"),
                        "custom log exporter failed",
                        Remediation::not_recoverable("test native exporter source retention"),
                    )
                    .source(Box::new(std::io::Error::other(
                        "custom exporter native source",
                    ))),
                ),
            })
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

    fn metric_record() -> MetricRecord {
        MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: service_name(),
            name: MetricName::new("agent.events_total").expect("valid metric"),
            kind: MetricKind::Counter,
            value: 1.0,
            unit: Some(sc_observability_types::MetricUnit::new("1").expect("valid unit")),
            attributes: Map::new(),
        }
    }

    fn complete_span_signals() -> (SpanSignal, SpanSignal) {
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace_context(),
            Map::new(),
        );
        let ended = started
            .clone()
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));
        (SpanSignal::Started(started), SpanSignal::Ended(ended))
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
    fn telemetry_constructors_preserve_invalid_configuration_diagnostics() {
        let config = TelemetryConfig {
            service_name: service_name(),
            resource: ResourceAttributes::default(),
            transport: OtelConfig {
                enabled: true,
                endpoint: None,
                ..OtelConfig::default()
            },
            logs: Some(LogsConfig::default()),
            traces: None,
            metrics: None,
        };
        let Err(legacy) = Telemetry::new(config.clone()) else {
            panic!("legacy invalid config should fail");
        };
        let Err(typed) = Telemetry::new_typed(config) else {
            panic!("typed invalid config should fail");
        };

        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
        assert_eq!(legacy.diagnostic().cause, typed.diagnostic().cause);
        assert_eq!(
            legacy.diagnostic().remediation,
            typed.diagnostic().remediation
        );
        let legacy_context = std::error::Error::source(&legacy).expect("legacy context");
        let typed_context = std::error::Error::source(&typed).expect("typed context");
        assert!(legacy_context.source().is_none());
        assert!(typed_context.source().is_none());
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
    fn typed_span_assembler_matches_legacy_completion() {
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
                .push_typed(SpanSignal::Started(started))
                .expect("typed started")
                .is_none()
        );
        let complete = assembler
            .push_typed(SpanSignal::Ended(ended))
            .expect("typed ended")
            .expect("complete span");

        assert!(complete.events.is_empty());
        assert_eq!(complete.record.duration_ms(), Some(DurationMs::from(42)));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the paired lifecycle transitions remain adjacent so parity is auditable"
    )]
    fn span_assembler_typed_and_legacy_errors_preserve_lifecycle_diagnostics() {
        fn assert_parity<L, T>(legacy: &L, typed: &T, expected_message: &str)
        where
            L: DiagnosticInfo + std::error::Error,
            T: DiagnosticInfo + std::error::Error,
        {
            assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
            assert_eq!(legacy.diagnostic().message, expected_message);
            assert_eq!(typed.diagnostic().message, expected_message);
            assert_eq!(legacy.diagnostic().cause, typed.diagnostic().cause);
            assert_eq!(
                legacy.diagnostic().remediation,
                typed.diagnostic().remediation
            );
            let legacy_context =
                std::error::Error::source(legacy).expect("legacy error context source");
            let typed_context =
                std::error::Error::source(typed).expect("typed error context source");
            assert!(std::error::Error::source(legacy_context).is_none());
            assert!(std::error::Error::source(typed_context).is_none());
        }

        let orphan_trace = trace_context();
        let orphan_event = SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: orphan_trace.clone(),
            name: ActionName::new("tool.call").expect("valid name"),
            attributes: Map::new(),
            diagnostic: None,
        });
        let orphan_ended = SpanSignal::Ended(
            SpanRecord::<SpanStarted>::new(
                Timestamp::UNIX_EPOCH,
                service_name(),
                ActionName::new("agent.run").expect("valid action"),
                orphan_trace,
                Map::new(),
            )
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42)),
        );
        let mut legacy = SpanAssembler::new();
        let mut typed = SpanAssembler::new();
        let error = legacy
            .push(orphan_event.clone())
            .expect_err("legacy orphan event");
        let typed_error = typed
            .push_typed(orphan_event)
            .expect_err("typed orphan event");
        assert_parity(
            &error,
            &typed_error,
            "received span event without a matching started span",
        );

        let mut legacy = SpanAssembler::new();
        let mut typed = SpanAssembler::new();
        let error = legacy
            .push(orphan_ended.clone())
            .expect_err("legacy orphan ended span");
        let typed_error = typed
            .push_typed(orphan_ended)
            .expect_err("typed orphan ended span");
        assert_parity(
            &error,
            &typed_error,
            "received ended span without a matching started span",
        );

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
        let mut legacy = SpanAssembler::new();
        let mut typed = SpanAssembler::new();

        assert!(
            legacy
                .push(SpanSignal::Started(started.clone()))
                .expect("legacy started")
                .is_none()
        );
        assert!(
            typed
                .push_typed(SpanSignal::Started(started))
                .expect("typed started")
                .is_none()
        );
        let key = span_key(trace.trace_id.as_str(), trace.span_id.as_str());
        legacy.remove_event_buffer(&key);
        typed.remove_event_buffer(&key);

        let error = legacy
            .push(SpanSignal::Ended(ended.clone()))
            .expect_err("legacy missing event buffer");
        let typed_error = typed
            .push_typed(SpanSignal::Ended(ended))
            .expect_err("typed missing event buffer");
        assert_parity(
            &error,
            &typed_error,
            "missing span event buffer for a started span",
        );
    }

    #[test]
    fn incomplete_span_drop_accounting_is_paired_for_legacy_and_typed_shutdown() {
        let trace = trace_context();
        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace,
            Map::new(),
        );
        let legacy = Telemetry::new(telemetry_config()).expect("legacy telemetry");
        let typed = Telemetry::new_typed(telemetry_config()).expect("typed telemetry");

        legacy
            .emit_span(&SpanSignal::Started(started.clone()))
            .expect("legacy started");
        typed
            .emit_span(&SpanSignal::Started(started))
            .expect("typed started");
        legacy.shutdown().expect("legacy shutdown");
        typed.shutdown_typed().expect("typed shutdown");

        let legacy_health = legacy.health();
        let typed_health = typed.health();
        assert_eq!(legacy_health.dropped_exports_total, 1);
        assert_eq!(
            legacy_health.dropped_exports_total,
            typed_health.dropped_exports_total
        );
        assert_eq!(legacy_health.state, TelemetryHealthState::Unavailable);
        assert_eq!(legacy_health.state, typed_health.state);
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
    fn orphaned_span_event_returns_export_failure_from_span_assembler() {
        let trace = trace_context();
        let telemetry = Telemetry::new(telemetry_config()).expect("telemetry");
        let event = SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace,
            name: ActionName::new("agent.tool_call").expect("valid action"),
            attributes: Map::new(),
            diagnostic: None,
        };

        let error = telemetry
            .emit_span(&SpanSignal::Event(event))
            .expect_err("event without a matching started span is rejected");
        let TelemetryError::ExportFailure(context) = error else {
            panic!("expected an export failure");
        };
        assert_eq!(
            context.diagnostic().code,
            error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED
        );
        assert_eq!(
            context.diagnostic().message,
            "received span event without a matching started span"
        );
    }

    #[test]
    fn export_failure_from_event_moves_original_context_without_reconstruction() {
        let context = Box::new(
            ErrorContext::new(
                error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                "received span event without a matching started span",
                Remediation::not_recoverable(
                    "emit started, event, and ended span signals in order",
                ),
            )
            .source(Box::new(std::io::Error::other(
                "orphaned span event source",
            ))),
        );
        let original_timestamp = context.diagnostic().timestamp;
        let original_backtrace_ptr = std::ptr::from_ref(context.backtrace());
        let failure = EventFailure::from_context(context);

        let TelemetryError::ExportFailure(exported) = export_failure_from_event(failure) else {
            panic!("expected an export failure");
        };

        assert_eq!(exported.diagnostic().timestamp, original_timestamp);
        assert_eq!(
            std::ptr::from_ref(exported.backtrace()),
            original_backtrace_ptr,
            "the original context's backtrace allocation must be moved, not recaptured"
        );
        let source = std::error::Error::source(&*exported).expect("source must be preserved");
        assert_eq!(source.to_string(), "orphaned span event source");
    }

    #[test]
    fn shutdown_flush_failure_preserves_flush_context_as_native_source() {
        let flush_failure = FlushFailure::from_context(Box::new(
            ErrorContext::new(
                error_codes::TELEMETRY_EXPORT_FAILED,
                "log export failed",
                Remediation::not_recoverable("test flush failure"),
            )
            .source(Box::new(std::io::Error::other("native flush source"))),
        ));

        let shutdown_failure = shutdown_flush_failure(flush_failure);

        assert_eq!(
            shutdown_failure.diagnostic().code,
            error_codes::TELEMETRY_FLUSH_FAILED
        );
        assert_eq!(
            shutdown_failure.diagnostic().message,
            "failed to flush telemetry during shutdown"
        );
        let shutdown_context = std::error::Error::source(&shutdown_failure)
            .expect("shutdown failure preserves its context");
        let chained_flush_failure = shutdown_context
            .source()
            .expect("shutdown context preserves the flush failure");
        assert_eq!(
            chained_flush_failure.to_string(),
            "log export failed; caused by: native flush source"
        );
        let flush_context = chained_flush_failure
            .source()
            .expect("flush failure preserves its context");
        let native_source = flush_context
            .source()
            .expect("flush context preserves the native export source");
        assert_eq!(native_source.to_string(), "native flush source");
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
    #[expect(
        clippy::too_many_lines,
        reason = "the paired exporter matrix remains adjacent so all six consumers are auditable"
    )]
    fn legacy_and_typed_flush_record_and_recover_all_exporter_families() {
        let legacy_log = Arc::new(RecordingLogExporter::default());
        let legacy_trace = Arc::new(RecordingTraceExporter::default());
        let legacy_metric = Arc::new(RecordingMetricExporter::default());
        let typed_log = Arc::new(RecordingLogExporter::default());
        let typed_trace = Arc::new(RecordingTraceExporter::default());
        let typed_metric = Arc::new(RecordingMetricExporter::default());
        for failure_switch in [
            &legacy_log.fail,
            &legacy_trace.fail,
            &legacy_metric.fail,
            &typed_log.fail,
            &typed_trace.fail,
            &typed_metric.fail,
        ] {
            failure_switch.store(true, Ordering::SeqCst);
        }
        let legacy = Telemetry::new_with_exporters(
            telemetry_config(),
            legacy_log.clone(),
            legacy_trace.clone(),
            legacy_metric.clone(),
        )
        .expect("legacy telemetry");
        let typed = Telemetry::new_with_exporters_typed(
            telemetry_config(),
            typed_log.clone(),
            typed_trace.clone(),
            typed_metric.clone(),
        )
        .expect("typed telemetry");

        legacy
            .emit_log(&log_event(service_name(), "first"))
            .expect("legacy log");
        let (started, ended) = complete_span_signals();
        legacy.emit_span(&started).expect("legacy started");
        legacy.emit_span(&ended).expect("legacy ended");
        legacy.emit_metric(&metric_record()).expect("legacy metric");
        typed
            .emit_log(&log_event(service_name(), "first"))
            .expect("typed log");
        let (started, ended) = complete_span_signals();
        typed.emit_span(&started).expect("typed started");
        typed.emit_span(&ended).expect("typed ended");
        typed.emit_metric(&metric_record()).expect("typed metric");
        legacy.flush().expect("legacy fail-open flush");
        typed.flush_typed().expect("typed fail-open flush");
        assert!(
            legacy
                .health()
                .exporter_statuses
                .iter()
                .all(|status| status.state == ExporterHealthState::Degraded)
        );
        assert!(
            typed
                .health()
                .exporter_statuses
                .iter()
                .all(|status| status.state == ExporterHealthState::Degraded)
        );

        for failure_switch in [
            &legacy_log.fail,
            &legacy_trace.fail,
            &legacy_metric.fail,
            &typed_log.fail,
            &typed_trace.fail,
            &typed_metric.fail,
        ] {
            failure_switch.store(false, Ordering::SeqCst);
        }
        legacy
            .emit_log(&log_event(service_name(), "second"))
            .expect("legacy log");
        let (started, ended) = complete_span_signals();
        legacy.emit_span(&started).expect("legacy started");
        legacy.emit_span(&ended).expect("legacy ended");
        legacy.emit_metric(&metric_record()).expect("legacy metric");
        typed
            .emit_log(&log_event(service_name(), "second"))
            .expect("typed log");
        let (started, ended) = complete_span_signals();
        typed.emit_span(&started).expect("typed started");
        typed.emit_span(&ended).expect("typed ended");
        typed.emit_metric(&metric_record()).expect("typed metric");
        legacy.flush().expect("legacy recovery flush");
        typed.flush_typed().expect("typed recovery flush");
        assert!(
            legacy
                .health()
                .exporter_statuses
                .iter()
                .all(|status| status.state == ExporterHealthState::Healthy)
        );
        assert!(
            typed
                .health()
                .exporter_statuses
                .iter()
                .all(|status| status.state == ExporterHealthState::Healthy)
        );
        for calls in [
            &legacy_log.calls,
            &legacy_trace.calls,
            &legacy_metric.calls,
            &typed_log.calls,
            &typed_trace.calls,
            &typed_metric.calls,
        ] {
            assert_eq!(*calls.lock().expect("calls poisoned"), vec![1, 1]);
        }
    }

    #[test]
    fn retained_emit_methods_return_shutdown_after_legacy_and_typed_lifecycle() {
        // Emitters intentionally remain the B.1 legacy public surface. This
        // pairs their unchanged calls after each lifecycle entry point rather
        // than adding parallel typed emitter methods to this preparation layer.
        let legacy = Telemetry::new(telemetry_config()).expect("legacy telemetry");
        let typed = Telemetry::new_typed(telemetry_config()).expect("typed telemetry");
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
            unit: Some(sc_observability_types::MetricUnit::new("1").expect("valid metric unit")),
            attributes: Map::new(),
        };

        legacy.shutdown().expect("legacy shutdown");
        typed.shutdown_typed().expect("typed shutdown");

        assert!(matches!(
            legacy.emit_log(&log_event(service_name(), "after-shutdown")),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            legacy.emit_span(&SpanSignal::Started(started.clone())),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            legacy.emit_metric(&metric),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            typed.emit_log(&log_event(service_name(), "after-shutdown")),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            typed.emit_span(&SpanSignal::Started(started)),
            Err(TelemetryError::Shutdown)
        ));
        assert!(matches!(
            typed.emit_metric(&metric),
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
    fn typed_telemetry_lifecycle_preserves_fail_open_and_repeat_shutdown() {
        let telemetry = Telemetry::new_typed(telemetry_config()).expect("typed telemetry");

        telemetry
            .flush_typed()
            .expect("typed flush remains fail-open");
        telemetry.shutdown_typed().expect("typed first shutdown");
        telemetry.shutdown_typed().expect("typed repeated shutdown");
    }

    #[test]
    fn shutdown_propagates_flush_failures_with_legacy_and_typed_parity() {
        let legacy_exporter = Arc::new(RecordingLogExporter::default());
        legacy_exporter.fail.store(true, Ordering::SeqCst);
        let typed_exporter = Arc::new(RecordingLogExporter::default());
        typed_exporter.fail.store(true, Ordering::SeqCst);
        let legacy = Telemetry::new_with_exporters(
            telemetry_config(),
            legacy_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("legacy telemetry");
        let typed = Telemetry::new_with_exporters_typed(
            telemetry_config(),
            typed_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("typed telemetry");

        legacy
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("legacy emit");
        typed
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("typed emit");

        let legacy_error = legacy
            .shutdown()
            .expect_err("legacy shutdown should surface flush failures");
        let typed_error = typed
            .shutdown_typed()
            .expect_err("typed shutdown should surface flush failures");
        assert_eq!(
            legacy_error.diagnostic().code,
            typed_error.diagnostic().code
        );
        assert_eq!(
            legacy_error.diagnostic().message,
            "failed to flush telemetry during shutdown"
        );
        assert_eq!(
            legacy_error.diagnostic().message,
            typed_error.diagnostic().message
        );
        assert_eq!(
            legacy_error.diagnostic().details,
            typed_error.diagnostic().details
        );

        let legacy_health = legacy.health();
        let typed_health = typed.health();
        assert_eq!(legacy_health.state, TelemetryHealthState::Unavailable);
        assert_eq!(legacy_health.state, typed_health.state);
        assert_eq!(legacy_health.dropped_exports_total, 1);
        assert_eq!(
            legacy_health.dropped_exports_total,
            typed_health.dropped_exports_total
        );
        assert_eq!(
            legacy_health.exporter_statuses[0].state,
            ExporterHealthState::Degraded
        );
        assert_eq!(
            legacy_health.exporter_statuses[0].state,
            typed_health.exporter_statuses[0].state
        );
    }

    #[test]
    fn shutdown_preserves_custom_export_code_and_native_source_for_both_apis() {
        fn assert_custom_source<E>(error: &E, telemetry: &Telemetry)
        where
            E: DiagnosticInfo + std::error::Error,
        {
            assert_eq!(
                error.diagnostic().details.get("exporter_error_code"),
                Some(&Value::String("SC_TEST_CUSTOM_EXPORT".to_owned()))
            );
            assert_eq!(
                telemetry
                    .health()
                    .last_error
                    .and_then(|summary| summary.code),
                Some(ErrorCode::new_static("SC_TEST_CUSTOM_EXPORT"))
            );

            let shutdown_context =
                std::error::Error::source(error).expect("shutdown failure preserves its context");
            let export_failure = shutdown_context
                .source()
                .expect("shutdown context preserves export failure");
            let export_context = export_failure
                .source()
                .expect("export failure preserves its context");
            let native_source = export_context
                .source()
                .expect("export context preserves native source");
            assert_eq!(native_source.to_string(), "custom exporter native source");
        }

        let legacy = Telemetry::new_with_exporters(
            telemetry_config(),
            Arc::new(SourcePreservingLogExporter),
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("legacy telemetry");
        let typed = Telemetry::new_with_exporters_typed(
            telemetry_config(),
            Arc::new(SourcePreservingLogExporter),
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("typed telemetry");
        legacy
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("legacy emit");
        typed
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("typed emit");

        let legacy_error = legacy
            .shutdown()
            .expect_err("legacy shutdown should retain the exporter failure");
        let typed_error = typed
            .shutdown_typed()
            .expect_err("typed shutdown should retain the exporter failure");
        assert_custom_source(&legacy_error, &legacy);
        assert_custom_source(&typed_error, &typed);
    }

    #[test]
    fn combined_export_failure_and_incomplete_span_preserve_baseline_shutdown_summary() {
        fn assert_baseline_summary<E>(error: &E, telemetry: &Telemetry)
        where
            E: DiagnosticInfo,
        {
            assert_eq!(error.diagnostic().code, error_codes::TELEMETRY_FLUSH_FAILED);
            assert_eq!(
                error.diagnostic().cause.as_deref(),
                Some("dropped incomplete spans during shutdown")
            );
            assert_eq!(
                error.diagnostic().details.get("exporter_error_code"),
                Some(&Value::String(
                    error_codes::TELEMETRY_INCOMPLETE_SPAN_DROPPED
                        .as_str()
                        .to_owned(),
                ))
            );
            let health = telemetry.health();
            assert_eq!(health.state, TelemetryHealthState::Unavailable);
            assert_eq!(health.dropped_exports_total, 2);
            assert_eq!(
                health.last_error.and_then(|summary| summary.code),
                Some(error_codes::TELEMETRY_INCOMPLETE_SPAN_DROPPED)
            );
            assert_eq!(
                health.exporter_statuses[0].state,
                ExporterHealthState::Degraded
            );
            assert_eq!(
                health.exporter_statuses[1].state,
                ExporterHealthState::Degraded
            );
        }

        let legacy_exporter = Arc::new(RecordingLogExporter::default());
        legacy_exporter.fail.store(true, Ordering::SeqCst);
        let typed_exporter = Arc::new(RecordingLogExporter::default());
        typed_exporter.fail.store(true, Ordering::SeqCst);
        let legacy = Telemetry::new_with_exporters(
            telemetry_config(),
            legacy_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("legacy telemetry");
        let typed = Telemetry::new_with_exporters_typed(
            telemetry_config(),
            typed_exporter,
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("typed telemetry");

        for telemetry in [&legacy, &typed] {
            telemetry
                .emit_log(&log_event(service_name(), "shutdown-export"))
                .expect("emit log");
            let (started, _) = complete_span_signals();
            telemetry.emit_span(&started).expect("emit incomplete span");
        }

        let legacy_error = legacy
            .shutdown()
            .expect_err("legacy shutdown should report final export failure");
        let typed_error = typed
            .shutdown_typed()
            .expect_err("typed shutdown should report final export failure");
        assert_baseline_summary(&legacy_error, &legacy);
        assert_baseline_summary(&typed_error, &typed);
        assert_eq!(
            legacy_error.diagnostic().code,
            typed_error.diagnostic().code
        );
        assert_eq!(
            legacy_error.diagnostic().cause,
            typed_error.diagnostic().cause
        );
        assert_eq!(
            legacy_error.diagnostic().details,
            typed_error.diagnostic().details
        );

        legacy.shutdown().expect("legacy repeated shutdown");
        typed.shutdown_typed().expect("typed repeated shutdown");
    }
}
