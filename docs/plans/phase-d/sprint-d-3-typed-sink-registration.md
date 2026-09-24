---
id: D.3
status: complete
branch: feature/phase-d-3-typed-sink-registration
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-3-typed-sink-registration
depends_on: [D.1]
relation: must_follow
owned_docs: [docs/api-design.md, docs/migrate-error-api.md]
---

# D.3 — Typed sink registration ergonomics (#203)

## Goal and dependency

After D.1, remove the 1.x consumer need to use deprecated `LogSinkError` or
manually call `typed::legacy_sink()` when registering a `TypedLogSink`.
This additive surface is checked against published 1.4.1. It is a deliberate
one-release bridge: D.4 removes the duplicate typed/legacy split in 2.0.

## Deliverables

1. Add `SinkRegistration::typed(Arc<dyn TypedLogSink>) -> Self` that applies
   the existing legacy adapter internally and preserves registration metadata.
2. Add `LoggerBuilder::register_typed_sink(...)` mirroring the retained
   registration flow, including chaining/error behavior and sink health/flush
   behavior.
3. Update `LogSinkError` deprecation/rustdoc and `migrate-error-api.md` to
   name `TypedLogSink`, `SinkRegistration::typed`, and builder registration.
4. Add public-only consumer fixtures implementing `TypedLogSink` without
   `#[allow(deprecated)]`, proving write, explicit flush, health, registration,
   and typed failure fidelity. Retain legacy `LogSink` compatibility fixtures.

## Acceptance criteria

- A consumer can register `Arc<dyn TypedLogSink>` through both public APIs
  without importing or calling `legacy_sink` and with no deprecated warning.
- The adapter performs one write/flush per request and preserves typed error
  diagnostic code, source, and health state.
- Existing legacy custom sinks still register and execute unchanged in 1.x.

## Required validation

- Focused core and external consumer tests, with warnings denied for the new
  typed consumer fixture.
- `cargo test --workspace --locked` and public API/semver validation.

## Non-closure

The retained `LogSink`/`LogSinkError` ABI is not removed here; D.4 owns the
2.0 removal and migration. D.4's inventory must disposition
`SinkRegistration::typed`, `register_typed_sink`, `TypedLogSink`, and
`legacy_sink` together so only one canonical sink path remains.
