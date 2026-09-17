---
status: accepted_target_design_not_source_acceptance
owner: sc-observability
implementation_source: beads-task-issue-tracker
baseline_inspected: 8d8e82ae9f8501dbbf78c76df24d943918d7a6e4
inspection_status: reconciled_snapshot_not_source_acceptance
target_design_accepted_revision: 84b32e9d6718418371ffd25a3de52346278725ca
target_design_qa2: https://github.com/randlee/sc-observability/pull/111#issuecomment-5708115438
---

# Target bridge public API contract and disposition matrix

sc-observability owns this proposed public contract. It is being reviewed now so
BTIT can complete its initial implementation against the accepted design.
`proposed_for_public_api_review` is not approval or a completed freeze. Acceptance
must name a committed revision of this contract; BTIT's subsequent implementation
and critical-review closure must refer to that revision. Only then may B.1 copy
the resulting accepted crate set. There is no post-copy bridge redesign sprint.

## Scoped target-design acceptance

aobs accepted this target design at
`84b32e9d6718418371ffd25a3de52346278725ca`, based on the independent QA2 PASS
recorded at [PR #111](https://github.com/randlee/sc-observability/pull/111#issuecomment-5708115438).
The current status is `accepted_target_design_not_source_acceptance`: it accepts
the reviewed target signatures and dispositions for BTIT implementation only.
It is not runtime public-API acceptance, BTIT source acceptance, merge
permission, or publication permission; QA-B010 remains owner-deferred. BTIT
must still supply its immutable implementation and critical-review evidence.

BTIT's current unpublished API creates no backward-compatibility or semver
obligation for this initial destination API. The preserve decisions below retain
useful behavior deliberately. Revise decisions are intentional initial-design
changes, not promises to grandfather old consumers. Existing published core
crates retain their independent API guarantees.

## Exported source API disposition

The inventory below was reconciled against the read-only BTIT snapshot
`8d8e82ae9f8501dbbf78c76df24d943918d7a6e4`. It is not acceptance of that
source, a claim that its implementation is complete, or a substitute for the
final source/review handoff. Every export added or changed after that snapshot
must receive a row or an explicit family disposition before the B.P3 contract
gate and the B.1 copy gate can be accepted.

| Existing exported API | Target decision | Signature/behavior consequence |
| --- | --- | --- |
| `init` | Preserve signature; revise internals | `fn(LoggerConfig, BridgeOptions) -> Result<LogGuard, InitError>`; one lifecycle owner, complete guard coverage, resolved identity |
| `BridgeOptions` and its `default_action`, `parse_bracket_action` fields; Debug/Clone | Preserve | Existing field names/types and bracket parsing opt-in; no hidden frontend/runtime policy |
| `LoggerConfig` re-export | Preserve | Same core type, not a duplicate binding configuration |
| `ActionName`, `ErrorCode`, `LevelFilter`, `ProcessIdentityPolicy`, `Remediation`, `ServiceName`, `TargetCategory` re-exports | Preserve all | Same defining core types and semantics |
| `Level`; TRACE/DEBUG/INFO/WARN/ERROR; Clone/Copy/Debug/Eq/PartialEq/Hash; conversion into core Level | Preserve all | Keep tracing-style constants and deliberately absent Ord/PartialOrd |
| `trace!`, `debug!`, `info!`, `warn!`, `error!`, `event!`, `#[instrument]` at bridge and macros crate roots | Preserve accepted grammar/output intent; revise guarded execution | Unit-return compatibility APIs intentionally ignore admission Result after exactly-once accounting; no Result-returning replacement of standard log macros |
| `DroppedEvents`; Debug/Clone/Copy/Default/PartialEq/Eq; get/total | Preserve; add serde projection support | `get(&self, DropCause) -> u64`, saturating `total(&self) -> u64`; private counters stay private |
| `DropCause`; ALL and current seven variants/derives | Preserve all | QueueFull, InvalidEvent, WriterDegraded, ShutdownTimedOut, NotInstalled, LoggerPanicked, ReentrantEmit; new direct-path errors map to these existing accounting categories |
| `DEFAULT_DROP_SHUTDOWN_TIMEOUT` | Preserve | Duration = 2 seconds; fallback wait limit, not a second shutdown operation |
| `LogGuard`; Debug, must_use, !Clone | Preserve shape of ownership; revise shutdown coordination | Sole owner; opaque fields; Send+Sync; consumers share LogControl rather than Arc<LogGuard> |
| `LogGuard::elevate_level` / `reset_level` | Add before copy | Owner-only Result methods defined by [runtime contract](runtime-level-contract.md); never added to LogControl |
| Runtime level value/error re-exports and BridgeHealthReport level fields | Add before copy | Core-owned definitions and coherent configured/effective/revision snapshot |
| `LogGuard::flush` | Preserve signature; revise post-stop behavior | `(&self, Duration) -> Result<(), FlushError>`; no success on a stopped/nonrunning logger |
| `LogGuard::shutdown` | Preserve signature; revise completion ownership | `(self, Duration) -> Result<(), ShutdownError>`; exactly one operation, deadline includes contention, late result retained |
| `LogGuard::dropped_events` | Preserve | `(&self) -> DroppedEvents` infallible snapshot accessor |
| `LogGuard::active_log_path` | Preserve | `(&self) -> Option<&Path>`; init-cached path, None if file sink disabled |
| `Drop for LogGuard` | Revise behavior intentionally | Starts final shutdown at most once; waits at most fallback timeout; preserves outcome for extant controls; cannot panic or start a second attempt |
| `InitError` and variants/code()/remediation() | Revise payloads; add RuntimeStart/UnsupportedLevel | Serializable typed diagnostics replace opaque native source objects; see definitions below |
| `FlushError` and variants/code()/remediation() | Revise payloads; add NotRunning and InProgress | Stop/timeout/overlap/helper failure remain distinct typed paths |
| `ShutdownError` and variants/code()/remediation() | Revise payloads; retain four named cases | Timeout is a waiting error, not terminal completion; final results are separately observable |
| `error_codes` module, seven named constants, ALL | Preserve existing names/string values; extend registry | Existing code meanings remain stable; new cases get the constants listed below |
| `__private` module and all reachable support | Retain as hidden macro support; may revise with macros in lockstep | Not a supported adapter API; exact bridge/macros version pin governs changes |
| CI-only consumer-check crate | Preserve role, never publish | Depends directly only on bridge and exercises generated macro support externally |

### Snapshot reconciliation and disposition

The following groups cover every root-reachable export observed at
`8d8e82ae9f8501dbbf78c76df24d943918d7a6e4`; the inspection command and paths
are retained in [the B.P3 handoff record](handoff-b-p3.md). Grouping does not
leave an export without a disposition.

| Observed source export family | Target disposition |
| --- | --- |
| `JsonMap`, `JsonValue`, `LogControl`, `StructuredRecord`, `SubmitOutcome` | Revise to the proposed direct-result `BridgeEvent`/`EmitOutcome` control surface; retain non-owning control and no mutation/shutdown authority. |
| `SubmitError`, `InvalidInputReason`, `DropCause`, `DroppedEvents` | Revise to `EmitError` plus nested `FieldKeyError`; preserve exactly-once rejection accounting and the existing drop-cause family. |
| `BridgeHealthReport`, `BridgeHealthState`, `BridgeLifecycle`, `HelperHealth`, `LoggerHealth`, `QueueHealth`, `FileSinkHealth`, `SinkHealthSnapshot`, `SinkStatus`, `WriterStatus`, `HealthDiagnostic`, `BRIDGE_HEALTH_SCHEMA_VERSION` | Revise to the proposed bridge health/completion projection, retaining readable post-stop health but separating failed/unconfirmed completion from a stopped claim. |
| `FailureReport`, `Failure`, `InitFailure`, `FlushFailure`, `ShutdownFailure`, `CONTROL_SCHEMA_VERSION` | Replace with the native result/error projection and B.3 wire contract; no source report shape becomes a destination compatibility baseline. |
| `Timestamp` and the listed neutral re-exports | Preserve as re-exports of their defining core types; add only the target's explicitly listed runtime-level values/errors. |
| Root macros `trace!`, `debug!`, `info!`, `warn!`, `error!`, `event!`, `#[instrument]`; macro-crate entry points | Preserve grammar/output intent and exact bridge-to-macro pin; revise internals only for the one guarded/admission path. |
| `Level`, `BridgeOptions`, `LogGuard`, `init`, `DEFAULT_DROP_SHUTDOWN_TIMEOUT`, `error_codes` and its observed registry | Preserve only the listed public shape/meaning; revise the documented lifecycle, error payload and registry extensions before copy. |
| `__private` reachable support (callsite, context and mapping helpers, including all public impls/derives) | Retain only as hidden exact-version macro support; inventory it in B.1's generated export report and preserve external macro fixtures, not private API compatibility. |

The observed source snapshot has no `FieldKeyError`; its current field rejection
is `SubmitError::InvalidInput(InvalidInputReason)`. The target's new nested
reason is therefore an intentional pre-copy design revision, not a claim that
the pinned source already implements it.

The seven existing code constants are ALREADY_INITIALIZED,
FOREIGN_LOGGER_INSTALLED, IDENTITY_RESOLUTION_FAILED, FLUSH_TIMED_OUT,
SHUTDOWN_TIMED_OUT, HELPER_SPAWN_FAILED, HELPER_LOST, each with the prefix
`SC_OBSERVABILITY_LOG_`; preserve their full names and string values.

Hidden-support inventory covered by the final matrix row: `Map`, `Value`, core
`Level`; `Callsite` (new), `DynamicKey` (new), `FieldRecord`, `FieldValue`,
`SerializeKind`, `DebugKind`, `DebugKindTag`, `FieldDebug`, dispatch methods and
impls, `debug_value`, `display_value`, `record_field`, `record_dynamic_field`,
`emit_callsite`; `CallLevels` fields, `CallOutcome`/label, `CallSpan` new/enabled/
enter/finish_ok/finish_err and Drop, `Entered` and Drop, `current_trace`;
`LabelKind`, `LabelError`, RESERVED_FIELD_PREFIX, sanitize_label, target_label,
action_label, field_key_label; `EventParts` and every existing field, enabled,
emit, record_drop. Hidden signatures may change to establish one guard and one
result/accounting path; preserve external macro fixtures, not accidental private
API consumers. Inventory public impls and derives in the generated export report
as well as named items so no reachable API silently escapes review.

## Proposed added public API

All types below are bridge-owned companion contracts under an explicit scoped
TYP-030 exception; core types remain owned by sc-observability-types. Pure data
has Debug/Clone/PartialEq plus serde Serialize/Deserialize; value enums without
payloads additionally have Copy/Eq. Native enum serialization uses snake_case
discriminants. Payload enums use `#[serde(tag = "kind", content = "value",
rename_all = "snake_case")]`; unit-only enums serialize their snake_case string.
Struct fields use snake_case and only active variant payloads are serialized.
B.3 owns the shared wire format; B.3a/B.4 project it rather than exposing native
Duration, PathBuf or large integers directly. Opaque LogControl is Debug/Clone/
Send/Sync, not Serialize; LogGuard is Debug/Send/Sync, not Clone or Serialize.

Re-export the defining neutral types needed by public signatures: LogEvent,
LogQuery, LogSnapshot, CorrelationId, TraceContext, OutcomeLabel,
LoggingHealthReport and OperationDiagnostic. Re-export core Level as `EventLevel`
so the existing tracing-style `Level` name retains its meaning. The input record
is a typed producer description, not permission to replace host identity:

```rust
pub struct BridgeEvent {
    pub level: EventLevel,
    pub target: TargetCategory,
    pub action: Option<ActionName>,
    pub message: Option<String>,
    pub outcome: Option<OutcomeLabel>,
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub request_id: Option<CorrelationId>,
    pub correlation_id: Option<CorrelationId>,
    pub trace: Option<TraceContext>,
}
pub use sc_observability_types::AdmissionOutcome;
pub type EmitOutcome = AdmissionOutcome; // compatibility spelling, not a second enum
pub enum LifecyclePhase { Running, Stopping, Stopped, Failed }

impl LogGuard {
    pub fn control(&self) -> LogControl;
    pub fn health(&self) -> Result<BridgeHealthReport, ControlError>;
}
impl LogControl {
    pub fn try_log(&self, event: BridgeEvent) -> Result<AdmissionOutcome, EmitError>;
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, ControlError>;
    pub fn flush(&self, timeout: std::time::Duration) -> Result<(), FlushError>;
    pub fn health(&self) -> Result<BridgeHealthReport, ControlError>;
    pub fn active_log_path(&self) -> Result<Option<std::path::PathBuf>, ControlError>;
    pub fn dropped_events(&self) -> DroppedEvents;
    pub fn wait_stopped(&self, timeout: std::time::Duration)
        -> Result<ShutdownReport, WaitError>;
}
```

Control has no shutdown method and dropping it has no shutdown effect. Direct
submission and facade/macro emission reach the same writer, validation, redaction
and accounting path. Ordinary operations use Result; source-compatible infallible
snapshot/constant accessors retain plain values or Option. No error path uses
intentional panic/unwrap/expect. Host-provided formatter/redactor panics reached
inside the logger are contained; caller expressions evaluated before entering
the API remain caller code.

## Error and completion signatures

`OperationDiagnostic` is the additive neutral code/message/remediation/timestamp
value defined by the [runtime-level contract](runtime-level-contract.md) and
qualified as a staged artifact by B.P2. It intentionally does not replace the
existing `Diagnostic`: both derive exactly `Debug`, `Clone`, `PartialEq`,
`Serialize`, and `Deserialize`, but `Diagnostic` is the reusable full payload
with `timestamp`, optional `cause`/`docs`, and `details`, whereas
`OperationDiagnostic` is the narrow operation projection with `at` and only
mandatory `code`, `message`, and `remediation`. It does not alter the sealed
`DiagnosticInfo` contract or rewrite any existing public type. The existing
`DiagnosticSummary` has only optional code, message and timestamp; it cannot
preserve remediation and remains unchanged. Convert from an original
`Diagnostic`/`ErrorContext` before reducing to a summary. Where a core API
exposes only summary data, use the explicit operation-specific fallback
remediation below rather than claiming original remediation survived.
Replace opaque source objects in the unpublished bridge's three old error enums
with OperationDiagnostic; no published core error or summary changes shape.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InitError {
    AlreadyInitialized,
    ForeignLoggerInstalled,
    UnsupportedLevel { configured: LevelFilter, available: LevelFilter },
    IdentityResolution { diagnostic: OperationDiagnostic },
    Logger { diagnostic: OperationDiagnostic },
    RuntimeStart { diagnostic: OperationDiagnostic },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FlushError {
    TimedOut { timeout: std::time::Duration },
    Logger { diagnostic: OperationDiagnostic },
    HelperSpawn { diagnostic: OperationDiagnostic },
    HelperLost { diagnostic: OperationDiagnostic },
    InProgress,
    NotRunning { phase: LifecyclePhase },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ShutdownError {
    TimedOut { timeout: std::time::Duration },
    FinalFlush { diagnostic: OperationDiagnostic },
    HelperSpawn { diagnostic: OperationDiagnostic },
    HelperLost { diagnostic: OperationDiagnostic },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FieldKeyError {
    Empty,
    ReservedPrefix,
    Collision { other_raw_key: String },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EmitError {
    InvalidField { raw_key: String, reason: FieldKeyError },
    InvalidEvent { diagnostic: OperationDiagnostic },
    QueueFull { diagnostic: OperationDiagnostic },
    WriterDegraded { diagnostic: OperationDiagnostic },
    ShutdownTimedOut { diagnostic: OperationDiagnostic },
    NotRunning { phase: LifecyclePhase },
    Reentrant,
    Panicked,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ControlError {
    NotRunning { phase: LifecyclePhase },
    Query { diagnostic: OperationDiagnostic },
    Unavailable { diagnostic: OperationDiagnostic },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WaitError {
    NotStarted,
    TimedOut { timeout: std::time::Duration },
    Unavailable { diagnostic: OperationDiagnostic },
}
pub enum UnconfirmedShutdown {
    HelperSpawn { diagnostic: OperationDiagnostic },
    HelperLost { diagnostic: OperationDiagnostic },
}
pub enum ShutdownOutcome {
    Stopped,
    StoppedWithFlushError { diagnostic: OperationDiagnostic },
    Unconfirmed { cause: UnconfirmedShutdown },
}
pub struct ShutdownReport {
    pub outcome: ShutdownOutcome,
    pub health: BridgeHealthReport,
}
pub struct BridgeHealthReport {
    pub schema_version: u32, // exactly 1 for this native report version
    pub logging: LoggingHealthReport,
    pub dropped: DroppedEvents,
    pub lifecycle: LifecyclePhase,
    pub active_log_path: Option<std::path::PathBuf>,
    pub configured_level: LevelFilter,
    pub effective_level: LevelFilter,
    pub level_revision: u64,
}
```

Every public failure enum has `code(&self) -> ErrorCode` and
`remediation(&self) -> Remediation`. Add these prefixed registry constants:
RUNTIME_START_FAILED, NOT_RUNNING, INVALID_FIELD, REENTRANT_EMIT, LOGGER_PANICKED,
STATUS_UNAVAILABLE, SHUTDOWN_NOT_STARTED, FLUSH_IN_PROGRESS, UNSUPPORTED_LEVEL. Existing helper/timeout constants apply
to the corresponding new observation variants; wrapped core diagnostics preserve
their native code and remediation. OperationDiagnostic.code is mandatory;
summary-only input uses STATUS_UNAVAILABLE if its optional code is absent and
uses the explicit fallback remediation policy below. FieldKeyError is a nested reason, not an independent operation
error. ALL lists every bridge-defined code once; test uniqueness and exhaustive
variant-to-code/remediation mapping. No language binding maps these errors to
exception classes; B.3's Result/Failure/Remediation union is the wire projection.

The six operation-error enums above deliberately derive only Debug, Clone,
Serialize and Deserialize; no Copy, Eq, PartialEq or default variant is promised.
Their nested LifecyclePhase and FieldKeyError must also implement Debug, Clone,
Serialize and Deserialize (LifecyclePhase additionally Copy + Eq + PartialEq).
OperationDiagnostic already supplies the required diagnostic derives. Keep the
native tagged serialization rules in the runtime contract. Implement Display
and std::error::Error for all six operation-error enums and for FieldKeyError.
FieldKeyError remains a nested `EmitError::InvalidField` reason: it has no
independent operation code, remediation method, or top-level operation-result
role. Display is diagnostic text, never a decoding protocol, and source()
returns None: the operation enums retain projected OperationDiagnostic data,
while FieldKeyError contains only field-validation data, not native source
objects. Assert trait bounds, clone fidelity, every variant's Serde round trip
and Error/Display usability in B.P3; B.1 reruns those accepted fixtures. No
published core derive list changes.

### Bridge variant/code and fallback remediation map

All bridge codes below use the SC_OBSERVABILITY_LOG_ prefix. Wrapped
OperationDiagnostic values return their contained code/remediation unchanged.
For native ErrorContext/Diagnostic input, copy those fields exactly. Where only
DiagnosticSummary or a foreign helper error exists, use the listed fallback;
never parse Display text, infer recoverability from text, or invent an original
source chain. Expected operational errors return these data values, not throws.

| Variant family | Code | Fallback remediation |
| --- | --- | --- |
| InitError::AlreadyInitialized | ALREADY_INITIALIZED | NotRecoverable: reuse the existing logger; do not reinstall |
| InitError::UnsupportedLevel | UNSUPPORTED_LEVEL | NotRecoverable: rebuild with the required static level support or choose a supported startup baseline |
| InitError::ForeignLoggerInstalled | FOREIGN_LOGGER_INSTALLED | NotRecoverable: choose one application logger before startup |
| InitError::IdentityResolution | IDENTITY_RESOLUTION_FAILED unless a native diagnostic exists | Recoverable: repair identity configuration, then explicitly retry initialization |
| InitError::RuntimeStart | RUNTIME_START_FAILED | Recoverable: inspect thread/resource availability, then explicitly retry initialization |
| InitError::Logger, FlushError::Logger, ShutdownError::FinalFlush | native diagnostic code; summary code if present; otherwise STATUS_UNAVAILABLE | NotRecoverable: inspect logger health and original diagnostic; no automatic retry or success claim |
| FlushError::TimedOut | FLUSH_TIMED_OUT | Recoverable: inspect health while the existing operation completes and releases its slot; no late-result retrieval or automatic retry |
| ShutdownError::TimedOut | SHUTDOWN_TIMED_OUT | Recoverable: use control.wait_stopped to observe the original shutdown |
| helper-spawn variants | HELPER_SPAWN_FAILED | NotRecoverable: inspect resource availability and logger health; lifecycle completion is unconfirmed |
| helper-lost variants | HELPER_LOST | NotRecoverable: inspect saved lifecycle/health; do not claim worker completion |
| FlushError::InProgress | FLUSH_IN_PROGRESS | Recoverable: wait for the in-flight flush to finish before an explicit new request |
| FlushError/EmitError/ControlError::NotRunning | NOT_RUNNING | NotRecoverable: stop submitting through this logger and inspect lifecycle |
| EmitError::InvalidField | INVALID_FIELD | Recoverable: repair the rejected field keys before explicit resubmission |
| EmitError::InvalidEvent/QueueFull/WriterDegraded/ShutdownTimedOut | contained native code | Preserve the native diagnostic remediation |
| EmitError::Reentrant | REENTRANT_EMIT | NotRecoverable: remove logging from formatter/redactor/diagnostic callbacks |
| EmitError::Panicked | LOGGER_PANICKED | NotRecoverable: repair the panicking callback; do not retry implicitly |
| ControlError::Query | contained native code | Preserve native query diagnostic; summary-only uses NotRecoverable inspection guidance |
| ControlError/WaitError::Unavailable | STATUS_UNAVAILABLE | NotRecoverable: inspect logger health and lifecycle evidence |
| WaitError::NotStarted | SHUTDOWN_NOT_STARTED | Recoverable: request shutdown from the owner before waiting |
| WaitError::TimedOut | SHUTDOWN_TIMED_OUT | Recoverable: call wait_stopped again to observe the same operation |

Fallback Recoverable values retain nonempty ordered action steps; diagnostic
message is a separate field. NotRecoverable values retain their justification. They never authorize an
automatic retry. FieldKeyError remains a nested reason under INVALID_FIELD.

## Lifecycle and admission contract

- Initialization establishes one owner and an independently retained completion
  state. A baseline above log::STATIC_MAX_LEVEL fails initialization with
  UnsupportedLevel before global installation or a usable writer is created.
  Core-only loggers have no facade ceiling. Failure to start required lifecycle machinery is RuntimeStart; it must
  not leave a usable partial global installation.
- Running accepts direct and compatibility records. Check the selected level
  before queue admission: Filtered means no enqueue, Accepted means successful
  nonblocking enqueue. Neither proves write, flush or durable persistence.
- The shutdown transition closes admission exactly once. An operation linearized
  before that transition may finish; later direct submissions get NotRunning.
  Compatibility calls may ignore/count it as NotInstalled. Accepted records drain
  through core shutdown. Controls do not hold ownership that prevents final
  shutdown; no Arc::try_unwrap polling on user control lifetimes.
- Owner shutdown waits up to its supplied duration including coordination. A
  timeout returns ShutdownError::TimedOut while the one shutdown operation
  continues. It does not become a terminal lifecycle phase. A control can call
  wait_stopped repeatedly; before shutdown it returns NotStarted, during a wait
  deadline it returns TimedOut, and after completion it returns the saved report.
- Successful join sets Stopped, including the StoppedWithFlushError outcome.
  An unconfirmed helper failure sets Failed and never claims the writer stopped.
  A failed final flush remains distinguishable from an unconfirmed join.
- Health/path/counter snapshots remain inspectable after stop or owner timeout;
  query and flush reject nonrunning use. Final health is retained without requiring
  the consumed Logger to remain borrowable. At most one native flush is in flight
  per logger. A concurrent request returns FlushError::InProgress immediately;
  no implicit retry or coalescing across different admission barriers. A timeout
  leaves the slot occupied until completion. Initialization reserves one lifecycle
  coordinator; shutdown is executed once by it, with at most one final shutdown
  operation. Do not spawn a thread per timeout. Completion waiters are synchronous
  caller-owned waits with no retained per-waiter native registration; async
  adapters enforce their own bounded waiter counts.
- Direct and ignored-result facade paths count each failed record exactly once.
  Filtered records are not drops. Failed diagnostic accounting cannot replace an
  operation result with a panic, recursion, retry or success.

EmitOutcome is a public alias of core AdmissionOutcome; matching/conversion uses
the same type identity. No duplicate admission enum or mapping is introduced.

## Behavioral compatibility decisions

| Behavior | Decision for the target contract | Required evidence |
| --- | --- | --- |
| Field-key normalization | Intentional revision of raw log::kv behavior: use the same sanitizer for facade, macros and BridgeEvent; `::` -> `.`, other non-ASCII-label characters -> `_` | Equivalent keys from all paths produce the same output |
| Empty/reserved keys | Reject the entire event once; reserved prefix is `sc_observability_log.` after normalization | Typed direct InvalidField and one InvalidEvent drop; no partial successful record |
| Normalized key collisions | Reject the entire event deterministically, including duplicate raw keys in log::kv; never last-write-wins | Colliding raw keys are reported; no ambiguous field overwrite |
| Target/action and bracket tag | Preserve existing target fallback `log`, action validation, and opt-in bracket parsing | Existing mapping fixtures plus invalid-label cases |
| Auto identity | Intentional revision: bridge resolves/cache hostname and PID once from the OS, rather than PID-only | Cross-platform hostname+PID fixture; resolution failure returns IdentityResolution before install |
| Fixed/Resolver identity | Preserve exact supplied/resolved identity; no per-event resolver calls | Resolver called once; failure retry is typed and does not partially install |
| Direct record envelope | Host supplies service, timestamp and process identity; explicit validated trace takes precedence over ambient context, otherwise inherit ambient trace | Frontend/Rust/Python provenance and correlation remain distinguishable without spoofed host identity |
| Global install | Preserve install-once; no normal reinitialization after a successful install/shutdown; foreign logger remains an explicit error | Subprocess tests for first/repeated/foreign/failed-init cases |
| No-op log::Log::flush | Preserve; bounded explicit control/guard flush is the observable API | Standard facade flush cannot block on sink I/O |
| Formatter panic/reentrancy | Correct implementation to meet the target no-unwind/no-recursion contract across formatting and submission | One outer guard, one result and one drop count per failed record |
| Lifecycle/shared guard | Intentional revision: share control, not shutdown ownership; retain late result | Concurrent flush/exit and long-lived control tests |

The Auto hostname implementation may add a reviewed bridge-only OS helper
dependency; it must not add runtime/platform tooling to the core types crate.
The exact dependency graph is reviewed with BTIT's implementation and copied
by B.1 as source evidence, not guessed or frozen from the incomplete baseline.

## Initial version and publication policy

The new companion pair joins the sc-observability workspace release train and
always publishes at one matching version, with bridge -> macros pinned exactly
`=V`. B.2 selects the next release-train version before release preflight and
records it in the release artifact inventory. BTIT's private version and API
are not a semver baseline. Intentional initial-design changes above need design
review and documentation, not a backwards-compatibility exception. Existing
published core packages retain their normal gates. After the first companion
publication its accepted public API becomes the baseline for subsequent releases.
Hidden __private support stays outside the public stability promise and changes
only with the exact matching macro package. DTO schema versions remain separate.

## Public-API contract gate required before copy

The [B.1 entry gate](sprint-b-1-copy.md#goal-and-entry-gate) is authoritative
for copy admission. This contract supplies its exported API inventory,
variant/code fixtures and behavioral evidence; B.P3 supplies accepted BTIT
implementation and cross-platform critical-review evidence.
All bridge changes foreseeable in this proposal are BTIT pre-migration work.
Do not defer an identified target-contract change to a planned post-copy API
revision. Future unforeseen changes can be reviewed after migration without
creating a speculative redesign workstream now.

## Required error and corner-case fixtures

B.P3 owns bridge integration fixtures and B.1 reruns the accepted set: every
InitError/FlushError/ShutdownError/EmitError/ControlError/WaitError variant and
its stable code/remediation; native enum Serde roundtrip; repeated/foreign/failed
install; resolver failure then retry; invalid/empty/reserved/colliding field keys;
formatter panic and nested logging; direct accepted versus filtered outcomes;
queue saturation; running/stopping/stopped/failed health and operations; concurrent
flush InProgress, timeout then late flush completion, zero-duration timeout;
shutdown during flush, retained controls after owner drop, repeated wait_stopped,
helper-spawn/lost-helper failures and final-flush failure without false stopped
claims. Fault accounting failure never replaces the original result. Include the
runtime-level transition/diagnostic/static-cap matrix from its contract.
Include B.P3 compile-fail proof that LogControl cannot mutate/elevate/reset or
obtain owner authority. When bindings exist, B.3a/B.4 own host-routing fixtures
that prove TypeScript and attached Python route level-change requests through an
application-owned handler rather than LogControl; this requirement does not
invent a pre-copy binding implementation.

The scoped target-design acceptance above does not assert source readiness,
runtime public-API acceptance, a merge permission, or a publication freeze.

## Runtime elevation extension required before copy

The [runtime level contract](runtime-level-contract.md) is part of this proposed
target: LogGuard gains owner-only elevate_level/reset_level Result methods and
BridgeHealthReport gains configured_level, effective_level and level_revision.
LogControl gains no mutation authority. The contract specifies the shared core
filter, concurrency, typed outcomes, diagnostic submission and compile-time
ceiling behavior. This extends the export disposition matrix for these methods,
re-exported level values/errors and health fields. Review/accept it and qualify
the B.P2 staged core prerequisite before BTIT completes the working reference.
B.P3 integrates against those immutable staged artifacts; B.7 alone owns live
publication and registry-only proof.
