//! Event, query, and diagnostic wire records.
use super::primitives::{
    LevelDto, LogOrderDto, ProcessIdentityDto, RemediationDto, StateTransitionDto,
    StoredDiagnosticDto, TraceContextDto, ValueDto, default_limit,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version-one LogEventDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LogEventDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// level.
    pub level: LevelDto,
    /// target.
    pub target: String,
    /// action.
    pub action: String,
    /// message.
    pub message: Option<String>,
    /// trace.
    pub trace: Option<TraceContextDto>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// outcome.
    pub outcome: Option<String>,
    /// fields.
    #[serde(default)]
    /// Wire fields.
    pub fields: BTreeMap<String, ValueDto>,
}

/// Version-one StoredEventDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StoredEventDto {
    /// version.
    pub version: String,
    /// timestamp.
    pub timestamp: String,
    /// service.
    pub service: String,
    /// identity.
    pub identity: ProcessIdentityDto,
    /// level.
    pub level: LevelDto,
    /// target.
    pub target: String,
    /// action.
    pub action: String,
    /// message.
    pub message: Option<String>,
    /// trace.
    pub trace: Option<TraceContextDto>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// outcome.
    pub outcome: Option<String>,
    /// fields.
    pub fields: BTreeMap<String, ValueDto>,
    /// diagnostic.
    pub diagnostic: Option<StoredDiagnosticDto>,
    /// state transition.
    pub state_transition: Option<StateTransitionDto>,
}

/// Version-one FieldMatchDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FieldMatchDto {
    /// field.
    pub field: String,
    /// value.
    pub value: ValueDto,
}

/// Version-one LogQueryDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LogQueryDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// service.
    pub service: Option<String>,
    /// levels.
    #[serde(default)]
    /// Wire levels.
    pub levels: Vec<LevelDto>,
    /// target.
    pub target: Option<String>,
    /// action.
    pub action: Option<String>,
    /// request id.
    pub request_id: Option<String>,
    /// correlation id.
    pub correlation_id: Option<String>,
    /// since.
    pub since: Option<String>,
    /// until.
    pub until: Option<String>,
    /// field matches.
    #[serde(default)]
    /// Wire field matches.
    pub field_matches: Vec<FieldMatchDto>,
    /// limit.
    #[serde(default = "default_limit")]
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1000)))]
    /// Wire limit.
    pub limit: usize,
    /// order.
    #[serde(default)]
    /// Wire order.
    pub order: LogOrderDto,
}

/// Version-one LogSnapshotDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LogSnapshotDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// events.
    pub events: Vec<StoredEventDto>,
    /// truncated.
    pub truncated: bool,
}

/// Version-one DiagnosticSummaryDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct DiagnosticSummaryDto {
    /// code.
    pub code: Option<String>,
    /// message.
    pub message: String,
    /// at.
    pub at: String,
}

/// Version-one Diagnostic wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct Diagnostic {
    /// at.
    pub at: String,
    /// code.
    pub code: String,
    /// message.
    pub message: String,
    /// remediation.
    pub remediation: RemediationDto,
}

/// Diagnostic retained by an unsuccessful transition event.
pub type OperationDiagnosticDto = Diagnostic;
