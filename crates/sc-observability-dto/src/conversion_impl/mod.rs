//! Checked semantic conversion; no runtime or transport dependency.
use crate::constants::{
    MAX_CONTAINER_DEPTH, MAX_DIAGNOSTIC_FIELD_BYTES, MAX_QUERY_LIMIT, MAX_REMEDIATION_STEPS,
    MAX_TIMEOUT_MS, MAX_WIRE_PAYLOAD_BYTES, WIRE_SCHEMA_VERSION,
};
use crate::error_codes;
use crate::{
    AdmissionDto, AvailabilityDto, CanonicalDiagnosticDto, CanonicalFailureDto,
    CanonicalWireEnvelope, ChangeDiagnosticDto, DecimalDtoError, Diagnostic, DiagnosticSummaryDto,
    Failure, LevelChangeDto, LevelChangeSourceDto, LevelDto, LevelFilterDto, LevelRequestDto,
    LevelStateDto, LogEventDto, LogHealthDto, LogOrderDto, LogQueryDto, LogSnapshotDto,
    LoggingHealthDto, MaintenanceHealthDto, PathDto, ProcessIdentityDto, QueryHealthDto,
    QueryStateDto, RemediationDto, SinkHealthDto, StateTransitionDto, StoredDiagnosticDto,
    StoredEventDto, TraceContextDto, ValueDto, WireEnvelope, WorkerStateDto,
};
use sc_observability_types as core;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::any::Any;
use std::collections::BTreeMap;

const UNREGISTERED_CODE_REMEDIATION: &str =
    "Inspect the diagnostic code and follow the emitting component's recovery guidance";

/// Host-selected values that cannot be supplied through an input DTO.
#[derive(Debug, Clone)]
pub struct EventStamp {
    /// Wire service.
    pub service: core::ServiceName,
    /// Wire timestamp.
    pub timestamp: core::Timestamp,
    /// Wire identity.
    pub identity: core::ProcessIdentity,
}
/// Constructs a boundary diagnostic using the sole binding-owned registry.
///
/// Native producers may retain codes outside that registry. Those diagnostics
/// receive an explicit recovery step rather than an empty recoverable payload.
pub fn boundary_diagnostic(code: &str, message: impl Into<String>) -> Diagnostic {
    let entry = error_codes::REGISTRY
        .iter()
        .find(|entry| entry.code == code);
    Diagnostic {
        at: core::Timestamp::now_utc().to_string(),
        code: code.into(),
        message: message.into(),
        remediation: RemediationDto::Recoverable {
            steps: entry
                .map(|entry| vec![entry.remediation.into()])
                .unwrap_or_else(|| vec![UNREGISTERED_CODE_REMEDIATION.into()]),
        },
    }
}
/// Reports a checked input failure with its offending field.
pub fn invalid_input(field: impl Into<String>, message: impl Into<String>) -> Failure {
    Failure::Validation {
        diagnostic: Box::new(boundary_diagnostic(
            error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
            message,
        )),
        field: field.into(),
    }
}
fn checked<T, E: std::fmt::Display + 'static>(
    value: std::result::Result<T, E>,
    field: &str,
) -> Result<T, Failure> {
    value.map_err(|error| {
        let source = &error as &dyn Any;
        let code = source
            .downcast_ref::<core::ValueValidationError>()
            .map(|error| error.code().as_str())
            .or_else(|| {
                source
                    .downcast_ref::<DecimalDtoError>()
                    .map(|error| error.code())
            });
        let message = format!("{}: {error}", std::any::type_name::<E>());
        let diagnostic = match code {
            Some(code) => boundary_diagnostic(code, message),
            None => {
                boundary_diagnostic(error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT, message)
            }
        };
        Failure::Validation {
            diagnostic: Box::new(diagnostic),
            field: field.into(),
        }
    })
}
fn version(version: u32) -> Result<(), Failure> {
    if version == WIRE_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(Failure::UnsupportedVersion {
            diagnostic: Box::new(boundary_diagnostic(
                error_codes::SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
                "unsupported schema version",
            )),
            received: version,
        })
    }
}
fn measure(value: &Value, depth: usize) -> Result<(), Failure> {
    match value {
        Value::Array(values) => {
            if depth >= MAX_CONTAINER_DEPTH {
                return Err(invalid_input(
                    "request",
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            for value in values {
                measure(value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            if depth >= MAX_CONTAINER_DEPTH {
                return Err(invalid_input(
                    "request",
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            for value in values.values() {
                measure(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn decode<T: DeserializeOwned>(value: Value, field: &str) -> Result<T, Failure> {
    measure(&value, 0)?;
    if checked(serde_json::to_vec(&value), field)?.len() > MAX_WIRE_PAYLOAD_BYTES {
        return Err(invalid_input(
            field,
            format!("request exceeds {MAX_WIRE_PAYLOAD_BYTES} UTF-8 bytes"),
        ));
    }
    checked(serde_json::from_value(value), field)
}
/// Structurally decodes an event DTO, checking keys, wire size/depth, and provenance.
/// [`to_core_event`] performs native constructor and event-bound validation.
pub fn decode_event(value: Value) -> Result<LogEventDto, Failure> {
    check_event_keys(&value)?;
    let dto: LogEventDto = decode(value, "event")?;
    check_input_provenance(&dto.fields, "fields")?;
    Ok(dto)
}
/// Structurally decodes a query DTO, checking nested keys, wire size, and depth.
/// [`to_core_query`] validates native constructors and query bounds.
pub fn decode_query(value: Value) -> Result<LogQueryDto, Failure> {
    if let Some(fields) = value.get("field_matches").and_then(Value::as_array) {
        for item in fields {
            if let Some(value) = item.get("value") {
                check_value_keys(value, "field_matches.value", 0)?;
            }
        }
    }
    decode(value, "query")
}
/// Decodes the owner-level request without granting an ownership capability.
pub fn decode_level_request(value: Value) -> Result<LevelRequestDto, Failure> {
    decode(value, "change")
}
/// Validates bounded integer milliseconds before scheduling a native operation.
pub fn decode_timeout(value: Value) -> Result<u32, Failure> {
    let timeout = value
        .as_u64()
        .filter(|v| *v <= u64::from(MAX_TIMEOUT_MS))
        .ok_or_else(|| {
            invalid_input(
                "timeout_ms",
                format!("expected integer milliseconds in 0..{MAX_TIMEOUT_MS}"),
            )
        })?;
    Ok(timeout as u32)
}
fn timestamp(value: String, field: &str) -> Result<core::Timestamp, Failure> {
    let ts: core::Timestamp = checked(serde_json::from_value(Value::String(value.clone())), field)?;
    if ts.to_string() != value {
        return Err(invalid_input(
            field,
            "expected canonical UTC RFC3339 timestamp",
        ));
    }
    Ok(ts)
}
/// Normalizes a user field key using the binding wire contract's rules.
pub fn normalize_field_key(value: &str) -> String {
    value
        .replace("::", ".")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
/// Returns whether a field key is reserved for trusted binding provenance.
pub fn is_protected_key(key: &str) -> bool {
    key.starts_with("sc_observability.binding.")
        || normalize_field_key(key).starts_with("sc_observability.binding.")
}
fn check_input_provenance(fields: &BTreeMap<String, ValueDto>, field: &str) -> Result<(), Failure> {
    for (key, value) in fields {
        let path = format!("{field}.{key}");
        if is_protected_key(key) {
            return Err(invalid_input(path, "reserved binding provenance field"));
        }
        match value {
            ValueDto::Array { value } => {
                for (index, value) in value.iter().enumerate() {
                    check_input_provenance_value(value, &format!("{path}[{index}]"))?;
                }
            }
            ValueDto::Object { value } => check_input_provenance(value, &path)?,
            _ => {}
        }
    }
    Ok(())
}
fn check_input_provenance_value(value: &ValueDto, field: &str) -> Result<(), Failure> {
    match value {
        ValueDto::Array { value } => {
            for (index, value) in value.iter().enumerate() {
                check_input_provenance_value(value, &format!("{field}[{index}]"))?;
            }
        }
        ValueDto::Object { value } => check_input_provenance(value, field)?,
        _ => {}
    }
    Ok(())
}
fn to_value(value: ValueDto, field: &str, protect: bool, depth: usize) -> Result<Value, Failure> {
    Ok(match value {
        ValueDto::Null {} => Value::Null,
        ValueDto::Boolean { value } => Value::Bool(value),
        ValueDto::String { value } => Value::String(value),
        ValueDto::Integer { value } => {
            if value.as_str().starts_with('-') {
                Value::from(checked(value.as_str().parse::<i64>(), field)?)
            } else {
                Value::from(checked(value.as_str().parse::<u64>(), field)?)
            }
        }
        ValueDto::Float { value } => Value::Number(
            serde_json::Number::from_f64(value)
                .ok_or_else(|| invalid_input(field, "float must be finite"))?,
        ),
        ValueDto::Array { value } => {
            if depth >= MAX_CONTAINER_DEPTH {
                return Err(invalid_input(
                    field,
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            Value::Array(
                value
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| to_value(v, &format!("{field}[{i}]"), protect, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        ValueDto::Object { value } => {
            if depth >= MAX_CONTAINER_DEPTH {
                return Err(invalid_input(
                    field,
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            Value::Object(
                value
                    .into_iter()
                    .map(|(k, v)| {
                        let path = format!("{field}.{k}");
                        if protect && is_protected_key(&k) {
                            return Err(invalid_input(path, "reserved binding provenance field"));
                        }
                        Ok((k, to_value(v, &path, protect, depth + 1)?))
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
    })
}
/// Encodes JSON values losslessly, distinguishing integer and float domains.
///
/// A non-integer `Number` that has no `f64` representation is rejected as
/// `invalid_input("value", "unrepresentable float")`. This workspace does not
/// enable `serde_json`'s `arbitrary_precision` feature, but Cargo feature
/// unification is additive: a downstream consumer that enables it makes
/// `Number` a decimal string, and an out-of-range literal such as `1e400`
/// then has no `f64` value. Keeping the projection fallible returns the
/// released validation failure for that untrusted input instead of panicking.
pub fn from_json_value(value: Value) -> Result<ValueDto, Failure> {
    project(value)
}

fn project(value: Value) -> Result<ValueDto, Failure> {
    Ok(match value {
        Value::Null => ValueDto::Null {},
        Value::Bool(value) => ValueDto::Boolean { value },
        Value::String(value) => ValueDto::String { value },
        Value::Number(value) => {
            if let Some(v) = value.as_i64() {
                ValueDto::Integer { value: v.into() }
            } else if let Some(v) = value.as_u64() {
                ValueDto::Integer { value: v.into() }
            } else {
                float_dto(value.as_f64())?
            }
        }
        Value::Array(value) => ValueDto::Array {
            value: value.into_iter().map(project).collect::<Result<_, _>>()?,
        },
        Value::Object(value) => ValueDto::Object {
            value: value
                .into_iter()
                .map(|(key, value)| Ok((key, project(value)?)))
                .collect::<Result<_, Failure>>()?,
        },
    })
}
fn float_dto(value: Option<f64>) -> Result<ValueDto, Failure> {
    value
        .map(|value| ValueDto::Float { value })
        .ok_or_else(|| invalid_input("value", "unrepresentable float"))
}
fn from_fields(fields: Map<String, Value>) -> Result<BTreeMap<String, ValueDto>, Failure> {
    fields
        .into_iter()
        .map(|(key, value)| Ok((key, project(value)?)))
        .collect()
}
fn trace(value: TraceContextDto) -> Result<core::TraceContext, Failure> {
    Ok(core::TraceContext {
        trace_id: checked(core::TraceId::new(value.trace_id), "trace.trace_id")?,
        span_id: checked(core::SpanId::new(value.span_id), "trace.span_id")?,
        parent_span_id: optional_checked(value.parent_span_id, "trace.parent_span_id", |value| {
            core::SpanId::new(value)
        })?,
    })
}
fn optional_checked<T, E: std::fmt::Display + 'static>(
    value: Option<String>,
    field: &str,
    convert: impl FnOnce(String) -> Result<T, E>,
) -> Result<Option<T>, Failure> {
    value
        .map(|value| checked(convert(value), field))
        .transpose()
}
/// Converts validated event input using host-selected identity and time.
pub fn to_core_event(dto: LogEventDto, stamp: EventStamp) -> Result<core::LogEvent, Failure> {
    version(dto.schema_version)?;
    let Value::Object(fields) =
        to_value(ValueDto::Object { value: dto.fields }, "fields", true, 0)?
    else {
        return Err(invalid_input("fields", "expected object"));
    };
    Ok(core::LogEvent {
        version: checked(
            core::SchemaVersion::new(core::constants::OBSERVATION_ENVELOPE_VERSION),
            "version",
        )?,
        timestamp: stamp.timestamp,
        service: stamp.service,
        identity: stamp.identity,
        level: dto.level.into(),
        target: checked(core::TargetCategory::new(dto.target), "target")?,
        action: checked(core::ActionName::new(dto.action), "action")?,
        message: dto.message,
        trace: dto.trace.map(trace).transpose()?,
        request_id: optional_checked(dto.request_id, "request_id", |value| {
            core::CorrelationId::new(value)
        })?,
        correlation_id: optional_checked(dto.correlation_id, "correlation_id", |value| {
            core::CorrelationId::new(value)
        })?,
        outcome: optional_checked(dto.outcome, "outcome", |value| {
            core::OutcomeLabel::new(value)
        })?,
        diagnostic: None,
        state_transition: None,
        fields,
    })
}
/// Converts a checked query; equal bounds remain inclusive.
pub fn to_core_query(dto: LogQueryDto) -> Result<core::LogQuery, Failure> {
    version(dto.schema_version)?;
    if !(1..=MAX_QUERY_LIMIT).contains(&dto.limit) {
        return Err(invalid_input(
            "limit",
            format!("query limit must be in 1..{MAX_QUERY_LIMIT}"),
        ));
    }
    let raw = checked(serde_json::to_value(&dto), "query")?;
    measure(&raw, 0)?;
    if checked(serde_json::to_vec(&dto), "query")?.len() > MAX_WIRE_PAYLOAD_BYTES {
        return Err(invalid_input(
            "query",
            format!("request exceeds {MAX_WIRE_PAYLOAD_BYTES} UTF-8 bytes"),
        ));
    }
    let query = core::LogQuery {
        service: optional_checked(dto.service, "service", |value| {
            core::ServiceName::new(value)
        })?,
        levels: dto.levels.into_iter().map(Into::into).collect(),
        target: optional_checked(dto.target, "target", |value| {
            core::TargetCategory::new(value)
        })?,
        action: optional_checked(dto.action, "action", core::ActionName::new)?,
        request_id: optional_checked(dto.request_id, "request_id", |value| {
            core::CorrelationId::new(value)
        })?,
        correlation_id: optional_checked(dto.correlation_id, "correlation_id", |value| {
            core::CorrelationId::new(value)
        })?,
        since: dto.since.map(|v| timestamp(v, "since")).transpose()?,
        until: dto.until.map(|v| timestamp(v, "until")).transpose()?,
        field_matches: dto
            .field_matches
            .into_iter()
            .map(|v| {
                Ok(core::LogFieldMatch {
                    field: v.field,
                    value: to_value(v.value, "field_matches.value", false, 0)?,
                })
            })
            .collect::<Result<_, Failure>>()?,
        limit: Some(dto.limit),
        order: match dto.order {
            LogOrderDto::OldestFirst => core::LogOrder::OldestFirst,
            LogOrderDto::NewestFirst => core::LogOrder::NewestFirst,
        },
    };
    if matches!((query.since,query.until),(Some(s),Some(u)) if s>u) {
        return Err(invalid_input(
            "since",
            "since must be less than or equal to until",
        ));
    }
    if query
        .field_matches
        .iter()
        .any(|f| f.field.trim().is_empty())
    {
        return Err(invalid_input(
            "field_matches.field",
            "field name must not be empty",
        ));
    }
    Ok(query)
}
macro_rules! enum_map {
    ($dto:ident, $native:ident, $($variant:ident),+ $(,)?)=>{
        impl From<core::$native> for $dto {fn from(value:core::$native)->Self {match value {$(core::$native::$variant=>Self::$variant),+}}}
        impl From<$dto> for core::$native {fn from(value:$dto)->Self {match value {$($dto::$variant=>Self::$variant),+}}}
    }
}
enum_map!(LevelDto, Level, Trace, Debug, Info, Warn, Error);
enum_map!(
    LevelFilterDto,
    LevelFilter,
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace
);
enum_map!(
    LevelChangeSourceDto,
    LevelChangeSource,
    Application,
    UserRequest,
    DiagnosticSession
);
enum_map!(AdmissionDto, AdmissionOutcome, Accepted, Filtered);
enum_map!(
    AvailabilityDto,
    LoggingHealthState,
    Healthy,
    DegradedDropping,
    Unavailable
);
enum_map!(
    AvailabilityDto,
    SinkHealthState,
    Healthy,
    DegradedDropping,
    Unavailable
);
enum_map!(WorkerStateDto, WriterState, Running, Degraded, Stopped);
enum_map!(
    WorkerStateDto,
    MaintenanceWorkerState,
    Running,
    Degraded,
    Stopped
);
enum_map!(
    QueryStateDto,
    QueryHealthState,
    Healthy,
    Degraded,
    Unavailable
);
impl From<core::Remediation> for RemediationDto {
    fn from(value: core::Remediation) -> Self {
        match value {
            core::Remediation::Recoverable { steps } => Self::Recoverable {
                steps: steps.steps().to_vec(),
            },
            core::Remediation::NotRecoverable { justification } => {
                Self::NotRecoverable { justification }
            }
        }
    }
}
impl From<core::LevelState> for LevelStateDto {
    fn from(v: core::LevelState) -> Self {
        Self {
            configured_level: v.configured_level.into(),
            effective_level: v.effective_level.into(),
            level_revision: v.revision.into(),
        }
    }
}
impl From<core::DiagnosticSummary> for DiagnosticSummaryDto {
    fn from(v: core::DiagnosticSummary) -> Self {
        Self {
            code: v.code.map(|v| v.as_str().into()),
            message: v.message,
            at: v.at.to_string(),
        }
    }
}
impl From<core::TraceContext> for TraceContextDto {
    fn from(v: core::TraceContext) -> Self {
        Self {
            trace_id: v.trace_id.as_str().into(),
            span_id: v.span_id.as_str().into(),
            parent_span_id: v.parent_span_id.map(|v| v.as_str().into()),
        }
    }
}
impl From<core::OperationDiagnostic> for Diagnostic {
    fn from(v: core::OperationDiagnostic) -> Self {
        Self {
            at: v.at.to_string(),
            code: v.code.as_str().into(),
            message: v.message,
            remediation: v.remediation.into(),
        }
    }
}
/// Checks diagnostic bounds without truncating original data.
pub fn validate_diagnostic(value: &Diagnostic, field: &str) -> Result<(), Failure> {
    let oversized = value.at.len() > MAX_DIAGNOSTIC_FIELD_BYTES
        || value.code.len() > MAX_DIAGNOSTIC_FIELD_BYTES
        || value.message.len() > MAX_DIAGNOSTIC_FIELD_BYTES
        || match &value.remediation {
            RemediationDto::Recoverable { steps } => {
                steps.len() > MAX_REMEDIATION_STEPS
                    || steps.iter().any(|s| s.len() > MAX_DIAGNOSTIC_FIELD_BYTES)
            }
            RemediationDto::NotRecoverable { justification } => {
                justification.len() > MAX_DIAGNOSTIC_FIELD_BYTES
            }
        };
    if oversized {
        return Err(Failure::Validation {
            diagnostic: Box::new(boundary_diagnostic(
                error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
                "diagnostic exceeds documented bounds",
            )),
            field: field.into(),
        });
    }
    timestamp(value.at.clone(), field)?;
    Ok(())
}
/// Preserves complete stored event payloads, including trusted output provenance.
pub fn from_core_event(v: core::LogEvent) -> Result<StoredEventDto, Failure> {
    Ok(StoredEventDto {
        version: v.version.as_str().into(),
        timestamp: v.timestamp.to_string(),
        level: v.level.into(),
        service: v.service.as_str().into(),
        target: v.target.as_str().into(),
        action: v.action.as_str().into(),
        message: v.message,
        identity: ProcessIdentityDto {
            hostname: v.identity.hostname,
            pid: v.identity.pid,
        },
        trace: v.trace.map(Into::into),
        request_id: v.request_id.map(|v| v.as_str().into()),
        correlation_id: v.correlation_id.map(|v| v.as_str().into()),
        outcome: v.outcome.map(|v| v.as_str().into()),
        fields: from_fields(v.fields)?,
        diagnostic: v
            .diagnostic
            .map(|d| {
                Ok(StoredDiagnosticDto {
                    timestamp: d.timestamp.to_string(),
                    code: d.code.as_str().into(),
                    message: d.message,
                    cause: d.cause,
                    remediation: d.remediation.into(),
                    docs: d.docs,
                    details: from_fields(d.details)?,
                })
            })
            .transpose()?,
        state_transition: v.state_transition.map(|v| StateTransitionDto {
            entity_kind: v.entity_kind.as_str().into(),
            entity_id: v.entity_id,
            from_state: v.from_state.as_str().into(),
            to_state: v.to_state.as_str().into(),
            reason: v.reason,
            trigger: v.trigger.map(|v| v.as_str().into()),
        }),
    })
}
/// Converts a stored snapshot without imposing the input request-size limit.
pub fn from_core_snapshot(v: core::LogSnapshot) -> Result<LogSnapshotDto, Failure> {
    Ok(LogSnapshotDto {
        schema_version: WIRE_SCHEMA_VERSION,
        events: v
            .events
            .into_iter()
            .map(from_core_event)
            .collect::<Result<_, _>>()?,
        truncated: v.truncated,
    })
}
/// Represents absent or non-UTF8 paths without lossy replacement.
pub fn from_path(path: Option<&std::path::Path>) -> PathDto {
    match path {
        None => PathDto::Absent,
        Some(path) => match path.to_str() {
            Some(value) => PathDto::Utf8 {
                value: value.into(),
            },
            None => PathDto::Unrepresentable,
        },
    }
}
/// Validates a user path and resolves it against the host startup directory.
pub fn to_path(value: &str, startup: &std::path::Path) -> Result<std::path::PathBuf, Failure> {
    if value.is_empty() || value.contains('\0') {
        return Err(invalid_input("path", "path must be nonempty and NUL-free"));
    }
    let path = std::path::Path::new(value);
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        startup.join(path)
    })
}
/// Projects every logging health field from a single captured native snapshot.
pub fn from_logging_health(v: core::LoggingHealthReport) -> LoggingHealthDto {
    LoggingHealthDto {
        state: v.state.into(),
        dropped_events_total: v.dropped_events_total.into(),
        flush_errors_total: v.flush_errors_total.into(),
        active_log_path: from_path(Some(&v.active_log_path)),
        sink_statuses: v
            .sink_statuses
            .into_iter()
            .map(|v| SinkHealthDto {
                name: v.name.as_str().into(),
                state: v.state.into(),
                last_error: v.last_error.map(Into::into),
            })
            .collect(),
        queue_depth: v.queue_depth.into(),
        queue_capacity: v.queue_capacity.into(),
        queue_high_water_mark: v.queue_high_water_mark.into(),
        queue_full_drops_total: v.queue_full_drops_total.into(),
        writer_state: v.writer_state.into(),
        last_writer_error: v.last_writer_error.map(Into::into),
        query: v.query.map(|v| QueryHealthDto {
            state: v.state.into(),
            last_error: v.last_error.map(Into::into),
        }),
        maintenance: v.maintenance.map(|v| MaintenanceHealthDto {
            state: v.state.into(),
            last_pass_at: v.last_pass_at.map(|v| v.to_string()),
            rotated_files_total: v.rotated_files_total.as_u64().into(),
            pruned_files_total: v.pruned_files_total.as_u64().into(),
            last_error: v.last_error.map(Into::into),
        }),
        last_error: v.last_error.map(Into::into),
    }
}
/// Projects an independent core logger without inventing bridge state.
pub fn from_canonical_core_health(
    value: core::LoggingHealthReport,
    level: core::LevelState,
) -> LogHealthDto {
    LogHealthDto {
        schema_version: WIRE_SCHEMA_VERSION,
        logging: from_logging_health(value),
        bridge: None,
        level_state: level.into(),
    }
}
/// Preserves the committed level change and diagnostic admission outcome.
pub fn from_level_change(value: core::LevelChange) -> Result<LevelChangeDto, Failure> {
    Ok(match value {
        core::LevelChange::Unchanged { state } => LevelChangeDto::Unchanged {
            state: state.into(),
        },
        core::LevelChange::Changed {
            previous,
            current,
            source,
            diagnostic,
        } => LevelChangeDto::Changed {
            previous: previous.into(),
            current: current.into(),
            source: source.into(),
            diagnostic: match diagnostic {
                core::ChangeDiagnostic::Accepted => ChangeDiagnosticDto::Accepted,
                core::ChangeDiagnostic::NotAccepted { diagnostic } => {
                    let diagnostic = diagnostic.into();
                    validate_diagnostic(&diagnostic, "diagnostic")?;
                    ChangeDiagnosticDto::NotAccepted { diagnostic }
                }
            },
        },
    })
}
/// Preserves native typed level errors and the original unavailable diagnostic.
pub fn from_level_error(value: core::LevelChangeError) -> Failure {
    if let core::LevelChangeError::Unavailable { diagnostic } = value {
        return Failure::Unavailable {
            diagnostic: Box::new(diagnostic.into()),
        };
    }
    let diagnostic = Box::new(Diagnostic {
        at: core::Timestamp::now_utc().to_string(),
        code: value.code().as_str().into(),
        message: value.to_string(),
        remediation: value.remediation().into(),
    });
    match value {
        core::LevelChangeError::Stopping | core::LevelChangeError::Stopped => {
            Failure::Closed { diagnostic }
        }
        core::LevelChangeError::BelowBaseline {
            requested,
            configured,
        } => Failure::BelowBaseline {
            diagnostic,
            requested: requested.into(),
            configured: configured.into(),
        },
        core::LevelChangeError::UnsupportedLevel {
            requested,
            available,
        } => Failure::UnsupportedLevel {
            diagnostic,
            requested: requested.into(),
            available: available.into(),
        },
        core::LevelChangeError::Unavailable { diagnostic } => Failure::Unavailable {
            diagnostic: Box::new(diagnostic.into()),
        },
    }
}
/// Decodes the shared output envelope after its diagnostic representation is selected.
///
/// The two public envelope entrypoints share structural and failure-kind handling,
/// while each keeps its own diagnostic parser and semantic validation boundary.
fn decode_envelope_shell<T: DeserializeOwned, D, P>(
    value: Value,
    preparse_diagnostic: impl Fn(Value) -> Result<P, Failure>,
    validate_diagnostic: impl Fn(&P) -> Result<(), Failure>,
    parse_failure: impl Fn(Value) -> Result<Failure<D>, Failure>,
    unknown_failure: impl Fn(Value, P, &str) -> Result<Failure<D>, Failure>,
    validate_failure: impl Fn(&Failure<D>) -> Result<(), Failure>,
) -> Result<WireEnvelope<T, D>, Failure> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_input("response", "expected envelope object"))?;
    let raw_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| invalid_input("response", "invalid schema version"))?;
    version(raw_version)?;
    match object.get("kind").and_then(Value::as_str) {
        Some("ok") => {
            if object.contains_key("error") {
                return Err(invalid_input("response", "conflicting envelope payload"));
            }
            let value = object
                .get("value")
                .ok_or_else(|| invalid_input("response", "missing value"))?
                .clone();
            Ok(WireEnvelope::Ok {
                schema_version: WIRE_SCHEMA_VERSION,
                value: checked(serde_json::from_value(value), "response")?,
            })
        }
        Some("error") => {
            if object.contains_key("value") {
                return Err(invalid_input("response", "conflicting envelope payload"));
            }
            let raw = object
                .get("error")
                .ok_or_else(|| invalid_input("response", "missing error"))?
                .clone();
            let diagnostic = preparse_diagnostic(raw.clone())?;
            validate_diagnostic(&diagnostic)?;
            let tag = raw
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_input("response", "missing failure kind"))?
                .to_owned();
            let error = if Failure::<D>::KNOWN_KINDS.contains(&tag.as_str()) {
                parse_failure(raw)?
            } else {
                unknown_failure(raw, diagnostic, &tag)?
            };
            validate_failure(&error)?;
            Ok(WireEnvelope::Error {
                schema_version: WIRE_SCHEMA_VERSION,
                error,
            })
        }
        _ => Err(invalid_input("response", "invalid result kind")),
    }
}

/// Decodes an additive output envelope; an unknown remote failure retains its code and tag.
pub fn decode_envelope<T: DeserializeOwned>(value: Value) -> Result<WireEnvelope<T>, Failure> {
    decode_envelope_shell(
        value,
        |value| checked(serde_json::from_value(value), "response"),
        |diagnostic: &Diagnostic| validate_diagnostic(diagnostic, "response.error"),
        |value| checked(serde_json::from_value(value), "response"),
        |_, diagnostic, tag| {
            Ok(Failure::UnknownRemote {
                diagnostic: Box::new(diagnostic),
                remote_kind: tag.into(),
            })
        },
        |_| Ok(()),
    )
}

fn strict_keys(value: &Value, allowed: &[&str], field: &str) -> Result<(), Failure> {
    if let Some(object) = value.as_object()
        && let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(invalid_input(
            format!("{field}.{key}"),
            "unknown input field",
        ));
    }
    Ok(())
}
fn check_value_keys(value: &Value, field: &str, depth: usize) -> Result<(), Failure> {
    if depth > MAX_CONTAINER_DEPTH {
        return Err(invalid_input(
            field,
            format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
        ));
    }
    let tag = value.get("kind").and_then(Value::as_str);
    strict_keys(
        value,
        if tag == Some("null") {
            &["kind"]
        } else {
            &["kind", "value"]
        },
        field,
    )?;
    match tag {
        Some("array") => {
            if let Some(values) = value.get("value").and_then(Value::as_array) {
                for (i, value) in values.iter().enumerate() {
                    check_value_keys(value, &format!("{field}[{i}]"), depth + 1)?;
                }
            }
        }
        Some("object") => {
            if let Some(values) = value.get("value").and_then(Value::as_object) {
                for (key, value) in values {
                    check_value_keys(value, &format!("{field}.{key}"), depth + 1)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn check_event_keys(value: &Value) -> Result<(), Failure> {
    if let Some(trace) = value.get("trace") {
        strict_keys(trace, &["trace_id", "span_id", "parent_span_id"], "trace")?;
    }
    if let Some(fields) = value.get("fields").and_then(Value::as_object) {
        for (key, value) in fields {
            check_value_keys(value, &format!("fields.{key}"), 0)?;
        }
    }
    Ok(())
}

mod canonical;
mod envelope;

pub use canonical::{
    failure_from_classification, failure_from_diagnostic, from_canonical_diagnostic,
};
pub use envelope::decode_canonical_envelope;

#[cfg(test)]
mod number_projection_tests {
    use super::*;

    #[test]
    fn float_without_an_f64_value_is_the_released_validation_failure() {
        let Err(Failure::Validation { diagnostic, field }) = float_dto(None) else {
            panic!("a missing f64 must be a validation failure");
        };
        let Failure::Validation {
            diagnostic: expected,
            field: expected_field,
        } = invalid_input("value", "unrepresentable float")
        else {
            unreachable!("invalid_input is a validation failure");
        };
        assert_eq!(field, expected_field);
        assert_eq!(
            Diagnostic {
                at: expected.at.clone(),
                ..*diagnostic
            },
            *expected
        );
    }

    #[test]
    fn float_with_an_f64_value_is_a_float() {
        assert_eq!(float_dto(Some(1.5)), Ok(ValueDto::Float { value: 1.5 }));
    }
}
