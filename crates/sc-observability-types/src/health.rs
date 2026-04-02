use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::DiagnosticSummary;

/// Top-level health state for the lightweight logging layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoggingHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

/// Health state for an individual log sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SinkHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

/// Health summary for one concrete logging sink.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinkHealth {
    pub name: String,
    pub state: SinkHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

/// Aggregate logging health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingHealthReport {
    pub state: LoggingHealthState,
    pub dropped_events_total: u64,
    pub flush_errors_total: u64,
    pub active_log_path: PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub last_error: Option<DiagnosticSummary>,
}

/// Top-level health state for the observation routing runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

/// Top-level health state for telemetry export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryHealthState {
    Disabled,
    Healthy,
    Degraded,
    Unavailable,
}

/// Health state for an individual telemetry exporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExporterHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

/// Health summary for one configured telemetry exporter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExporterHealth {
    pub name: String,
    pub state: ExporterHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

/// Aggregate telemetry/export health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryHealthReport {
    pub state: TelemetryHealthState,
    pub dropped_exports_total: u64,
    pub malformed_spans_total: u64,
    pub exporter_statuses: Vec<ExporterHealth>,
    pub last_error: Option<DiagnosticSummary>,
}

/// Aggregate routing/runtime health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservabilityHealthReport {
    pub state: ObservationHealthState,
    pub dropped_observations_total: u64,
    pub subscriber_failures_total: u64,
    pub projection_failures_total: u64,
    pub logging: Option<LoggingHealthReport>,
    pub telemetry: Option<TelemetryHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}
