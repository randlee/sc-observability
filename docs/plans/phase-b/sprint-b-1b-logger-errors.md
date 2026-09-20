---
id: B.1b
status: complete
branch: feature/phase-b-1b-logger-prep
base: fix/phase-b-1-provenance-integrity
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1b-logger-prep
---

# B.1b — Typed logger operations and sink interoperability

## Goal and dependencies

`must_follow` B.1a for shared failure values and B.1 for the copied bridge.
B.1c `must_follow` this sprint because observation routes invoke the logger.
Implement the preferred core logger path without changing its accepted bridge
consumer, writer ownership, level state or existing queue semantics.

For every `must_follow`, merge pushed parent development into the child before
every development/fix round; the parent PR merges before child completion.
No listed related sprint is `parallel_safe`: shared neutral contracts, runtime
call sites or release artifacts intersect.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Implement the additive methods and admission failure enums below. Convert
   logger construction, event validation, queue admission and flush implementation
   to typed failures at the production failure site; legacy methods adapt those
   results back to their exact published types. Preserve `emit`'s compatibility
   behavior, including its existing conditional flush; do not reimplement it as
   an alias that changes timing.
2. Add `TypedLogSink` and explicit adapter functions below. Built-in file/console
   sinks implement the typed trait; keep their existing `LogSink` impls backed
   by the same implementation. Preserve sink health accounting and maintenance
   error sources. Maintain only one physical write/flush per logical call.
3. Add logger-specific old/new consumer parity fixtures, source mapping and
   bridge regression coverage. Do not activate new warnings yet.

## Contract

```rust
// New neutral enums in sc-observability-types, re-exported by sc-observability.
#[non_exhaustive]
pub enum LogFailure {
    InvalidEvent(EventFailure),
    WriterDegraded(Box<ErrorContext>),
    ShutdownTimedOut(Box<ErrorContext>),
}
#[non_exhaustive]
pub enum TryLogFailure {
    InvalidEvent(EventFailure),
    QueueFull(Box<ErrorContext>),
    WriterDegraded(Box<ErrorContext>),
    ShutdownTimedOut(Box<ErrorContext>),
}
impl Logger<Running> {
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure>;
    pub fn new_with_level_owner_typed(config: LoggerConfig)
        -> Result<(Self, LevelOwner), InitFailure>;
    pub fn builder_typed(config: LoggerConfig) -> Result<LoggerBuilder, InitFailure>;
    pub fn log_typed(&self, event: LogEvent) -> Result<(), LogFailure>;
    pub fn try_log_typed(&self, event: LogEvent) -> Result<(), TryLogFailure>;
    pub fn try_log_with_outcome_typed(&self, event: LogEvent)
        -> Result<AdmissionOutcome, TryLogFailure>;
    pub fn flush_typed(&self) -> Result<(), FlushFailure>;
}
impl LoggerBuilder {
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure>;
    pub fn build_typed(self) -> Result<Logger<Running>, InitFailure>;
    pub fn build_with_level_owner_typed(self)
        -> Result<(Logger<Running>, LevelOwner), InitFailure>;
}
pub trait TypedLogSink: Send + Sync {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkFailure>;
    fn flush(&self) -> Result<(), LogSinkFailure> { Ok(()) }
    fn health(&self) -> SinkHealth;
}
pub fn legacy_sink(value: Arc<dyn TypedLogSink>) -> Arc<dyn LogSink>;
pub fn typed_sink(value: Arc<dyn LogSink>) -> Arc<dyn TypedLogSink>;
```

`TypedLogSink`, `legacy_sink` and `typed_sink` live only in the new
`sc_observability::typed` module, with no root re-exports. Existing root-glob
consumers therefore do not acquire another same-named trait method.

`AdmissionOutcome` is the B.P1 `Accepted | Filtered` enum and is not redefined.
The outcome method preserves B.P1's filtering linearization and level revision
semantics. Unit-returning old/new log methods preserve filtered `Ok(())`.
`log_typed` blocks for admission exactly as `log`; `try_log_typed` never waits
for capacity. Neither success promises persistence. New failure enums derive
Debug/PartialEq and implement Display/Error (payload source retained), but not
Serde; provide bidirectional From with their old counterparts, moving each
payload and converting only InvalidEvent. Their variant discriminant is the
classification; they need no new `ClassifiedError` impl. Existing LogError and
TryLogError remain exhaustive, structurally unchanged, supported public types.
Do not deprecate them merely because their nested EventError is deprecated.

New logger construction methods and builder build_typed/build_with_level_owner_typed
use one fallible startup implementation. Thread/coordinator spawn failure returns
InitFailure::LoggerInitialization, preserving its native source and the stable
SC_OBSERVABILITY_LOGGER_INIT_FAILED code; no new constructor relies on legacy
infallible build or panics to handle ordinary resource failure. Already-started
resources are rolled back before returning Err and no usable partial owner escapes.
Legacy LoggerBuilder::build(self) -> Logger<Running> remains unchanged and is
not newly deprecated; documentation recommends build_typed when callers need
recoverable startup outcomes. B.P1 build_with_level_owner already returns Result
and receives its additive typed counterpart.

Public query/follow/health/shutdown methods retain their signatures and are not
deprecated: they already use named outcomes or typestate. In particular,
`Logger<Running>::shutdown(self) -> Logger<Stopped>` is not changed to Result.
The new runtime-level owner methods remain on the B.P1 runtime contract; its
public acceptance remains owner-deferred to Phase B completion.

## Acceptance criteria (authoritative)

- AC1: Old and typed methods produce identical admission/filtering, ordering,
  flush and lifecycle behavior; converted failures preserve diagnostic/source data.
- AC2: Both sink adapter directions support a downstream custom sink with default
  flush and explicit flush, with one invocation and preserved health each time.
- AC3: Bridge API/behavior fixtures remain unchanged; any internal compatibility
  accommodation is isolated and recorded without altering the source import SHA.
- AC4: Every logger production constructor is typed or explicitly limited to a
  legacy compatibility boundary. Published APIs have no removed/changed signatures.

## Required validation (authoritative)

Run formatting, `cargo test --locked -p sc-observability --all-targets`, workspace
clippy/doctests, public API diff/semver/docs and the copied bridge suite. Compile unchanged root-glob-import consumers to prove the new traits do not
change method resolution. New consumers import typed::TypedLogSink explicitly;
qualification is allowed only in intentionally mixed-trait migration fixtures. Add paired
old/new tests for zero queue capacity; thread/coordinator spawn failure and
partial-start rollback without leaked threads or owner; invalid schema and mismatched service;
filtered event; full queue; disconnected/degraded writer; timeout reporting;
file/console write and flush failure; maintenance failure; fault-injection
feature on/off; custom unknown sink code; two-way adapters; source preservation;
and flush/shutdown concurrent with admission. Verify no extra writes, health
increments or blocking introduced. Test Accepted versus Filtered under level
change with B.P1's same concurrency fixture. Record `handoff-b-1b.md`.

## Paths to delete

None. Existing APIs, representations, registrations and compatibility paths remain.

## Non-closure

No API removal, new warning activation, publication or change to the accepted
bridge/control/lifecycle contract. Observation and telemetry migrations have their
own sprints; B.1e owns warning rollout and adoption guidance.
