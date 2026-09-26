# d-14: Observe error migration

## Plan metadata

- Wave: 12
- Branch: `sprint/d-14-observe-error-migration`
- PR target: `sprint/d-8-otlp-http-json-transplant`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observe/**`
  - `crates/sc-observe/tests/**`

## Deliverables

1. Migrate the ObserveError and error wrappers in sc-observe to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observe.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe

| Current type | D12 target | ErrorContext fields | Count | Tests to retype |
| --- | --- | --- | ---: | --- |
| ObservationError | `EventError::Routing` | code, source, route | 19 | `tests/routing_integration.rs`, `tests/typed_observation.rs` |
| InitError / InitFailure | `InitError::Configuration` | code, source, config_field | 22 | `tests/typed_observation.rs` |
| FlushError | `FlushError::Drain` | code, source, queue_depth | 4 | `tests/routing_integration.rs` |
| LogSinkError | `LogSinkError::Write` | code, source, sink | 7 | `tests/typed_observation.rs` |
| ProjectionError | `ProjectionError::Projection` | code, source, projector | 11 | `tests/typed_observation.rs` |
| ShutdownError | `ShutdownError::Drain` | code, source, deadline_ms | 4 | `tests/routing_integration.rs` |
| SubscriberError | `SubscriberError::Subscriber` | code, source, subscriber | 8 | `tests/typed_observation.rs` |

```rust
// crates/sc-observe/src/lib.rs:383
// before: return Err(ObservationError::RoutingFailure(Box::new(context)));
// after:  return Err(EventError::Routing { context: Box::new(context) });
```

## Implementation targets

- `crates/sc-observe/src/lib.rs`: replace the tabled public and internal constructions (deliverables 1–2).
- `crates/sc-observe/tests/routing_integration.rs` and `typed_observation.rs`: assert named variants, codes, and source identity (deliverable 3).

## Acceptance criteria

- `cargo test -p sc-observe` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observe` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
