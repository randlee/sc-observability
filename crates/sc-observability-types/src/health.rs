use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{DiagnosticSummary, SinkName, telemetry_health_provider_sealed};

/// Top-level health state for the lightweight logging layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoggingHealthState {
    /// Logging is operating normally.
    Healthy,
    /// Logging is operating but dropping some events or flushes.
    DegradedDropping,
    /// Logging is unavailable.
    Unavailable,
}

/// Health state for an individual log sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SinkHealthState {
    /// The sink is operating normally.
    Healthy,
    /// The sink is operating but dropping writes.
    DegradedDropping,
    /// The sink is unavailable.
    Unavailable,
}

/// Health summary for one concrete logging sink.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinkHealth {
    /// Stable sink name.
    pub name: SinkName,
    /// Current sink health state.
    pub state: SinkHealthState,
    /// Optional last sink error summary.
    pub last_error: Option<DiagnosticSummary>,
}

/// Aggregate logging health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingHealthReport {
    /// Aggregate logging health state.
    pub state: LoggingHealthState,
    /// Total dropped log events.
    pub dropped_events_total: u64,
    /// Total flush failures.
    pub flush_errors_total: u64,
    /// Active JSONL log path used by the logger.
    pub active_log_path: PathBuf,
    /// Per-sink health snapshots.
    pub sink_statuses: Vec<SinkHealth>,
    /// Optional query/follow health snapshot.
    pub query: Option<QueryHealthReport>,
    /// Optional last logging error summary.
    pub last_error: Option<DiagnosticSummary>,
}

/// Top-level health state for historical query and follow availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryHealthState {
    /// Query and follow are operating normally.
    Healthy,
    /// Query and follow are operating with degraded behavior.
    Degraded,
    /// Query and follow are unavailable.
    Unavailable,
}

/// Aggregate health report for the shared query/follow surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryHealthReport {
    /// Aggregate query/follow health state.
    pub state: QueryHealthState,
    /// Optional last query/follow error summary.
    pub last_error: Option<DiagnosticSummary>,
}

/// Top-level health state for the observation routing runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationHealthState {
    /// Routing is operating normally.
    Healthy,
    /// Routing is operating with degraded behavior.
    Degraded,
    /// Routing is unavailable.
    Unavailable,
}

/// Top-level health state for telemetry export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryHealthState {
    /// Telemetry is disabled by configuration.
    Disabled,
    /// Telemetry is operating normally.
    Healthy,
    /// Telemetry is operating with degraded exporters or dropped data.
    Degraded,
    /// Telemetry is unavailable.
    Unavailable,
}

/// Health state for an individual telemetry exporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExporterHealthState {
    /// The exporter is operating normally.
    Healthy,
    /// The exporter is operating with degraded behavior.
    Degraded,
    /// The exporter is unavailable.
    Unavailable,
}

/// Health summary for one configured telemetry exporter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExporterHealth {
    /// Stable exporter name.
    pub name: SinkName,
    /// Current exporter health state.
    pub state: ExporterHealthState,
    /// Optional last exporter error summary.
    pub last_error: Option<DiagnosticSummary>,
}

/// Aggregate telemetry/export health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryHealthReport {
    /// Aggregate telemetry health state.
    pub state: TelemetryHealthState,
    /// Total dropped exports.
    pub dropped_exports_total: u64,
    /// Total malformed or incomplete spans observed by telemetry.
    pub malformed_spans_total: u64,
    /// Per-exporter health snapshots.
    pub exporter_statuses: Vec<ExporterHealth>,
    /// Optional last telemetry error summary.
    pub last_error: Option<DiagnosticSummary>,
}

/// Shared contract for exposing telemetry health without an OTLP crate dependency.
pub trait ObservabilityHealthProvider:
    telemetry_health_provider_sealed::Sealed + Send + Sync
{
    /// Returns the current telemetry health snapshot.
    fn telemetry_health(&self) -> TelemetryHealthReport;
}

/// Aggregate routing/runtime health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservabilityHealthReport {
    /// Aggregate routing health state.
    pub state: ObservationHealthState,
    /// Total observations dropped because no route handled them.
    pub dropped_observations_total: u64,
    /// Total subscriber failures recorded by the runtime.
    pub subscriber_failures_total: u64,
    /// Total projector failures recorded by the runtime.
    pub projection_failures_total: u64,
    /// Optional attached logging health.
    pub logging: Option<LoggingHealthReport>,
    /// Optional attached telemetry health.
    pub telemetry: Option<TelemetryHealthReport>,
    /// Optional last routing error summary.
    pub last_error: Option<DiagnosticSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    use crate::{
        Diagnostic, ErrorCode, ExporterHealth, ExporterHealthState, LoggingHealthState,
        QueryHealthState, Remediation, Timestamp,
    };

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST_DIAGNOSTIC"),
            message: "diagnostic invalid".to_string(),
            cause: Some("invalid example".to_string()),
            remediation: Remediation::recoverable(
                "fix the input",
                ["rerun the command", "review the docs"],
            ),
            docs: Some("https://example.test/docs".to_string()),
            details: Map::from_iter([("key".to_string(), json!("value"))]),
        }
    }

    #[test]
    fn health_reports_round_trip_through_serde() {
        let sink = SinkHealth {
            name: SinkName::new("jsonl").expect("valid sink name"),
            state: SinkHealthState::Healthy,
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };
        let logging = LoggingHealthReport {
            state: LoggingHealthState::Healthy,
            dropped_events_total: 0,
            flush_errors_total: 0,
            active_log_path: std::path::PathBuf::from("/var/log/logs/service.log.jsonl"),
            sink_statuses: vec![sink],
            query: Some(QueryHealthReport {
                state: QueryHealthState::Healthy,
                last_error: None,
            }),
            last_error: None,
        };
        let telemetry = TelemetryHealthReport {
            state: TelemetryHealthState::Healthy,
            dropped_exports_total: 1,
            malformed_spans_total: 0,
            exporter_statuses: vec![ExporterHealth {
                name: SinkName::new("otlp").expect("valid sink name"),
                state: ExporterHealthState::Degraded,
                last_error: Some(DiagnosticSummary::from(&diagnostic())),
            }],
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };
        let report = ObservabilityHealthReport {
            state: ObservationHealthState::Degraded,
            dropped_observations_total: 2,
            subscriber_failures_total: 3,
            projection_failures_total: 4,
            logging: Some(logging),
            telemetry: Some(telemetry),
            last_error: Some(DiagnosticSummary::from(&diagnostic())),
        };

        let encoded = serde_json::to_string(&report).expect("serialize observability health");
        let decoded: ObservabilityHealthReport =
            serde_json::from_str(&encoded).expect("deserialize observability health");
        assert_eq!(decoded, report);
    }
}
