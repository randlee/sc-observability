//! Health and status wire records.
use super::events::DiagnosticSummaryDto;
use super::primitives::{
    AvailabilityDto, DecimalDto, LevelFilterDto, LifecycleDto, PathDto, QueryStateDto,
    WorkerStateDto, unsigned_decimal,
};
use serde::{Deserialize, Serialize};

/// Version-one SinkHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct SinkHealthDto {
    /// name.
    pub name: String,
    /// state.
    pub state: AvailabilityDto,
    /// last error.
    pub last_error: Option<DiagnosticSummaryDto>,
}

/// Version-one QueryHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct QueryHealthDto {
    /// state.
    pub state: QueryStateDto,
    /// last error.
    pub last_error: Option<DiagnosticSummaryDto>,
}

/// Version-one MaintenanceHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct MaintenanceHealthDto {
    /// state.
    pub state: WorkerStateDto,
    /// last pass at.
    pub last_pass_at: Option<String>,
    /// rotated files total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire rotated files total.
    pub rotated_files_total: DecimalDto,
    /// pruned files total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire pruned files total.
    pub pruned_files_total: DecimalDto,
    /// last error.
    pub last_error: Option<DiagnosticSummaryDto>,
}

/// Version-one LoggingHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LoggingHealthDto {
    /// state.
    pub state: AvailabilityDto,
    /// dropped events total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire dropped events total.
    pub dropped_events_total: DecimalDto,
    /// flush errors total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire flush errors total.
    pub flush_errors_total: DecimalDto,
    /// queue depth.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire queue depth.
    pub queue_depth: DecimalDto,
    /// queue capacity.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire queue capacity.
    pub queue_capacity: DecimalDto,
    /// queue high water mark.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire queue high water mark.
    pub queue_high_water_mark: DecimalDto,
    /// queue full drops total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire queue full drops total.
    pub queue_full_drops_total: DecimalDto,
    /// active log path.
    pub active_log_path: PathDto,
    /// sink statuses.
    pub sink_statuses: Vec<SinkHealthDto>,
    /// writer state.
    pub writer_state: WorkerStateDto,
    /// last writer error.
    pub last_writer_error: Option<DiagnosticSummaryDto>,
    /// query.
    pub query: Option<QueryHealthDto>,
    /// maintenance.
    pub maintenance: Option<MaintenanceHealthDto>,
    /// last error.
    pub last_error: Option<DiagnosticSummaryDto>,
}

/// Version-one DropCountsDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct DropCountsDto {
    /// queue full.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire queue full.
    pub queue_full: DecimalDto,
    /// invalid event.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire invalid event.
    pub invalid_event: DecimalDto,
    /// writer degraded.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire writer degraded.
    pub writer_degraded: DecimalDto,
    /// shutdown timed out.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire shutdown timed out.
    pub shutdown_timed_out: DecimalDto,
    /// not installed.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire not installed.
    pub not_installed: DecimalDto,
    /// logger panicked.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire logger panicked.
    pub logger_panicked: DecimalDto,
    /// reentrant emit.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire reentrant emit.
    pub reentrant_emit: DecimalDto,
}

/// Version-one LevelStateDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LevelStateDto {
    /// configured level.
    pub configured_level: LevelFilterDto,
    /// effective level.
    pub effective_level: LevelFilterDto,
    /// level revision.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire level revision.
    pub level_revision: DecimalDto,
}

/// Version-one BridgeHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct BridgeHealthDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// logging.
    pub logging: LoggingHealthDto,
    /// dropped.
    pub dropped: DropCountsDto,
    /// lifecycle.
    pub lifecycle: LifecycleDto,
    /// active log path.
    pub active_log_path: PathDto,
    /// configured level.
    pub configured_level: LevelFilterDto,
    /// effective level.
    pub effective_level: LevelFilterDto,
    /// level revision.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    /// Wire level revision.
    pub level_revision: DecimalDto,
}

/// Version-one LogHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LogHealthDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// logging.
    pub logging: LoggingHealthDto,
    /// bridge.
    pub bridge: Option<BridgeHealthDto>,
    /// level state.
    pub level_state: LevelStateDto,
}
