# d-14: Observe error migration

## Plan metadata

- Wave: 12
- Branch: `sprint/d-14-observe-error-migration`
- PR target: `sprint/d-8-otlp-http-json-transplant`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observe/**`
  - `crates/sc-observe/tests/**`
  - `docs/plans/phase-d/**`

## Deliverables

1. Migrate the ObserveError and error wrappers in sc-observe to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observe.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

One-boundary boundary sprint.

## Acceptance criteria

boundary:sc-observe: implementation is production ready; run target tests, workspace build, and boundary validation.
