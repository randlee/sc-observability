---
id: B.P1
status: proposed
branch: feature/phase-b-p1-runtime-core
base: develop
---

# B.P1 — Add per-logger runtime level ownership

## Goal and dependencies

Owner: sc-observability core team. Implement issue #97's core capability while
preserving every published interface. This is a pre-copy prerequisite; B.1
remains the first migration sprint. `must_follow` reviewed acceptance of the
[runtime contract](runtime-level-contract.md). B.P2 `must_follow` B.P1 because
it qualifies and stages these exact artifacts; B.7 alone publishes them. No parallel-safe public-contract work is
claimed. Pushed parent development triggers merge-forward before each child
round; parent PR merges before child completion.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Add the runtime contract's OperationDiagnostic, LevelState, LevelChangeSource, ChangeDiagnostic,
   LevelChange, LevelChangeError and AdmissionOutcome to sc-observability-types, with central
   codes/remediation; add LevelOwner and construction/accessors to
   sc-observability. Implement its complete baseline, reset, revision, lifetime,
   diagnostic and lifecycle semantics in builder/runtime/health paths. Add a
   fallible internal writer-start primitive used directly by the new constructors;
   thread creation failure returns InitError with LOGGER_INIT_FAILED plus its
   original source/remediation, never a caught expect panic. Existing build's
   signature remains intact; new APIs never route through its infallible path.
2. Replace config.level admission filtering with per-logger effective state in
   both existing log and try_log paths without changing their signatures or
   filtered Ok(()) behavior. Add try_log_with_outcome over the same implementation
   so the bridge distinguishes Accepted from Filtered without duplicate policy.
   Serialize state changes with filtering and stopping;
   level_state returns a coherent retained snapshot. Keep queue waits, writer
   work and user callbacks outside the state critical section. A stale owner
   cannot retain the logger, sender or writer, or postpone shutdown.
3. Add synchronized fault/concurrency tests, published-consumer/Serde fixtures,
   scoped additive API review evidence, and execution record
   `docs/plans/phase-b/handoff-b-p1.md`. Update API-design/requirements/ADR
   implementation status only after behavior is verified. No artifact is
   published by this sprint.

## Boundary signatures

The complete enum payloads and behavior are incorporated from the
[runtime contract](runtime-level-contract.md#proposed-api-and-values).
QA reviews that reference together with the following owned boundaries:

```rust
impl Logger<Running> {
    pub fn try_log_with_outcome(&self, event: LogEvent)
        -> Result<AdmissionOutcome, TryLogError>;
    pub fn new_with_level_owner(config: LoggerConfig)
        -> Result<(Self, LevelOwner), sc_observability_types::InitError>;
}
impl LoggerBuilder {
    pub fn build_with_level_owner(self)
        -> Result<(Logger<Running>, LevelOwner), sc_observability_types::InitError>;
}
impl<State> Logger<State> {
    pub fn level_state(&self) -> LevelState;
}
impl LevelOwner {
    pub fn elevate_level(&mut self, level: LevelFilter, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
    pub fn reset_level(&mut self, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
}
```

Existing constructors stay unchanged. Core logging does not acquire a `log`
facade dependency or change any process-global filter. LevelOwner carries weak
control-state authority; it does not own a runtime. Separate logger instances
have separate state. Runtime synchronization must not invoke custom redactors
while holding the state lock. A submission overlapping mutation may use either
revision; one starting after successful return uses the committed revision.

## Acceptance criteria (authoritative)

- AC1: Baseline/elevate/reduce-to-baseline/reset/repeat/Off and revision overflow
  match the contract; Accepted/Filtered agree with legacy Ok(()) and actual
  queue contents; injected writer-thread creation failure returns InitError
  without process panic or a leaked worker; failed requests preserve state and typed diagnostics.
- AC2: Deterministic races prove coherent snapshots, post-return filtering,
  shutdown serialization, stale-owner Stopped outcomes and isolated independent
  loggers. Holding/dropping LevelOwner cannot keep the writer alive.
- AC3: One bounded Info diagnostic is attempted per actual change, including
  Warn/Error/Off; redaction and queue policy apply. Diagnostic admission failure
  remains Changed/NotAccepted, never rollback, panic or recursive logging.
- AC4: Existing consumer source, traits, default behavior, enum matching and
  serialized fixtures remain compatible; no approved additive diff hides a
  breaking signature/layout/Serde change. The new core path works through Trace
  even when an unrelated log facade is capped or absent.

## Required validation (authoritative)

Run workspace tests/doctests, formatting, clippy, dependency-boundary checks,
public API diff/semver/docs checks and docs consistency. Add deterministic
transition/admission/shutdown tests, queue saturation, failed sink and diagnostic
accounting, redaction, overflow and owner-outliving-logger tests. Compile/run an
unchanged published-baseline consumer and round-trip its existing serialized
fixtures. Attach scoped crate-specific approvals and exact results to the
handoff; tool failure is not an API approval. Execute debug and release tests.

## Paths to delete

None. Replace internal config.level filtering in place; remove no public API.

## Non-closure

No publication, BTIT bridge implementation, DTO/bindings, settings loader,
legacy deprecation (#92), automatic expiry or sc-runtime topology. B.P2 owns
publication; B.P3 owns BTIT integration acceptance.
