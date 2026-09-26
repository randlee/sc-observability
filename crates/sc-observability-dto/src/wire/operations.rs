//! Operation, result, request, and client-status wire records.
use super::events::{Diagnostic, LogEventDto, LogQueryDto, OperationDiagnosticDto};
use super::health::LevelStateDto;
use super::primitives::{DecimalDto, LevelChangeSourceDto, LevelFilterDto, unsigned_decimal};
use serde::{Deserialize, Serialize};

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
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = crate::constants::MAX_TIMEOUT_MS)))]
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
    #[cfg_attr(feature = "schema-gen", schemars(range(min = 0, max = crate::constants::MAX_CLIENT_IN_FLIGHT)))]
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

/// Additive operational failures preserving canonical diagnostic metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonicalFailureDto {
    /// Wire validation.
    Validation {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire field.
        field: String,
    },
    /// Wire queue full.
    QueueFull {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire below baseline.
    BelowBaseline {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire requested.
        requested: LevelFilterDto,
        /// Wire configured.
        configured: LevelFilterDto,
    },
    /// Wire unsupported level.
    UnsupportedLevel {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire requested.
        requested: LevelFilterDto,
        /// Wire available.
        available: LevelFilterDto,
    },
    /// Wire permission denied.
    PermissionDenied {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire closed.
    Closed {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire unavailable.
    Unavailable {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire io.
    Io {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire timeout.
    Timeout {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire operation.
        operation: String,
    },
    /// Wire cancelled.
    Cancelled {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire operation.
        operation: String,
    },
    /// Wire unsupported version.
    UnsupportedVersion {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire received.
        received: u32,
    },
    /// Wire internal.
    Internal {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
    },
    /// Wire unknown remote.
    UnknownRemote {
        #[serde(flatten)]
        /// Wire diagnostic.
        diagnostic: Box<super::events::CanonicalDiagnosticDto>,
        /// Wire remote kind.
        remote_kind: String,
    },
}

impl CanonicalFailureDto {
    /// Returns the original diagnostic without parsing display text.
    pub fn diagnostic(&self) -> &super::events::CanonicalDiagnosticDto {
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
/// Staged envelope with unchanged schema/version/discriminants and richer diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-gen", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonicalWireEnvelope<T> {
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
        error: CanonicalFailureDto,
    },
}
