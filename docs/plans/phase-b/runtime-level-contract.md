---
status: proposed_for_public_api_review
issue: 97
---

# Runtime level elevation — pre-copy contract and core prerequisite

## Ownership and sequencing

This is an explicit prerequisite to B.1, not a post-copy implementation task.
sc-observability implements, reviews and publishes the additive core capability
first. BTIT implements the corresponding bridge API and behavior against that
released core and the accepted target contract, closes its critical review, and
hands off the working reference. B.1 remains the first migration sprint and
copies it mechanically. Do not mark this prerequisite complete from plan approval
alone; record the core release and accepted BTIT source in the import gate.

`must_follow`: acceptance of this contract and scoped core API approval before
core implementation closure/publication; BTIT integration must follow core
registry availability; B.1 must follow accepted BTIT integration/review.
No parallel-safe relationship is claimed for these shared public contracts.
Apply the phase merge-forward and parent-merge rules to their development PRs.

## Deliverables (authoritative)

1. Add one core-owned runtime level state shared by all producer paths. Retain
   LoggerConfig.level as the immutable configured baseline. Core admission uses
   effective state rather than continuing to filter against config.level.
   New owner-capability construction is additive; existing construction works
   unchanged without granting producer handles mutation rights.
2. Implement owner-only temporary override/reset, typed outcomes, lifecycle
   serialization, health snapshots and bounded best-effort change diagnostics
   under the contract below. Keep the capability non-Clone; do not add mutation
   methods to LogControl or other attached producer handles.
3. Publish the reviewed core API through the existing release process with scoped
   semver approvals and a registry-only capability consumer. BTIT then implements
   all bridge integration before B.1, including release-build filter checks.
4. Extend B.3/B.4 health projections and conformance requirements with configured
   and effective levels and the state revision. Document the host-owned command
   forwarding example. Add adoption guidance for compile-time filter limits,
   baseline/reset behavior and diagnostic failure handling.

## Proposed API and values

The following is the target direction for review, not an approved API freeze.
Core types live in the neutral types crate; owner/state implementation lives in
sc-observability. Bridge re-exports these values rather than duplicating enums.

```rust
pub enum LevelChangeSource { Application, UserRequest, DiagnosticSession }
pub struct LevelState {
    pub configured_level: LevelFilter,
    pub effective_level: LevelFilter,
    pub revision: u64,
}
pub enum ChangeDiagnostic {
    Accepted,
    NotAccepted { diagnostic: DiagnosticSummary },
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
    Unavailable { diagnostic: DiagnosticSummary },
}
impl Logger<Running> {
    pub fn new_with_level_owner(config: LoggerConfig)
        -> Result<(Self, LevelOwner), sc_observability_types::InitError>;
    pub fn level_state(&self) -> LevelState;
}
impl LoggerBuilder {
    pub fn build_with_level_owner(self) -> (Logger<Running>, LevelOwner);
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

B.1a's improved error API must include the new construction boundary in its
inventory; this prerequisite preserves existing core error conventions until
that additive migration ships. Failure enums carry stable code/remediation
accessors. Binding DTOs use explicit snake_case discriminators and their checked
u64 conversion for revision; deriving Serde alone does not establish wire
compatibility. Core level health is an additive accessor, not a breaking field
addition to a published constructible health struct. The unpublished bridge
health contract includes configured_level, effective_level and level_revision.

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
- LevelOwner owns mutation authority, not logger shutdown. Dropping it leaves
  the effective level until shutdown; hosts must explicitly reset. LogGuard
  remains sole bridge lifecycle owner and retains this capability internally.
  Python owned mode may expose owner operations; attached Python and TypeScript
  request changes through an application-owned handler, never LogControl.
- A committed change has one linearization point shared with authoritative core
  admission and stopping transitions. A submission overlapping the change may
  use either revision; a submission starting after successful return sees the
  new state. Already admitted records are not retroactively filtered or purged.
  Health returns a coherent baseline/effective/revision snapshot. Serialize
  mutations against shutdown; failed requests leave state unchanged. Revision
  overflow returns Unavailable without wrapping or panicking.
- THRESHOLD and log::set_max_level are separate atomics, not one transaction.
  Remove independent bridge policy in favor of shared core state. Any facade
  fast filter must be a conservative ceiling throughout transitions; it must
  never reject an event the current core level allows. A fixed runtime Trace
  ceiling with core-owned effective filtering is acceptable. Document the
  chosen strategy and its performance tradeoff. Only the installed bridge
  changes process-global facade state; standalone core loggers do not.
- Compile-time max_level/release_max_level features cannot be undone at runtime.
  BTIT's supported release feature graph must retain Debug/Trace sites required
  by elevation. If a bridge build cannot honor a requested level, return
  UnsupportedLevel with no mutation; do not claim that the facade captured
  events compiled out of the executable.
- Each actual change attempts one structured Info diagnostic containing old,
  new, baseline, revision and typed source. Use a dedicated internal admission
  path that bypasses only the level threshold (including Off), preserving
  redaction, sink policy and queue bounds. Never recurse through the facade.
  Accepted means queue admission, not persistence; sink filters or writer
  failures can still prevent storage. Diagnostic failure produces Changed with
  NotAccepted, never rollback, an exception, or a false transition failure.
  Failure of secondary health accounting preserves the original outcome.
- No unconditional lossless guarantee is made. Existing queue-full, writer and
  shutdown failures remain typed and accounted for. A level change must not
  introduce extra silent drops or bypass application validation/redaction.

## Acceptance criteria (authoritative)

- AC1: Baseline/elevate/reduce/reset/repeat and Off cases obey the table above;
  invalid/lifecycle/unsupported requests leave level state unchanged.
- AC2: Core direct, facade, macro, Tauri and Python paths observe the same state;
  post-return ordering and coherent health are proven with synchronized tests,
  including concurrent submissions and shutdown. Queue saturation remains a
  visible admission failure, not a failed lossless test assumption.
- AC3: Diagnostic admission at Warn/Error/Off and queue-full/writer-failure cases
  preserves the change outcome and nonfatal behavior without recursive logging.
- AC4: Debug/Trace elevation works in the supported BTIT release build; a capped
  build explicitly reports its limitation. Legacy core consumers still compile.
- AC5: Core release, registry consumer proof, target approval, BTIT integration
  and critical-review acceptance are recorded before the copy gate passes.

## Required validation (authoritative)

Run core workspace tests/doctests, formatting, clippy and existing public API,
semver, documentation and release checks. Add synchronized transition/admission/
shutdown tests, baseline and reset cases, typed error and serialization fixtures,
redaction and diagnostic-failure tests. Inspect the resolved release feature graph
and run facade/macro elevation in release mode. BTIT records focused integration
and critical-review evidence. B.3/B.4 run shared binding level-state fixtures.
Record the prerequisite's core commit/version, release proof and source handoff
in `docs/plans/phase-b/handoff-runtime-level.md` when executed.

## Paths to delete

Remove obsolete independent bridge threshold policy during BTIT integration;
record exact affected symbols/files in its accepted implementation inventory.
No published core API is removed.

## Non-closure

No #96 settings loader, persistence, automatic expiry, multiple override leases,
UI authorization policy, sc-runtime worker topology, or post-copy bridge redesign.
