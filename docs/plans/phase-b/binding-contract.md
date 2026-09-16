---
status: proposed_for_public_api_review
owners: B.3 shared DTO and Tauri, B.4 Python projection
---

# Shared binding declarations and boundary rules

This appendix supplies the complete new wire data shapes used by B.3/B.4.
The Result, Failure, RemediationDto, ClientStatus, ClientOutcome, LevelStateDto, LevelChangeDto,
LevelRequestDto, DiagnosticSummaryDto and OperationDiagnosticDto declarations
in [B.3](sprint-b-3-typescript.md#contract-and-operation-signatures) are
incorporated here without a second definition. All other wire declarations are
below. These are new DTOs, not edits to published core structs or their Serde.
Python exposes equivalent frozen dataclasses/tagged unions; wire integer strings
become checked Python ints in ergonomic values. New DTO Rust types live only in
sc-observability-dto, use the same names/fields and explicit Serde tags, and derive
Debug, Clone, PartialEq, Serialize, Deserialize. A property typed `T | null` maps
to Option<T>; non-nullable fields are required. TS number is u32 for pid, schema_version, bounded timeouts and in_flight,
usize for validated limit, and f64 only for tagged float values.
Other counters/integers use the decimal-string newtype described below.

## Complete wire declarations

```ts
export type LevelDto = "trace" | "debug" | "info" | "warn" | "error";
export type DecimalDto = string; // validated canonical decimal, never JSON number
export type PathDto =
  | { kind: "utf8"; value: string }
  | { kind: "unrepresentable" }
  | { kind: "absent" };
export type ValueDto =
  | { kind: "null" }
  | { kind: "boolean"; value: boolean }
  | { kind: "string"; value: string }
  | { kind: "integer"; value: DecimalDto }
  | { kind: "float"; value: number }
  | { kind: "array"; value: ValueDto[] }
  | { kind: "object"; value: Record<string, ValueDto> };
export interface TraceContextDto {
  trace_id: string;
  span_id: string;
  parent_span_id: string | null;
}
export interface ProcessIdentityDto { hostname: string | null; pid: number | null }
export interface StateTransitionDto {
  entity_kind: string;
  entity_id: string | null;
  from_state: string;
  to_state: string;
  reason: string | null;
  trigger: string | null;
}
export interface StoredDiagnosticDto {
  timestamp: string;
  code: string;
  message: string;
  cause: string | null;
  remediation: RemediationDto;
  docs: string | null;
  details: Record<string, ValueDto>;
}
export interface LogEventDto {
  schema_version: 1;
  level: LevelDto;
  target: string;
  action: string;
  message: string | null;
  trace: TraceContextDto | null;
  request_id: string | null;
  correlation_id: string | null;
  outcome: string | null;
  fields: Record<string, ValueDto>;
}
export interface StoredEventDto {
  version: string;
  timestamp: string;
  level: LevelDto;
  service: string;
  target: string;
  action: string;
  message: string | null;
  identity: ProcessIdentityDto;
  trace: TraceContextDto | null;
  request_id: string | null;
  correlation_id: string | null;
  outcome: string | null;
  diagnostic: StoredDiagnosticDto | null;
  state_transition: StateTransitionDto | null;
  fields: Record<string, ValueDto>;
}
export interface LogQueryDto {
  schema_version: 1;
  service: string | null;
  levels: LevelDto[];
  target: string | null;
  action: string | null;
  request_id: string | null;
  correlation_id: string | null;
  since: string | null;
  until: string | null;
  field_matches: { field: string; value: ValueDto }[];
  limit: number;
  order: "oldest_first" | "newest_first";
}
export interface LogSnapshotDto {
  schema_version: 1;
  events: StoredEventDto[];
  truncated: boolean;
}
export type AvailabilityDto = "healthy" | "degraded_dropping" | "unavailable";
export type WorkerStateDto = "running" | "degraded" | "stopped";
export interface SinkHealthDto {
  name: string;
  state: AvailabilityDto;
  last_error: DiagnosticSummaryDto | null;
}
export interface QueryHealthDto {
  state: "healthy" | "degraded" | "unavailable";
  last_error: DiagnosticSummaryDto | null;
}
export interface MaintenanceHealthDto {
  state: WorkerStateDto;
  last_pass_at: string | null;
  rotated_files_total: DecimalDto;
  pruned_files_total: DecimalDto;
  last_error: DiagnosticSummaryDto | null;
}
export interface LoggingHealthDto {
  state: AvailabilityDto;
  dropped_events_total: DecimalDto;
  flush_errors_total: DecimalDto;
  active_log_path: PathDto;
  sink_statuses: SinkHealthDto[];
  queue_depth: DecimalDto;
  queue_capacity: DecimalDto;
  queue_high_water_mark: DecimalDto;
  queue_full_drops_total: DecimalDto;
  writer_state: WorkerStateDto;
  last_writer_error: DiagnosticSummaryDto | null;
  query: QueryHealthDto | null;
  maintenance: MaintenanceHealthDto | null;
  last_error: DiagnosticSummaryDto | null;
}
export interface DropCountsDto {
  queue_full: DecimalDto;
  invalid_event: DecimalDto;
  writer_degraded: DecimalDto;
  shutdown_timed_out: DecimalDto;
  not_installed: DecimalDto;
  logger_panicked: DecimalDto;
  reentrant_emit: DecimalDto;
}
export interface BridgeHealthDto {
  schema_version: 1;
  logging: LoggingHealthDto;
  dropped: DropCountsDto;
  lifecycle: "running" | "stopping" | "stopped" | "failed";
  active_log_path: PathDto;
  configured_level: LevelFilterDto;
  effective_level: LevelFilterDto;
  level_revision: DecimalDto;
}
export interface LogHealthDto {
  schema_version: 1;
  logging: LoggingHealthDto;
  bridge: BridgeHealthDto | null;
  level_state: LevelStateDto;
}
export type WireEnvelope<T> =
  | { schema_version: 1; kind: "ok"; value: T }
  | { schema_version: 1; kind: "error"; error: Failure };
export interface TryLogRequest { schema_version: 1; event: LogEventDto }
export interface QueryRequest { schema_version: 1; query: LogQueryDto }
export interface HealthRequest { schema_version: 1 }
export interface FlushRequest { schema_version: 1; timeout_ms: number }
export interface LevelChangeRequest { schema_version: 1; change: LevelRequestDto }
```

`StoredEventDto` projects every field of existing
[LogEvent](../../../crates/sc-observability-types/src/events.rs), including
Diagnostic and StateTransition even though first-release emission does not
accept those two optional payloads. `TraceContextDto` and StateTransitionDto map
[tracing types](../../../crates/sc-observability-types/src/tracing.rs);
ProcessIdentityDto maps [process types](../../../crates/sc-observability-types/src/process.rs).
LogQueryDto and LogSnapshotDto map [query types](../../../crates/sc-observability-types/src/query.rs).
LoggingHealthDto and its children map every field of
[existing health types](../../../crates/sc-observability-types/src/health.rs).
Native types and their serialization are unchanged. If bridge is present,
LogHealthDto.logging equals bridge.logging and level_state equals the bridge's
configured/effective/revision fields from the same snapshot, not independent
reads that can cross a mutation. Capture one native logging report and one
coherent level snapshot, then clone their projections into the duplicated fields;
never recapture either just to populate the nested bridge object. Coherence here
means identical duplicated data and one atomic level revision, not an invented
atomic transaction across all independently changing native writer counters. DiagnosticSummaryDto keeps
its optional code/message/at exactly; unlike OperationDiagnostic it cannot
claim to carry remediation. RecoverableSteps.steps() supplies every ordered
remediation step, including an empty externally constructed legacy list; there
is no separate message field on native Remediation.

## TypeScript ergonomic conversion API

The client log/tryLog methods deliberately consume validated wire DTO values.
The following public helpers supply the promised ergonomic bigint conversion;
they return validation results, not thrown exceptions, and share all limits below.

```ts
export type EventValueInput =
  | null | boolean | string | number | bigint
  | readonly EventValueInput[]
  | { readonly [key: string]: EventValueInput };
export type LogEventInput = Omit<LogEventDto, "schema_version" | "fields"> & {
  fields?: Readonly<Record<string, EventValueInput>>;
};
export declare function encodeValue(value: EventValueInput): Result<ValueDto>;
export declare function encodeEvent(event: LogEventInput): Result<LogEventDto>;
```

encodeEvent stamps schema_version 1, defaults absent fields to an empty object,
and otherwise preserves supplied event values after validation; it neither logs
nor supplies host identity. encodeValue emits explicit null/boolean/string/array/
object tags. Safe integral numbers become integer decimal strings (negative zero
normalizes to zero), finite nonintegral numbers become float values, and bigint
is accepted only in -2^63..2^64-1. Unsafe integral numbers, NaN/infinity, cycles,
unsupported prototypes/objects, getters that fail, or non-string object keys
return validation/INVALID_INPUT. Foreign getter errors are contained at the
boundary. Copy own enumerable plain-object entries once; reject symbol keys and
never traverse inherited properties. Callers can use encodeValue for query field
matches. No overload silently accepts ergonomic inputs where a wire DTO is required.

B.3 must test each input variant, min/max/overflow bigint, safe/unsafe number
boundaries, negative zero, cycles, own/inherited/symbol keys, failing getters and
exact size/depth limits. The packed external consumer runs encodeEvent then log
and observes the exact stored integer via query; conversion-only tests are not
sufficient. encodeValue/encodeEvent are included in API-COVERAGE and exports.

## Rust conversion and Tauri boundaries

DTOs use explicit checked conversion functions, not infallible casts or From
implementations that can panic. Native error types are mapped from variants,
never parsed display text. New DTO conversion API signatures are:

```rust
pub fn decode_event(value: serde_json::Value) -> Result<LogEventDto, Failure>;
pub fn decode_query(value: serde_json::Value) -> Result<LogQueryDto, Failure>;
pub fn decode_level_request(value: serde_json::Value) -> Result<LevelRequestDto, Failure>;
pub fn to_core_event(dto: LogEventDto, stamp: EventStamp)
    -> Result<sc_observability_types::LogEvent, Failure>;
pub fn to_core_query(dto: LogQueryDto)
    -> Result<sc_observability_types::LogQuery, Failure>;
pub fn from_core_snapshot(value: sc_observability_types::LogSnapshot)
    -> Result<LogSnapshotDto, Failure>;
pub fn from_core_health(value: sc_observability_types::LoggingHealthReport,
    level: sc_observability_types::LevelState) -> Result<LogHealthDto, Failure>;
pub fn from_level_change(value: sc_observability_types::LevelChange)
    -> Result<LevelChangeDto, Failure>;
pub fn from_level_error(value: sc_observability_types::LevelChangeError) -> Failure;
pub struct EventStamp {
    pub service: sc_observability_types::ServiceName,
    pub timestamp: sc_observability_types::Timestamp,
    pub identity: sc_observability_types::ProcessIdentity,
}
```

EventStamp comes from the host, never input DTOs. `to_core_event` supplies current
native schema version and no diagnostic/state_transition; source provenance is
added by the host using protected metadata before conversion. The DTO crate
depends on public types, serde and serde_json only; bridge health conversion lives in the
Tauri/Python adapters to avoid depending on runtime crates in the DTO crate.
EventStamp fields are public because hosts construct it; its values already use
validated core newtypes. Existing core validation applies again before admission.

Tauri adapter commands return WireEnvelope values even on failure. Their
serialization/argument shape is precisely the B.3 command table; command bodies
accept raw serde_json::Value so malformed user input becomes a tagged validation
result rather than Tauri's implicit argument-extraction rejection. Missing or
invalid invoke-level arguments that prevent command entry are foreign transport
failures contained by JsonTransport. No unvalidated raw value reaches core.

The application retains its LogGuard and gives the adapter a read-only
LogControl; independent core-host implementations provide equivalent public
operations, without creating a bridge or global logger. Python's
HostLoggingBackend contract in B.4 is the embedding boundary, not a raw pointer
or cross-dynamic-library ABI. Full core query/health use public native types;
bridge admission uses EmitOutcome; core admission uses AdmissionOutcome.

## Python ergonomic declarations

In addition to B.4 Logger/AttachedLogger/Result signatures, the following
constructible input data is part of the first-release Python API. Construction
of inert frozen values is not validation; every factory/operation validates them
and returns Result before side effects. Wrong native types/formatter exceptions
become validation or internal failures at the boundary.

```python
Level = Literal["trace", "debug", "info", "warn", "error"]
LevelFilter = Literal["off", "error", "warn", "info", "debug", "trace"]
LevelChangeSource = Literal["application", "user_request", "diagnostic_session"]

@dataclass(frozen=True)
class LoggerConfig:
    service: str
    log_root: str
    level: LevelFilter = "info"
    enable_file_sink: bool = True
    enable_console_sink: bool = False

@dataclass(frozen=True)
class LogEvent:
    level: Level
    target: str
    action: str
    message: str | None = None
    trace: TraceContext | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    outcome: str | None = None
    fields: Mapping[str, object] = field(default_factory=dict)

@dataclass(frozen=True)
class LogQuery:
    service: str | None = None
    levels: tuple[Level, ...] = ()
    target: str | None = None
    action: str | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    since: str | None = None
    until: str | None = None
    field_matches: tuple[FieldMatch, ...] = ()
    limit: int = 100
    order: Literal["oldest_first", "newest_first"] = "oldest_first"

@dataclass(frozen=True)
class FieldMatch:
    field: str
    value: object

@dataclass(frozen=True)
class TraceContext:
    trace_id: str
    span_id: str
    parent_span_id: str | None = None
```

All output values (Admission, Completion, LogSnapshot, LogHealth, LevelState,
LevelChange, ChangeDiagnostic, Failure and their nested records) are generated
from the exact wire declarations/B.3, removing the Dto suffix and translating
integer decimal fields to Python int. Frozen discriminated dataclasses use
Literal kind values with init=False; no success/error boolean pairs or Exception
subclasses. `get_host_logger` has no configuration inputs. Owned LoggerConfig
maps service/root to LoggerConfig::default_for plus its three explicit optional
settings; remaining core config fields retain the published defaults. A future
expansion of configuration needs its own additive reviewed contract.

## Validation and resource decisions

- Input schemas reject unknown fields/tags/versions; optional fields normalize
  to null, levels/field_matches to empty arrays, limit to 100 and order to
  oldest_first. Required event level/target/action never get guessed defaults.
  LogQuery since > until, limit outside 1..1000, and empty field-match names
  return validation, preserving native inclusive time bounds and ordering.
- Call validated core constructors for service/target/action/outcome/trace and
  correlation values rather than invent a second regex. Path input is UTF-8
  text, nonempty and NUL-free; resolve relative paths from the host's documented
  startup working directory once. No environment variable secretly changes an
  explicit Python log_root. Output non-UTF8 paths use unrepresentable; no lossy
  replacement and no file-operation authority is exposed.
- DecimalDto canonical form is `0` or nonzero leading digit followed by digits;
  a negative prefix is permitted only in integer event values. No plus, leading
  zeros or negative zero. Event integers range -2^63..2^64-1 to match serde_json;
  counters/revisions are 0..2^64-1. Floats must be finite. Python bool is never
  accepted as integer. TS ergonomic bigint converts to tagged integer values;
  unsafe integer numbers are rejected, not rounded. Containers are copied with
  a depth/cycle guard before crossing threads.
- Input request size is at most 64 KiB UTF-8 JSON and maximum container depth is
  32. Python applies the same serialized-equivalent bound after checked encoding.
  Cycles, unsupported objects and object keys other than strings fail validation.
  Request size does not limit existing stored output records; queries still
  bound event count to 1000. No truncation silently rewrites stored values.
- All public timeout inputs are integer milliseconds 0..60000 inclusive, default
  2000 where B.4/B.6 signatures supply a default. Zero performs an immediate
  status check, not an unbounded wait. Booleans, fractions, negatives, NaN and
  overflow return validation before starting work. A timeout stops only that
  wait, never cancels underlying ownership, changes level state or implies
  shutdown completed.
- Each adapter maintains at most one in-flight flush and one query per logger.
  Overlap returns queue_full with stable code SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS or
  SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS. Do not coalesce a later flush behind an earlier barrier.
  Tauri fire-and-forget dispatch allows at most 256 outstanding requests per
  client; the next returns queue_full before scheduling. Query and flush helper
  slots remain occupied until actual operation completion even after timeout.
  Level request commands share the owner's short critical section and run on
  that bounded host adapter worker, never spawn an unbounded thread per call.
- Best-effort client failure state is one last Failure and saturating counters
  by known Failure kind; no unbounded history. A failed update preserves the
  original result and makes no logging attempt. Diagnostic code/message and
  remediation are preserved through valid conversions; arbitrary remote text
  is bounded to 4096 UTF-8 bytes per string and 32 remediation steps, with an
  explicit validation/SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE result for
  oversized diagnostics instead of silent loss. That replacement contains a
  short bounded explanation and remediation; it does not copy the oversized
  payload. No result claims to preserve data it rejected.

## Failure mapping and required corner cases

Validation/newtype/DTO failures -> validation with field path; queue admission
full or operation busy -> queue_full with the specific stable code; stopping or
stopped -> closed; unavailable backend/failed writer -> unavailable; actual I/O
failure -> io; timeout/cancel -> their named operation variants; unknown schema
-> unsupported_version; contained foreign panic/formatter failure -> internal;
known but newly unrecognized remote failure tags -> unknown_remote retaining tag
and diagnostic data. Level BelowBaseline and UnsupportedLevel have the explicit
B.3 payload variants. Permission denial is permission_denied. Preserve the
original OperationDiagnostic or ErrorContext code/message/remediation/timestamp; never
replace a typed error by parsing its display string. Legacy health summaries
remain summaries rather than an invented full diagnostic.

The B.3/B.4 authoritative validation lists include fixtures for every union tag,
all nested stored event/health fields, empty and multistep remediation, absent
summary code, maximum integers, invalid decimal forms, NaN/infinity, cycles,
exact size/depth boundaries, unknown input versus additive output fields,
version mismatch, non-UTF8 output paths, timeout zero/max/overflow, duplicate
flush barriers, full dispatch/helper slots and completion after caller timeout.
Run both accepted/filtered paths and malformed versus unknown remote failures.
Owned Python instances remain independent; attached language clients observe
one host snapshot without ownership. No test may normalize away a field or
regenerate expected output to hide a shape change.


## Binding-owned stable diagnostic registry

Define each literal once in `sc-observability-dto::error_codes`, mirror through
schema-generated language constants and enumerate it in errors-v1.json. Native
core/bridge codes pass through and are not redefined in this registry. The
following new boundary codes have fixed Failure kinds and one ordered
Recoverable step; callers decide whether to follow it, and no automatic retry
occurs. Capture time is boundary UTC unless an original native timestamp exists.

| Literal | Failure kind | First remediation step |
| --- | --- | --- |
| `SC_OBSERVABILITY_BINDING_INVALID_INPUT` | validation | Correct the named input field and submit a new request |
| `SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION` | unsupported_version | Install client and host packages supporting the same schema |
| `SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE` | validation | Reduce remote diagnostic text or remediation steps to the documented bounds |
| `SC_OBSERVABILITY_BINDING_DISPATCH_FULL` | queue_full | Wait for an outstanding request to complete before submitting again |
| `SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS` | queue_full | Wait for the existing query to finish before starting another |
| `SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED` | unavailable | Install a host backend before requesting an attached logger |
| `SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED` | unavailable | Reuse the module's existing backend; replacement is unsupported |
| `SC_OBSERVABILITY_BINDING_PERMISSION_DENIED` | permission_denied | Request access through the application's authorized window |
| `SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE` | unavailable | Restore the host connection before submitting a new request |
| `SC_OBSERVABILITY_BINDING_TIMEOUT` | timeout | Inspect operation status before deciding whether another operation is needed |
| `SC_OBSERVABILITY_BINDING_CANCELLED` | cancelled | Inspect the saved operation result if confirmation is still needed |
| `SC_OBSERVABILITY_BINDING_INTERNAL` | internal | Inspect the retained status and restore the affected host or client |

Overlap of native or adapter flush uses the existing accepted bridge literal
`SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS` and queue_full; preserve that bridge
code and its operation-specific remediation. For core-only Python hosts, the
adapter uses the same documented literal for equivalent semantics, without a
runtime bridge dependency. unknown_remote retains the remote code and tag,
not a fabricated replacement code. A malformed remote envelope uses INVALID_INPUT
with field `response`; an oversized diagnostic uses DIAGNOSTIC_TOO_LARGE with
field `response.error`. Validators reject duplicate/missing registry literals
and test kind/remediation/timestamp mapping for each row. Additional B.5/B.6
operation-specific codes are defined by those sprints before first publication,
using the same generated registry rather than standalone language constants.
