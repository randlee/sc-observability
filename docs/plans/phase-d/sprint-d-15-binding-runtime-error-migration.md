# d-15: Binding runtime error migration

## Plan metadata

- Wave: 13
- Branch: `sprint/d-15-binding-runtime-error-migration`
- PR target: `sprint/d-14-observe-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-binding-runtime/**`

## Deliverables

1. Migrate the Callback, conversion, coordinator, operation, spawn, sync, and timer errors in sc-observability-binding-runtime to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-binding-runtime.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe

| Current wrapper/error | D12 enum variant | ErrorContext fields | Call-site count on develop | Tests to retype |
| --- | --- | --- | --- | --- |
| binding runtime callback/coordinator errors | `InitError::Context` | `code, source, operation` | `rg -n 'binding' crates` recorded before edit | `crates/sc-observability-binding-runtime/src/lib.rs` |

```rust
// before
return Err(LegacyError::from(context));
// after
return Err(InitError::Context(Box::new(context)));
```

## Implementation targets

- `crates/sc-observability-binding-runtime/src/lib.rs`: replace the listed construction sites and preserve `ErrorContext` source identity (deliverable 1).
- `crates/sc-observability-binding-runtime/src/lib.rs`: delete local wrappers only after each call site is typed (deliverable 2).
- `crates/sc-observability-binding-runtime/src/lib.rs`: add variant, stable diagnostic, and source-context assertions (deliverable 3).
- Sprint documentation: record the migrated surface (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observability-binding-runtime` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-binding-runtime` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
