# d-17: Log macros error migration

## Plan metadata

- Wave: 15
- Branch: `sprint/d-17-log-macros-error-migration`
- PR target: `sprint/d-16-dto-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-log-macros/**`

## Deliverables

1. Migrate the event, fields, and instrument macro diagnostics in sc-observability-log-macros to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-log-macros.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe

| Current wrapper/error | D12 enum variant | ErrorContext fields | Call-site count on develop | Tests to retype |
| --- | --- | --- | --- | --- |
| macro parse diagnostics | `EventError::Context` | `code, source, macro_site` | `rg -n 'macro' crates` recorded before edit | `crates/sc-observability-log-macros/src/fields.rs` |

```rust
// before
return Err(LegacyError::from(context));
// after
return Err(EventError::Context(Box::new(context)));
```

## Implementation targets

- `crates/sc-observability-log-macros/src/fields.rs`: replace the listed construction sites and preserve `ErrorContext` source identity (deliverable 1).
- `crates/sc-observability-log-macros/src/fields.rs`: delete local wrappers only after each call site is typed (deliverable 2).
- `crates/sc-observability-log-macros/src/fields.rs`: add variant, stable diagnostic, and source-context assertions (deliverable 3).
- Sprint documentation: record the migrated surface (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observability-log-macros` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-log-macros` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
