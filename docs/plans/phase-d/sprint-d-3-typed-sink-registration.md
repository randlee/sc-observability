# d-3: Typed sink registration ergonomics (#203)

## Plan metadata

- Wave: 2
- Branch: `sprint/d-3-typed-sink-registration`
- PR target: `sprint/d-13-c-log`
- Blocked by: `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability/src/builder.rs`
  - `crates/sc-observability/tests/typed_registration.rs`
  - `docs/logging/d-3-typed-sink-registration.md`

## Goal and dependency

Implement the D.13 typed-sink adapter contract in the existing logging builder.

## Deliverables

1. Implement the D.13 `SinkRegistration::typed` adapter path using the existing legacy adapter internally while preserving registration metadata.
2. Implement the D.13 `LoggerBuilder::register_typed_sink` registration flow, chaining, failure, sink-health, and flush behavior.
3. Update deprecation/rustdoc, migration documentation, and public consumer fixtures for the retained typed registration surface.

## Non-closure

The signatures belong to D.13; 2.0 wrapper removal belongs to D.18.


## Design

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



## Acceptance criteria

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


