---
status: proposed_for_public_api_review
owner_deferral_date: 2026-09-17T02:41:50Z
owner_deferral_message: 01M2PKX8R4J4VJP5V6RRV9JJPB
execution_status: authorized
execution_stop_withdrawn_by: aobs
issue: 97
owner_direction_date: 2026-09-16 America/Los_Angeles
---

# Runtime level elevation — pre-copy contract and core prerequisite

## Ownership and sequencing

This is an explicit prerequisite to B.1, not a post-copy implementation task.
On 2026-09-16 (America/Los_Angeles), aobs relayed the owner's instruction:
"you task was to complete phase-b w/ publish delayed until the end."
As coordinating lead, aobs withdraws the additional manual execution stop it
imposed under QA-B010. The owner subsequently deferred contract acceptance to
Phase B completion (ATM `01M2PKX8R4J4VJP5V6RRV9JJPB`, 2026-09-17T02:41:50Z).
This contract remains proposed for public API review; QA-B010 is owner-deferred,
not resolved and not a development blocker.

sc-observability implements and qualifies the additive core capability first;
BTIT implements the corresponding bridge API and behavior against B.P2's exact
staged core artifacts and the accepted target contract, closes its critical
review, and hands off the working reference. B.1 remains the first migration
sprint and copies it mechanically. Live registry publication and registry-only
consumer proof are deferred to the phase-end release checklist in B.7; record
the staged core artifact and accepted BTIT source in the import gate instead.

Execution ownership and closure are defined once in the prerequisite sprints:
[B.P1 core implementation](sprint-b-p1-runtime-core.md),
[B.P2 package qualification](sprint-b-p2-runtime-publish.md), and
[B.P3 BTIT integration](sprint-b-p3-runtime-btit.md).
This document is the normative signature/behavior reference, not an additional
sprint or a separate closure checklist. B.3 owns the shared wire projection; B.3b owns native backends and conversions;
B.3a and B.4 own TypeScript/Tauri and Python runtime projections.

## Proposed API and values

The following remains proposed for review and is not a registry-release or
cross-project API-freeze approval.
Core types live in the neutral types crate; owner/state implementation lives in
sc-observability. Bridge re-exports these values rather than duplicating enums.

```rust
// Value types derive Debug, Clone, PartialEq; AdmissionOutcome, LevelChangeSource
// and LevelState additionally derive Copy + Eq. DiagnosticSummary is not Eq.
// Error/value native Serde is additive and separate from binding wire Serde.
// OperationDiagnostic derives Debug + Clone + PartialEq + Serialize + Deserialize.
pub struct OperationDiagnostic {
    pub code: ErrorCode,
    pub message: String,
    pub remediation: Remediation,
    pub at: Timestamp,
}
pub enum AdmissionOutcome { Accepted, Filtered }
pub enum LevelChangeSource { Application, UserRequest, DiagnosticSession }
pub struct LevelState {
    pub configured_level: LevelFilter,
    pub effective_level: LevelFilter,
    pub revision: u64,
}
pub enum ChangeDiagnostic {
    Accepted,
    NotAccepted { diagnostic: OperationDiagnostic },
}
pub enum LevelChange {
    Changed {
        previous: LevelState,
        current: LevelState,
        source: LevelChangeSource,
        diagnostic: ChangeDiagnostic,
    },
    Unchanged { state: LevelState },
}
pub enum LevelChangeError {
    Stopping,
    Stopped,
    BelowBaseline { requested: LevelFilter, configured: LevelFilter },
    UnsupportedLevel { requested: LevelFilter, available: LevelFilter },
    Unavailable { diagnostic: OperationDiagnostic },
}
impl LevelChangeError {
    pub fn code(&self) -> ErrorCode;
    pub fn remediation(&self) -> Remediation;
}
// LevelChangeError implements Display + std::error::Error. No source object is
// invented from diagnostic data; source() returns None.
// Opaque capability: Debug (no internal addresses), Send + Sync, not Clone.
// Private fields are deliberately not part of the public contract.
pub struct LevelOwner { /* private weak control-state reference */ }
impl Logger<Running> {
    pub fn try_log_with_outcome(&self, event: LogEvent)
        -> Result<AdmissionOutcome, TryLogError>;
    pub fn new_with_level_owner(config: LoggerConfig)
        -> Result<(Self, LevelOwner), sc_observability_types::InitError>;
}
impl<State> Logger<State> {
    pub fn level_state(&self) -> LevelState;
}
impl LoggerBuilder {
    pub fn build_with_level_owner(self)
        -> Result<(Logger<Running>, LevelOwner), sc_observability_types::InitError>;
}
impl LevelOwner {
    pub fn elevate_level(&mut self, level: LevelFilter, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
    pub fn reset_level(&mut self, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
}
impl LogGuard {
    pub fn elevate_level(&mut self, level: LevelFilter, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
    pub fn reset_level(&mut self, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
}
```

`OperationDiagnostic` is intentionally a narrow operation projection, not a
rename or replacement for the existing `Diagnostic`. Both derive exactly
`Debug`, `Clone`, `PartialEq`, `Serialize`, and `Deserialize`; `Diagnostic`
retains its full reusable payload (`timestamp`, optional `cause`/`docs`, and
`details`), while OperationDiagnostic carries only mandatory code, message,
remediation, and `at` for a committed operation's retained outcome. It neither
implements nor changes the sealed `DiagnosticInfo` contract. Conversion copies
the original code/remediation unchanged when they are available; no conversion
claims recovery of details a legacy summary already discarded.

B.1b's improved logger error API must include the new construction boundary in its
inventory; this prerequisite preserves existing core error conventions until
that additive migration ships. Failure enums carry stable code/remediation
accessors. Binding DTOs use explicit snake_case discriminators and their checked
u64 conversion for revision; deriving Serde alone does not establish wire
compatibility. AdmissionOutcome lives in the neutral types crate and is re-exported by core.
try_log_with_outcome returns Filtered only after successful event validation and
level filtering, with no enqueue attempt; Accepted means queue admission. It
uses the same implementation as legacy try_log, which maps either outcome to
Ok(()). No additional queue or second validation/redaction pass is introduced.

Core level health is an additive accessor, not a breaking field
addition to a published constructible health struct. The unpublished bridge
health contract includes configured_level, effective_level and level_revision.

## Native serialization contract

All new value/error types derive Serialize/Deserialize. AdmissionOutcome and
LevelChangeSource serialize as snake_case strings. LevelState and
OperationDiagnostic serialize as objects with their declared snake_case fields;
revision is a native u64 number and ErrorCode/Timestamp/Remediation retain their
existing public native encodings. ChangeDiagnostic, LevelChange and
LevelChangeError use `#[serde(tag = "kind", content = "value", rename_all = "snake_case")]`:
unit variants have kind only; payload variants have a value object containing
exactly their declared fields. Missing required fields or unknown enum tags
fail deserialization; additional object fields follow Serde's default ignored
unknown-field behavior. No native field has a silent default. LevelOwner has no
Serialize/Deserialize implementation. Native shapes are frozen on first release;
the B.3 wire DTOs deliberately project them into their own flattened versioned
schema with checked integers. Existing native types/Serde are untouched.

## Level failure registry

The new closed enums have no non_exhaustive attribute; adding variants later
requires a separately reviewed compatibility strategy. LevelChangeError code()
returns the following stable registry identifiers. Display is diagnostic text,
never a machine classification input. remediation() returns Recoverable with
these actions, except unavailable/overflow as specified.

| Variant | Stable code | Remediation action |
| --- | --- | --- |
| Stopping | `SC_OBSERVABILITY_LEVEL_STOPPING` | Wait for shutdown completion; create a new logger if logging is still needed |
| Stopped | `SC_OBSERVABILITY_LEVEL_STOPPED` | Create a new logger; do not retry this owner |
| BelowBaseline | `SC_OBSERVABILITY_LEVEL_BELOW_BASELINE` | Request the configured level or greater verbosity |
| UnsupportedLevel | `SC_OBSERVABILITY_LEVEL_UNSUPPORTED` | Rebuild the application without the conflicting static level cap |
| Unavailable | contained OperationDiagnostic.code | Preserve its contained remediation; no inferred retry |

For Unavailable, code()/remediation() return the contained fields unchanged.
When synthesizing state-unavailable or revision-exhausted diagnostics, use
NotRecoverable with justification to inspect state and create a new logger;
other variants use the listed Recoverable action as their sole first step.

Revision exhaustion stores diagnostic code `SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED`
inside Unavailable; it never wraps. Other Unavailable conditions are a failed
writer or poisoned internal mutation state. They carry the original diagnostic
where available, otherwise a stable `SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE`
diagnostic. Diagnostic admission failures preserve code/message/remediation/time
from their original ErrorContext in ChangeDiagnostic::NotAccepted. Existing
DiagnosticSummary stays unchanged; if only a legacy summary is available, use
the explicit operation-specific remediation above and retain its message/time,
rather than claim lost remediation was preserved.

## Behavioral contract

- Baseline comes from resolved LoggerConfig before construction. #96 may later
  supply it but its loader is not a dependency. Never re-read configuration or
  persist an override during mutation. Restart starts from baseline.
- One override exists per logger. Requests may increase verbosity or reduce an
  existing override down to baseline; requests below baseline return
  BelowBaseline without mutation. Off follows the same ordering. Reset removes
  the override. Repeating the effective level is Unchanged, without a diagnostic
  or revision increment. No timer, lease stack or automatic expiry is promised;
  the host serializes multiple UI requests and explicitly resets on session end.
- LevelOwner is opaque, non-Clone and Send, owns mutation authority, and retains
  only a weak reference to per-logger control state, never a writer sender, a
  Logger, or shutdown ownership. Logger shutdown/drop marks state stopped even
  if the capability outlives it; subsequent mutation returns Stopped. It cannot
  delay final writer shutdown or manufacture a new logger. Dropping it leaves
  the effective level until shutdown; hosts must explicitly reset. LogGuard
  remains sole bridge lifecycle owner and retains this capability internally.
  Python owned mode exposes owner operations; attached Python and TypeScript
  request changes through an application-owned handler, never LogControl.
- Core `LevelLifecycle` and the bridge's proposed `LifecyclePhase` are distinct
  state machines. Core's crate-private `Running`/`Stopping`/`Stopped` only
  governs whether a LevelOwner may mutate its logger. Bridge completion tracks
  its own installed facade/coordinator: `Running`, `Stopping`, `Stopped`, or
  `Failed`. Bridge `Failed` records unconfirmed helper/coordinator completion
  (for example HelperSpawn or HelperLost); it does **not** assert that the core
  logger stopped, and it is never projected backwards as a fourth core state.
  Conversely, core Stopped does not prove a bridge helper joined. A shutdown
  wait timeout is not terminal: it remains observable until the retained bridge
  completion becomes Stopped or Failed.
- A committed change has one linearization point shared with authoritative core
  admission and stopping transitions. A submission overlapping the change may
  use either revision; a submission starting after successful return sees the
  new state. Already admitted records are not retroactively filtered or purged.
  Health returns a coherent baseline/effective/revision snapshot. The initial
  revision is zero. Read-only level_state returns the last committed snapshot
  even after stop; internal poison does not panic or fabricate a new revision.
  Mutation on poisoned state returns Unavailable without modifying the snapshot. Serialize
  mutations against shutdown; failed requests leave state unchanged. Revision
  overflow returns Unavailable without wrapping or panicking.
- THRESHOLD and log::set_max_level are separate atomics, not one transaction.
  Remove independent bridge policy in favor of shared core state. Any facade
  fast filter must be a conservative ceiling throughout transitions; it must
  never reject an event the current core level allows. A fixed runtime Trace
  ceiling with core-owned effective filtering is the selected implementation.
  enabled() consults the current core snapshot; submission rechecks core state.
  Document the added enabled-path synchronization cost. Only the installed bridge
  changes process-global facade state; standalone core loggers do not.
- Compile-time max_level/release_max_level features cannot be undone at runtime.
  BTIT's supported release feature graph must retain Debug/Trace sites required
  by elevation. If a bridge build cannot honor a requested level, return
  UnsupportedLevel with no mutation; do not claim that the facade captured
  events compiled out of the executable. The bridge checks resolved
  `log::STATIC_MAX_LEVEL` before mutation. Bridge initialization rejects a
  configured baseline above that cap with InitError::UnsupportedLevel before
  installing the global facade; reset therefore always has a supported baseline. Independent core loggers support
  constructed events through Trace regardless of facade compile features and
  never read/write global facade settings. Application calls to log::set_max_level
  after installation are unsupported because they bypass bridge ownership.
- Each actual change attempts one structured Info diagnostic containing old,
  new, baseline, revision and typed source. Use a dedicated internal admission
  path that bypasses only the level threshold (including Off), preserving
  redaction, sink policy and queue bounds. This path is private to the core
  LevelOwner implementation; expose no public threshold-bypass operation.
  The core stamps its configured service/identity, target `sc_observability`,
  action `logging.level_changed`, and fields `previous_level`, `effective_level`,
  `configured_level`, `level_revision`, `source`; no user-supplied fields enter
  this event. Never recurse through the facade.
  Accepted means queue admission, not persistence; sink filters or writer
  failures can still prevent storage. Diagnostic failure produces Changed with
  NotAccepted, never rollback, an exception, or a false transition failure.
  Failure of secondary health accounting preserves the original outcome.
- No unconditional lossless guarantee is made. Existing queue-full, writer and
  shutdown failures remain typed and accounted for. A level change must not
  introduce extra silent drops or bypass application validation/redaction.

## Published compatibility

Keep existing LoggerConfig and LoggingHealthReport fields, legacy Result types,
constructors, trait implementability, enum exhaustiveness, and serialized forms
unchanged. level_state is a new accessor, including on Logger<Stopped>; runtime
state is internal. New construction methods opt in to owner capability without
changing Logger::new, LoggerBuilder::new or build. No published type gains a
required field or trait method. The existing default level and legacy filtered
Ok(()) behavior stay unchanged. B.1b's distinct improved entry points may expose
richer admission/errors additively; the bridge's new EmitOutcome must preserve
Accepted versus Filtered independently. New level types get documented stable
codes/remediation and explicit wire conversions, not a rewrite of legacy Serde.

## Non-closure

No #96 settings loader, persistence, automatic expiry, multiple override leases,
UI authorization policy, sc-runtime worker topology, or post-copy bridge redesign.
