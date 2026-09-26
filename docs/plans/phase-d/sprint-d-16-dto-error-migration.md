# d-16: DTO error migration

## Plan metadata

- Wave: 14
- Branch: `sprint/d-16-dto-error-migration`
- PR target: `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-dto/**`
  - ``

## Deliverables

1. Migrate the DTO conversion, wire, and error-code wrappers in sc-observability-dto to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-dto.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

One-boundary boundary sprint.

## Acceptance criteria

boundary:sc-observability-dto: implementation is production ready; run target tests, workspace build, and boundary validation.
