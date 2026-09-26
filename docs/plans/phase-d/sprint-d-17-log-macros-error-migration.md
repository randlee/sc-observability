# d-17: Log macros error migration

## Plan metadata

- Wave: 15
- Branch: `sprint/d-17-log-macros-error-migration`
- PR target: `sprint/d-16-dto-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-log-macros/**`
  - `docs/plans/phase-d/**`
  - ``

## Deliverables

1. Migrate the event, fields, and instrument macro diagnostics in sc-observability-log-macros to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-log-macros.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

One-boundary boundary sprint.

## Acceptance criteria

boundary:sc-observability-log-macros: implementation is production ready; run target tests, workspace build, and boundary validation.
