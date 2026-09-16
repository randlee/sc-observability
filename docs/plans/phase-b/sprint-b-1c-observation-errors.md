---
id: B.1c
status: proposed
branch: feature/phase-b-1c-errors
base: develop
---

# B.1c — Typed observation construction and lifecycle failures

## Goal and dependencies

`must_follow` B.1b because observation construction/flush delegates to Logger,
and B.1a for neutral subscriber/projector adapters. B.1d `must_follow` this sprint
because telemetry projectors integrate with the accepted observation contracts.

For every `must_follow`, merge pushed parent development into the child before
every development/fix round; the parent PR merges before child completion.
No listed related sprint is `parallel_safe`: shared neutral contracts, runtime
call sites or release artifacts intersect.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Implement the methods below over the same observation runtime, using typed
   init/flush/shutdown failure production and legacy boundary adapters. Preserve
   the existing ObservationError routing enum and emit signature unchanged.
2. Exercise B.1a's typed subscriber/projector adapters through real registrations;
   migrate first-party routes to typed implementations. No required methods,
   fields or variants are added to existing registrations/configs/traits.
3. Add exact construction/lifecycle and custom route parity tests plus the
   observation section of the checked source inventory.

## Contract

```rust
impl ObservabilityConfig {
    pub fn default_for_typed(tool_name: ToolName, log_root: PathBuf)
        -> Result<Self, InitFailure>;
    pub fn service_name_typed(&self) -> Result<ServiceName, InitFailure>;
}
impl Observability {
    pub fn new_typed(config: ObservabilityConfig) -> Result<Self, InitFailure>;
    pub fn flush_typed(&self) -> Result<(), FlushFailure>;
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure>;
}
impl ObservabilityBuilder {
    pub fn build_typed(self) -> Result<Observability, InitFailure>;
}
```

`builder`, registration methods, `emit<T>`, health and ObservationError remain
supported unchanged. Typed routes use `legacy_subscriber`/`legacy_*_projector`
from B.1a when passed to existing registrations; the private wrapper forwards
once and converts only its error. Failure aggregation retains the same stable
routing error, health accounting and individual source diagnostics. Shutdown
remains idempotent and owned by the same runtime. Existing unimplemented queue
capacity is not introduced by this migration.

## Acceptance criteria (authoritative)

- AC1: Every listed recommended method is callable without deprecated public
  types and shares the legacy operation's runtime and error behavior.
- AC2: Unchanged custom subscribers/projectors and typed adapters execute through
  the real router with identical filtering, ordering and failure aggregation.
- AC3: Source/API review confirms no changed trait implementations, registrations,
  serializer shapes, lifecycle outcomes or bridge public surfaces.

## Required validation (authoritative)

Run formatting, `cargo test --locked -p sc-observe --all-targets`, workspace
clippy/doctests and public API diff/semver/docs checks. Paired legacy/typed tests
cover invalid tool/service name; empty route builder; downstream logger init
failure; eligible/ineligible filters; no routes for emitted type; one successful
route plus one failure; all routes failing; each projector output family;
custom and wrong-family diagnostic codes; flush failure; shutdown followed by
emit; and repeated/concurrent shutdown. Assert exact routing outcomes, context
retention and no double invocation. Record `handoff-b-1c.md`.

## Paths to delete

None. Existing APIs, representations, registrations and compatibility paths remain.

## Non-closure

No routing redesign, new queue, changed ObservationError or public trait,
publication, warning activation, or bridge API migration.
