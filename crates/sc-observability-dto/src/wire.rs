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
    /// Wire trace.
    Trace,
    /// Wire debug.
    Debug,
    /// Wire info.
    Info,
    /// Wire warn.
    Warn,
    /// Wire error.
    Error,
}

/// Version-one LevelFilterDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelFilterDto {
    /// Wire off.
    Off,
    /// Wire error.
    Error,
    /// Wire warn.
    Warn,
    /// Wire info.
    Info,
    /// Wire debug.
    Debug,
    /// Wire trace.
    Trace,
}

/// Version-one LevelChangeSourceDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LevelChangeSourceDto {
    /// Wire application.
    Application,
    /// Wire user request.
    UserRequest,
    /// Wire diagnostic session.
    DiagnosticSession,
}

/// Version-one AvailabilityDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityDto {
    /// Wire healthy.
    Healthy,
    /// Wire degraded dropping.
    DegradedDropping,
    /// Wire unavailable.
    Unavailable,
}

/// Version-one WorkerStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkerStateDto {
    /// Wire running.
    Running,
    /// Wire degraded.
    Degraded,
    /// Wire stopped.
    Stopped,
}

/// Version-one QueryStateDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum QueryStateDto {
    /// Wire healthy.
    Healthy,
    /// Wire degraded.
    Degraded,
    /// Wire unavailable.
    Unavailable,
}

/// Version-one LifecycleDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDto {
    /// Wire running.
    Running,
    /// Wire stopping.
    Stopping,
    /// Wire stopped.
    Stopped,
    /// Wire failed.
    Failed,
}

/// Version-one LogOrderDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LogOrderDto {
    #[default]
    /// Wire oldest first.
    OldestFirst,
    /// Wire newest first.
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
    /// Wire utf8.
    Utf8 {
        /// Active variant value.
        value: String,
    },
    /// Wire unrepresentable.
    Unrepresentable,
    /// Wire absent.
    Absent,
}

/// Version-one ValueDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-input-strict" = true)))]
pub enum ValueDto {
    /// Wire null.
    Null {},
    /// Wire boolean.
    Boolean {
        /// Active variant value.
        value: bool,
    },
    /// Wire string.
    String {
        /// Active variant value.
        value: String,
    },
    /// Wire integer.
    Integer {
        /// Active variant value.
        value: DecimalDto,
    },
    /// Wire float.
    Float {
        /// Active variant value.
        value: f64,
    },
    /// Wire array.
    Array {
        /// Active variant value.
        value: Vec<ValueDto>,
    },
    /// Wire object.
    Object {
        /// Active variant value.
        value: BTreeMap<String, ValueDto>,
    },
}

/// Version-one RemediationDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationDto {
    /// Wire recoverable.
    Recoverable {
        /// Active variant steps.
        steps: Vec<String>,
    },
    /// Wire not recoverable.
    NotRecoverable {
        /// Active variant justification.
        justification: String,
    },
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

/// Version-one DispatchDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchDto {
    /// Wire scheduled.
    Scheduled,
}

/// Version-one AdmissionDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdmissionDto {
    /// Wire accepted.
    Accepted,
    /// Wire filtered.
    Filtered,
}

/// Version-one CompletionDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompletionDto {
    /// Wire completed.
    Completed,
}

/// Version-one ChangeDiagnosticDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeDiagnosticDto {
    /// Wire accepted.
    Accepted,
    /// Wire not accepted.
    NotAccepted {
        /// Active variant diagnostic.
        diagnostic: OperationDiagnosticDto,
    },
}

/// Version-one LevelChangeDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LevelChangeDto {
    /// Wire changed.
    Changed {
        /// Wire previous.
        previous: LevelStateDto,
        /// Wire current.
        current: LevelStateDto,
        /// Wire source.
        source: LevelChangeSourceDto,
        /// Wire diagnostic.
        diagnostic: ChangeDiagnosticDto,
    },
    /// Wire unchanged.
    Unchanged {
        /// Wire state.
        state: LevelStateDto,
    },
}

/// Version-one LevelRequestDto wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LevelRequestDto {
    /// Wire elevate.
    Elevate {
        /// Active variant level.
        level: LevelFilterDto,
    },
    /// Wire reset.
    Reset {},
}

/// Version-one Failure wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
    /// Wire validation.
    Validation {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire field.
        field: String,
    },
    /// Wire queue full.
    QueueFull {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire below baseline.
    BelowBaseline {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire requested.
        requested: LevelFilterDto,
        /// Wire configured.
        configured: LevelFilterDto,
    },
    /// Wire unsupported level.
    UnsupportedLevel {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire requested.
        requested: LevelFilterDto,
        /// Wire available.
        available: LevelFilterDto,
    },
    /// Wire permission denied.
    PermissionDenied {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire closed.
    Closed {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire unavailable.
    Unavailable {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire io.
    Io {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire timeout.
    Timeout {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire operation.
        operation: String,
    },
    /// Wire cancelled.
    Cancelled {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire operation.
        operation: String,
    },
    /// Wire unsupported version.
    UnsupportedVersion {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire received.
        received: u32,
    },
    /// Wire internal.
    Internal {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Wire unknown remote.
    UnknownRemote {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<Diagnostic>,
        /// Wire remote kind.
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
/// Version-one `ResultDto<T>` wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultDto<T> {
    /// Wire ok.
    Ok {
        /// Active variant value.
        value: T,
    },
    /// Wire error.
    Error {
        /// Active variant error.
        error: Failure,
    },
}

/// Version-one `WireEnvelope<T>` wire value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireEnvelope<T> {
    /// Wire ok.
    Ok {
        /// Active variant schema version.
        #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
        schema_version: u32,
        /// Active variant value.
        value: T,
    },
    /// Wire error.
    Error {
        /// Active variant schema version.
        #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
        schema_version: u32,
        /// Active variant error.
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
    /// Wire schema version.
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
    /// Wire schema version.
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
    /// Wire schema version.
    pub schema_version: u32,
}

/// Version-one FlushRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlushRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
    pub schema_version: u32,
    /// timeout ms.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = 60000)))]
    /// Wire timeout ms.
    pub timeout_ms: u32,
}

/// Version-one LevelChangeRequest wire record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelChangeRequest {
    /// schema version.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 1, max = 1)))]
    /// Wire schema version.
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
    /// Wire log.
    Log,
}
/// Operations returning final logging admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AdmissionOperationDto {
    /// Wire log.
    Log,
    /// Wire try log.
    TryLog,
}
/// Operations returning a completed client observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CompletionOperationDto {
    /// Wire query.
    Query,
    /// Wire health.
    Health,
    /// Wire flush.
    Flush,
}
/// Payload-free client outcome, distinct from host persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClientOutcome {
    /// Wire idle.
    Idle,
    /// Wire scheduled.
    Scheduled {
        /// Active variant operation.
        operation: LogOperationDto,
    },
    /// Wire accepted.
    Accepted {
        /// Active variant operation.
        operation: AdmissionOperationDto,
    },
    /// Wire filtered.
    Filtered {
        /// Active variant operation.
        operation: AdmissionOperationDto,
    },
    /// Wire completed.
    Completed {
        /// Active variant operation.
        operation: CompletionOperationDto,
    },
}
/// Bounded local client status; no ownership or IPC capability is represented.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
pub struct ClientStatus {
    /// Number of outstanding operations, bounded by client admission.
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = 256)))]
    /// Wire in flight.
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
    /// Wire validation.
    pub validation: DecimalDto,
    /// Saturating queue_full counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire queue full.
    pub queue_full: DecimalDto,
    /// Saturating below_baseline counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire below baseline.
    pub below_baseline: DecimalDto,
    /// Saturating unsupported_level counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire unsupported level.
    pub unsupported_level: DecimalDto,
    /// Saturating permission_denied counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire permission denied.
    pub permission_denied: DecimalDto,
    /// Saturating closed counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire closed.
    pub closed: DecimalDto,
    /// Saturating unavailable counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire unavailable.
    pub unavailable: DecimalDto,
    /// Saturating io counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire io.
    pub io: DecimalDto,
    /// Saturating timeout counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire timeout.
    pub timeout: DecimalDto,
    /// Saturating cancelled counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire cancelled.
    pub cancelled: DecimalDto,
    /// Saturating unsupported_version counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire unsupported version.
    pub unsupported_version: DecimalDto,
    /// Saturating internal counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire internal.
    pub internal: DecimalDto,
    /// Saturating unknown_remote counter.
    #[serde(deserialize_with = "unsigned_decimal")]
    #[cfg_attr(feature = "schema-gen", schemars(extend("x-sc-integer-domain" = "unsigned")))]
    /// Wire unknown remote.
    pub unknown_remote: DecimalDto,
}
