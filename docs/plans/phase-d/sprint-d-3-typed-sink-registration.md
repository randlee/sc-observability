---
id: D.3
status: planned
branch: sprint/d-3-typed-sink-registration
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/sprint/d-3-typed-sink-registration
depends_on: []
relation: parallel_safe
assignee: cobs
model_class: terra
requirements: ["LOG-004", "LOG-015", "PHB-003", "PHB-004"]
owned_docs: ["docs/logging/d-3-typed-sink-registration.md"]
adrs: ["ADR-012"]
closure_type: integration
target_boundary: "typed sink registration compatibility"
owned_paths: ["crates/sc-observability/src/builder.rs", "crates/sc-observability/src/typed.rs", "crates/sc-observability-types/src/errors.rs", "crates/sc-observability/tests/typed_registration.rs", "docs/api-approvals/d-3-*.json", "docs/logging/d-3-typed-sink-registration.md"]
---

# D.3 — Typed sink registration ergonomics (#203)

## Goal and dependency

In a compatible 1.x release, remove the consumer need to use deprecated `LogSinkError` or
manually call `typed::legacy_sink()` when registering a `TypedLogSink`.
This additive surface is checked against published 1.4.1. It is a deliberate
one-release bridge. D.4 may remove this bridge only in its later 2.0 release.

## Deliverables

1. Add `SinkRegistration::typed(Arc<dyn TypedLogSink>) -> Self` that applies
   the existing legacy adapter internally and preserves registration metadata.
2. Add `LoggerBuilder::register_typed_sink(...)` mirroring the retained
   registration flow, including chaining/error behavior and sink health/flush
   behavior.
3. Update `LogSinkError` deprecation/rustdoc and the additive API/migration
   sections in `docs/logging/d-3-typed-sink-registration.md` to
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

## Owned Paths and Exact Targets

- `crates/sc-observability/src/builder.rs`
- `crates/sc-observability/src/typed.rs`
- `crates/sc-observability-types/src/errors.rs`
- `crates/sc-observability/tests/typed_registration.rs`
- `docs/api-approvals/d-3-*.json`
- `docs/logging/d-3-typed-sink-registration.md`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

Parallel-safe with the other additive logging sprints: this sprint owns its separate additive document and scoped API approval. D.4 owns linking these documents from the shared API design. No shared normative document or release baseline is edited here.

Implement both inherent registration entry points in core `builder.rs` and
reuse `typed.rs`; update the existing error deprecation at its types owner.
The same-crate inherent impl for `SinkRegistration` avoids editing D.1-owned `lib.rs`.

## Non-closure

The retained `LogSink`/`LogSinkError` ABI is not removed here; the separate
D.4 major-version migration owns any later removal.
