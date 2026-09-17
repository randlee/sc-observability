//! Explicit wire shapes; input entrypoints apply checked semantic validation.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical integer string: signed i64 or unsigned u64; counters additionally reject negatives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct DecimalDto(
    #[cfg_attr(
        feature = "schema-gen",
        schemars(regex(pattern = r"^(0|[1-9][0-9]*|-[1-9][0-9]*)$"))
    )]
    String,
);
impl DecimalDto {
    /// Checks canonical spelling and the shared JSON integer domain.
    pub fn new(value: impl Into<String>) -> std::result::Result<Self, String> {
        let value = value.into();
        let digits = value.strip_prefix('-').unwrap_or(&value);
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
            || value == "-0"
        {
            return Err("expected canonical decimal integer".into());
        }
        if value.starts_with('-') {
            value
                .parse::<i64>()
                .map_err(|_| "signed integer overflow")?;
        } else {
            value
                .parse::<u64>()
                .map_err(|_| "unsigned integer overflow")?;
        }
        Ok(Self(value))
    }
    /// Returns the canonical representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Checks the unsigned counter domain.
    pub fn as_u64(&self) -> std::result::Result<u64, String> {
        self.0
            .parse()
            .map_err(|_| "expected unsigned decimal".into())
    }
}
impl From<u64> for DecimalDto {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}
impl From<i64> for DecimalDto {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}
impl<'de> Deserialize<'de> for DecimalDto {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Version-one LevelDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelDto {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Version-one LevelFilterDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelFilterDto {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Version-one LevelChangeSourceDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelChangeSourceDto {
    Application,
    UserRequest,
    DiagnosticSession,
}

/// Version-one AvailabilityDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityDto {
    Healthy,
    DegradedDropping,
    Unavailable,
}

/// Version-one WorkerStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkerStateDto {
    Running,
    Degraded,
    Stopped,
}

/// Version-one QueryStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum QueryStateDto {
    Healthy,
    Degraded,
    Unavailable,
}

/// Version-one LifecycleDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDto {
    Running,
    Stopping,
    Stopped,
    Failed,
}

/// Version-one LogOrderDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LogOrderDto {
    #[default]
    OldestFirst,
    NewestFirst,
}

fn default_limit() -> usize {
    100
}
/// Version-one PathDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PathDto {
    Utf8 { value: String },
    Unrepresentable,
    Absent,
}

/// Version-one ValueDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-input-strict" = true)))]
pub enum ValueDto {
    Null {},
    Boolean { value: bool },
    String { value: String },
    Integer { value: DecimalDto },
    Float { value: f64 },
    Array { value: Vec<ValueDto> },
    Object { value: BTreeMap<String, ValueDto> },
}

/// Version-one RemediationDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationDto {
    Recoverable { steps: Vec<String> },
    NotRecoverable { justification: String },
}

/// Version-one TraceContextDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-input-strict" = true)))]
pub struct TraceContextDto {
    /// trace id.
    pub trace_id: String,
    /// span id.
    pub span_id: String,
    /// parent span id.
    pub parent_span_id: Option<String>,
}

/// Version-one ProcessIdentityDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct ProcessIdentityDto {
    /// hostname.
    pub hostname: Option<String>,
    /// pid.
    pub pid: Option<u32>,
}

/// Version-one StateTransitionDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StateTransitionDto {
    /// entity kind.
    pub entity_kind: String,
    /// entity id.
    pub entity_id: Option<String>,
    /// from state.
    pub from_state: String,
    /// to state.
    pub to_state: String,
    /// reason.
    pub reason: Option<String>,
    /// trigger.
    pub trigger: Option<String>,
}

/// Version-one StoredDiagnosticDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct StoredDiagnosticDto {
    /// timestamp.
    pub timestamp: String,
    /// code.
    pub code: String,
    /// message.
    pub message: String,
    /// cause.
    pub cause: Option<String>,
    /// remediation.
    pub remediation: RemediationDto,
    /// docs.
    pub docs: Option<String>,
    /// details.
    pub details: BTreeMap<String, ValueDto>,
}

/// Version-one LogEventDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LogEventDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
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
    pub schema_version: u32,
    /// service.
    pub service: Option<String>,
    /// levels.
    #[serde(default)]
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
    pub field_matches: Vec<FieldMatchDto>,
    /// limit.
    #[serde(default = "default_limit")]
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1000)))]
    pub limit: usize,
    /// order.
    #[serde(default)]
    pub order: LogOrderDto,
}

/// Version-one LogSnapshotDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LogSnapshotDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
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
    pub rotated_files_total: DecimalDto,
    /// pruned files total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
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
    pub dropped_events_total: DecimalDto,
    /// flush errors total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub flush_errors_total: DecimalDto,
    /// queue depth.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub queue_depth: DecimalDto,
    /// queue capacity.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub queue_capacity: DecimalDto,
    /// queue high water mark.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub queue_high_water_mark: DecimalDto,
    /// queue full drops total.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
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
    pub queue_full: DecimalDto,
    /// invalid event.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub invalid_event: DecimalDto,
    /// writer degraded.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub writer_degraded: DecimalDto,
    /// shutdown timed out.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub shutdown_timed_out: DecimalDto,
    /// not installed.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub not_installed: DecimalDto,
    /// logger panicked.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
    pub logger_panicked: DecimalDto,
    /// reentrant emit.
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    #[serde(deserialize_with = "unsigned_decimal")]
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
    pub level_revision: DecimalDto,
}

/// Version-one BridgeHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct BridgeHealthDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
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
    pub level_revision: DecimalDto,
}

/// Version-one LogHealthDto wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct LogHealthDto {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    /// logging.
    pub logging: LoggingHealthDto,
    /// bridge.
    pub bridge: Option<BridgeHealthDto>,
    /// level state.
    pub level_state: LevelStateDto,
}

/// Version-one DispatchDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchDto {
    Scheduled,
}

/// Version-one AdmissionDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdmissionDto {
    Accepted,
    Filtered,
}

/// Version-one CompletionDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompletionDto {
    Completed,
}

/// Version-one ChangeDiagnosticDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeDiagnosticDto {
    Accepted,
    NotAccepted { diagnostic: OperationDiagnosticDto },
}

/// Version-one LevelChangeDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LevelChangeDto {
    Changed {
        previous: LevelStateDto,
        current: LevelStateDto,
        source: LevelChangeSourceDto,
        diagnostic: ChangeDiagnosticDto,
    },
    Unchanged {
        state: LevelStateDto,
    },
}

/// Version-one LevelRequestDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LevelRequestDto {
    Elevate { level: LevelFilterDto },
    Reset {},
}

/// Version-one Failure wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
    Validation {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        field: String,
    },
    QueueFull {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    BelowBaseline {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        requested: LevelFilterDto,
        configured: LevelFilterDto,
    },
    UnsupportedLevel {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        requested: LevelFilterDto,
        available: LevelFilterDto,
    },
    PermissionDenied {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    Closed {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    Unavailable {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    Io {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    Timeout {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        operation: String,
    },
    Cancelled {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        operation: String,
    },
    UnsupportedVersion {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        received: u32,
    },
    Internal {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
    },
    UnknownRemote {
        #[serde(flatten)]
        diagnostic: Box<Diagnostic>,
        remote_kind: String,
    },
}

impl Failure {
    /// Returns the original diagnostic without parsing display text.
    pub fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::Validation { diagnostic, .. }
            | Self::QueueFull { diagnostic, .. }
            | Self::BelowBaseline { diagnostic, .. }
            | Self::UnsupportedLevel { diagnostic, .. }
            | Self::PermissionDenied { diagnostic, .. }
            | Self::Closed { diagnostic, .. }
            | Self::Unavailable { diagnostic, .. }
            | Self::Io { diagnostic, .. }
            | Self::Timeout { diagnostic, .. }
            | Self::Cancelled { diagnostic, .. }
            | Self::UnsupportedVersion { diagnostic, .. }
            | Self::Internal { diagnostic, .. }
            | Self::UnknownRemote { diagnostic, .. } => diagnostic,
        }
    }
}
/// Version-one ResultDto<T> wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultDto<T> {
    Ok { value: T },
    Error { error: Failure },
}

/// Version-one WireEnvelope<T> wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireEnvelope<T> {
    Ok {
        #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
        schema_version: u32,
        value: T,
    },
    Error {
        #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
        schema_version: u32,
        error: Failure,
    },
}

/// Version-one TryLogRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TryLogRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    /// event.
    pub event: LogEventDto,
}

/// Version-one QueryRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    /// query.
    pub query: LogQueryDto,
}

/// Version-one HealthRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HealthRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
}

/// Version-one FlushRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlushRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    /// timeout ms.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = 60000)))]
    pub timeout_ms: u32,
}

/// Version-one LevelChangeRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelChangeRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    /// change.
    pub change: LevelRequestDto,
}

fn unsigned_decimal<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<DecimalDto, D::Error> {
    let value = DecimalDto::deserialize(d)?;
    value.as_u64().map_err(serde::de::Error::custom)?;
    Ok(value)
}

/// The log-only operation in a scheduled client outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogOperationDto {
    Log,
}
/// Operations returning final logging admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AdmissionOperationDto {
    Log,
    TryLog,
}
/// Operations returning a completed client observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CompletionOperationDto {
    Query,
    Health,
    Flush,
}
/// Payload-free client outcome, distinct from host persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClientOutcome {
    Idle,
    Scheduled { operation: LogOperationDto },
    Accepted { operation: AdmissionOperationDto },
    Filtered { operation: AdmissionOperationDto },
    Completed { operation: CompletionOperationDto },
}
/// Bounded local client status; no ownership or IPC capability is represented.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct ClientStatus {
    /// Number of outstanding operations, bounded by client admission.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = 256)))]
    pub in_flight: u32,
    /// Saturating counters for every declared failure kind.
    pub failures_by_kind: FailureCountsDto,
    /// Most recent completion-order result.
    pub last_result: ResultDto<ClientOutcome>,
    /// Retained failure survives subsequent successful operations.
    pub last_failure: Option<Failure>,
}

/// Fixed bounded counters for the declared Failure union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct FailureCountsDto {
    /// Saturating validation counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub validation: DecimalDto,
    /// Saturating queue_full counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub queue_full: DecimalDto,
    /// Saturating below_baseline counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub below_baseline: DecimalDto,
    /// Saturating unsupported_level counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub unsupported_level: DecimalDto,
    /// Saturating permission_denied counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub permission_denied: DecimalDto,
    /// Saturating closed counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub closed: DecimalDto,
    /// Saturating unavailable counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub unavailable: DecimalDto,
    /// Saturating io counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub io: DecimalDto,
    /// Saturating timeout counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub timeout: DecimalDto,
    /// Saturating cancelled counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub cancelled: DecimalDto,
    /// Saturating unsupported_version counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub unsupported_version: DecimalDto,
    /// Saturating internal counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub internal: DecimalDto,
    /// Saturating unknown_remote counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    pub unknown_remote: DecimalDto,
}
