# d-16: DTO error migration

## Plan metadata

- Wave: 14
- Branch: `sprint/d-16-dto-error-migration`
- PR target: `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-dto/**`

## Deliverables

1. Migrate the DTO conversion, wire, and error-code wrappers in sc-observability-dto to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-dto.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe

| Current wrapper/error | D12 enum variant | ErrorContext fields | Call-site count on develop | Tests to retype |
| --- | --- | --- | --- | --- |
| DTO conversion and wire errors | `EventError::Context` | `code, source, wire_field` | `rg -n 'DTO' crates` recorded before edit | `crates/sc-observability-dto/src/wire/primitives.rs` |

```rust
// before
return Err(LegacyError::from(context));
// after
return Err(EventError::Context(Box::new(context)));
```

## Implementation targets

- `crates/sc-observability-dto/src/wire/primitives.rs`: replace the listed construction sites and preserve `ErrorContext` source identity (deliverable 1).
- `crates/sc-observability-dto/src/wire/primitives.rs`: delete local wrappers only after each call site is typed (deliverable 2).
- `crates/sc-observability-dto/src/wire/primitives.rs`: add variant, stable diagnostic, and source-context assertions (deliverable 3).
- Sprint documentation: record the migrated surface (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observability-dto` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-dto` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
