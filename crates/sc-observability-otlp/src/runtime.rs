//! OTLP telemetry runtime: canonical and released admission, buffering,
//! flush and shutdown, and the health view merged with the shared exporter
//! lifecycle.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use sc_observability_types::typed::{EventFailure, FlushFailure, InitFailure, ShutdownFailure};
use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
use sc_observability_types::v2::{EventError as CanonicalEventError, ExportError};
use sc_observability_types::v2::{
    MetricRecord as CanonicalMetricRecord, SpanSignal as CanonicalSpanSignal,
};
use sc_observability_types::{
    DiagnosticSummary, ErrorContext, ExporterHealth, ExporterHealthState, LogEvent,
    ObservabilityHealthProvider, Remediation, SinkName, TelemetryHealthReport,
    TelemetryHealthState, telemetry_health_provider_sealed,
};
use serde_json::Value;

use crate::assembly::{SpanAssemblyLoss, V2SpanAssembler};
use crate::config::{
    TelemetryConfig as RuntimeTelemetryConfig, ValidatedTransportBounds, validated_telemetry_bounds,
};
use crate::contracts::{self, ExportRecord, ExporterSet, LogRecord};
use crate::exporter_factory::exporter_factory_prepared;
use crate::failure::{
    export_failure_from_canonical_event, flush_lifecycle_failure, shutdown_export_failure_typed,
};
use crate::lifecycle::{LifecycleHealth, LifecycleState, Signal};
use crate::{error_codes, export_records};

/// Metric admitted to the shared canonical buffer.
enum BufferedMetric {
    /// Canonical record, exported with its full aggregation.
    Canonical(Box<ExportRecord<CanonicalMetricRecord>>),
}

/// OTLP-backed telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "telemetry owns exporter trait objects and runtime state that are intentionally not exposed through a stable Debug contract"
)]
pub struct RuntimeTelemetry {
    config: RuntimeTelemetryConfig,
    pub(crate) exporters: ExporterSet,
    // MUTEX: exporter flush/shutdown paths mutate buffers and per-signal runtime health together;
    // Mutex keeps the buffered state and last_error snapshot consistent, and RwLock would not help
    // because these operations are write-heavy critical sections.
    pub(crate) runtime: Mutex<TelemetryRuntime>,
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
pub(crate) struct TelemetryRuntime {
    pub(crate) span_assembler: V2SpanAssembler,
    log_buffer: Vec<ExportRecord<LogRecord>>,
    span_buffer: Vec<ExportRecord<contracts::CompleteSpan>>,
    metric_buffer: Vec<BufferedMetric>,
    log_status: ExporterRuntime,
    pub(crate) trace_status: ExporterRuntime,
    pub(crate) metric_status: ExporterRuntime,
    last_error: Option<DiagnosticSummary>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExporterRuntime {
    pub(crate) state: ExporterHealthState,
    pub(crate) last_error: Option<DiagnosticSummary>,
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

/// Merges facade-local assembly state with the lifecycle core's current
/// per-signal degradation; cumulative loss counts never force a state.
fn merged_lifecycle_status(
    runtime: &ExporterRuntime,
    lifecycle: Option<&LifecycleHealth>,
    signal: Signal,
) -> ExporterRuntime {
    let mut status = runtime.clone();
    if lifecycle.is_some_and(|health| health.degraded_for(signal)) {
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
    pub(crate) fn new_prepared(
        config: RuntimeTelemetryConfig,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, InitFailure> {
        let exporters = exporter_factory_prepared(&config, bounds)
            .map_err(|error| InitFailure::from_context(error.into_context()))?;
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

    fn buffer_log(&self, event: &LogEvent) {
        let record = export_records::log_record(event);
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
            let resource = export_records::resource(complete.record.service());
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
            resource: export_records::resource(metric.service()),
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
    /// blocks that runtime thread to emulate synchronous HTTP behavior.
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
    /// SDK callers use this method to await admitted RPC completion. Synchronous HTTP
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
        let log_status =
            merged_lifecycle_status(&runtime.log_status, lifecycle_health.as_ref(), Signal::Logs);
        let trace_status = merged_lifecycle_status(
            &runtime.trace_status,
            lifecycle_health.as_ref(),
            Signal::Traces,
        );
        let metric_status = merged_lifecycle_status(
            &runtime.metric_status,
            lifecycle_health.as_ref(),
            Signal::Metrics,
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
