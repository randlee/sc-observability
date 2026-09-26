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

| Current wrapper/error | D12 enum variant | ErrorContext fields | Call-site count on develop | Tests to retype |
| --- | --- | --- | --- | --- |
| sc-observe wrappers | `ObservationError::Context` | `code, source, route` | `rg -n 'sc-observe' crates` recorded before edit | `crates/sc-observe/src/lib.rs; crates/sc-observe/tests/typed_observation.rs` |

```rust
// before
return Err(LegacyError::from(context));
// after
return Err(ObservationError::Context(Box::new(context)));
```

## Implementation targets

- `crates/sc-observe/src/lib.rs; crates/sc-observe/tests/typed_observation.rs`: replace the listed construction sites and preserve `ErrorContext` source identity (deliverable 1).
- `crates/sc-observe/src/lib.rs; crates/sc-observe/tests/typed_observation.rs`: delete local wrappers only after each call site is typed (deliverable 2).
- `crates/sc-observe/src/lib.rs; crates/sc-observe/tests/typed_observation.rs`: add variant, stable diagnostic, and source-context assertions (deliverable 3).
- Sprint documentation: record the migrated surface (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observe` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observe` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
