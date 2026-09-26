# d-3: Typed sink registration ergonomics (#203)

## Plan metadata

- Wave: 6
- Branch: `sprint/d-3-typed-sink-registration`
- PR target: `sprint/d-2-host-logger-bridge`
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

## Implementation targets

- `crates/sc-observability/src/builder.rs`: implement `SinkRegistration::typed` with the D.13 adapter contract (deliverable 1) and `LoggerBuilder::register_typed_sink` chaining/error behavior (deliverable 2).
- `crates/sc-observability/tests/typed_registration.rs`: add typed registration, flush, health, and failure-fidelity assertions (deliverables 2–3).
- `docs/logging/d-3-typed-sink-registration.md`: document the retained migration path (deliverable 3).


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


