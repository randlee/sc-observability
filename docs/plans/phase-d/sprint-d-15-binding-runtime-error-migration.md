# d-15: Binding runtime error migration

## Plan metadata

- Wave: 2
- Branch: `sprint/d-15-binding-runtime-error-migration`
- PR target: `sprint/d-12-c-types`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-binding-runtime/**`
  - `docs/plans/phase-d/**`
  - ``

## Deliverables

1. Migrate the Callback, conversion, coordinator, operation, spawn, sync, and timer errors in sc-observability-binding-runtime to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-binding-runtime.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

One-boundary boundary sprint.

## Acceptance criteria

boundary:sc-observability-binding-runtime: implementation is production ready; run target tests, workspace build, and boundary validation.
