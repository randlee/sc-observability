pub mod constants;
pub mod error_codes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability_types::{
    DiagnosticSummary, ErrorContext, EventError, ExportError, ExporterHealth, ExporterHealthState,
    FlushError, LogEvent, MetricRecord, Remediation, ServiceName, SpanEnded, SpanEvent, SpanRecord,
    SpanSignal, SpanStarted, TelemetryError, TelemetryHealthReport, TelemetryHealthState,
};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    HttpBinary,
    HttpJson,
    Grpc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub protocol: OtlpProtocol,
    pub auth_header: Option<String>,
    pub ca_file: Option<PathBuf>,
    pub insecure_skip_verify: bool,
    pub timeout_ms: u64,
    pub debug_local_export: bool,
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
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
            timeout_ms: constants::DEFAULT_OTLP_TIMEOUT_MS,
            debug_local_export: false,
            max_retries: constants::DEFAULT_OTLP_MAX_RETRIES,
            initial_backoff_ms: constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS,
            max_backoff_ms: constants::DEFAULT_OTLP_MAX_BACKOFF_MS,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceAttributes {
    pub attributes: Map<String, Value>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsConfig {
    pub batch_size: usize,
    pub export_interval_ms: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            batch_size: constants::DEFAULT_METRIC_BATCH_SIZE,
            export_interval_ms: constants::DEFAULT_METRIC_EXPORT_INTERVAL_MS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub service_name: ServiceName,
    pub resource: ResourceAttributes,
    pub transport: OtelConfig,
    pub logs: Option<LogsConfig>,
    pub traces: Option<TracesConfig>,
    pub metrics: Option<MetricsConfig>,
}

pub struct TelemetryConfigBuilder {
    service_name: ServiceName,
    resource: ResourceAttributes,
    transport: OtelConfig,
    logs: Option<LogsConfig>,
    traces: Option<TracesConfig>,
    metrics: Option<MetricsConfig>,
}

impl TelemetryConfigBuilder {
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

    pub fn with_resource(mut self, resource: ResourceAttributes) -> Self {
        self.resource = resource;
        self
    }

    pub fn with_transport(mut self, transport: OtelConfig) -> Self {
        self.transport = transport;
        self
    }

    pub fn enable_logs(mut self, config: LogsConfig) -> Self {
        self.logs = Some(config);
        self
    }

    pub fn enable_traces(mut self, config: TracesConfig) -> Self {
        self.traces = Some(config);
        self
    }

    pub fn enable_metrics(mut self, config: MetricsConfig) -> Self {
        self.metrics = Some(config);
        self
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteSpan {
    pub record: SpanRecord<SpanEnded>,
    pub events: Vec<SpanEvent>,
}

pub trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

pub trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

pub trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}

pub struct SpanAssembler {
    started: HashMap<String, sc_observability_types::SpanRecord<SpanStarted>>,
    events: HashMap<String, Vec<SpanEvent>>,
}

impl SpanAssembler {
    pub fn new() -> Self {
        Self {
            started: HashMap::new(),
            events: HashMap::new(),
        }
    }

    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError> {
        match signal {
            SpanSignal::Started(record) => {
                self.started
                    .insert(record.trace().span_id.as_str().to_string(), record);
                Ok(None)
            }
            SpanSignal::Event(event) => {
                self.events
                    .entry(event.trace.span_id.as_str().to_string())
                    .or_default()
                    .push(event);
                Ok(None)
            }
            SpanSignal::Ended(record) => Ok(Some(CompleteSpan {
                events: self
                    .events
                    .remove(record.trace().span_id.as_str())
                    .unwrap_or_default(),
                record,
            })),
        }
    }

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

pub struct Telemetry {
    shutdown: AtomicBool,
    _config: TelemetryConfig,
}

impl Telemetry {
    pub fn new(config: TelemetryConfig) -> Result<Self, sc_observability_types::InitError> {
        Ok(Self {
            shutdown: AtomicBool::new(false),
            _config: config,
        })
    }

    pub fn emit_log(&self, _event: &LogEvent) -> Result<(), TelemetryError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(TelemetryError::Shutdown);
        }
        Ok(())
    }

    pub fn emit_span(&self, _span: &SpanSignal) -> Result<(), TelemetryError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(TelemetryError::Shutdown);
        }
        Ok(())
    }

    pub fn emit_metric(&self, _metric: &MetricRecord) -> Result<(), TelemetryError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(TelemetryError::Shutdown);
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<(), FlushError> {
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), sc_observability_types::ShutdownError> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn health(&self) -> TelemetryHealthReport {
        TelemetryHealthReport {
            state: if self._config.transport.enabled {
                TelemetryHealthState::Healthy
            } else {
                TelemetryHealthState::Disabled
            },
            dropped_exports_total: 0,
            exporter_statuses: vec![ExporterHealth {
                name: "otlp".to_string(),
                state: ExporterHealthState::Healthy,
                last_error: Option::<DiagnosticSummary>::None,
            }],
            last_error: None,
        }
    }
}

mod sealed_emitters {
    pub trait Sealed {}
}

pub trait SpanEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_span(&self, span: SpanSignal) -> Result<(), TelemetryError>;
}

pub trait MetricEmitter: sealed_emitters::Sealed + Send + Sync {
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

pub fn export_failure(message: impl Into<String>) -> TelemetryError {
    TelemetryError::ExportFailure(Box::new(ErrorContext::new(
        error_codes::TELEMETRY_EXPORT_FAILED,
        message,
        Remediation::not_recoverable("retry/export policy is owned by telemetry runtime"),
    )))
}
