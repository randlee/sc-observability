# d-18: Integration and public API

## Plan metadata

- Wave: 16
- Branch: `sprint/d-18-integration-and-public-api`
- PR target: `sprint/d-17-log-macros-error-migration`
- Blocked by: `obs-d-2-sanity`, `obs-d-3-sanity`, `obs-d-16-sanity`, `obs-d-1-sanity`, `obs-d-10-sanity`, `obs-d-17-sanity`, `obs-d-14-sanity`, `obs-d-6-sanity`, `obs-d-4-sanity`, `obs-d-5-sanity`, `obs-d-15-sanity`, `obs-d-8-sanity`, `obs-d-7-sanity`
- Owned paths:
  - `Cargo.lock`
  - `release/release-inventory.json`
  - `release/public-api-major-breaks.toml`
  - `docs/api-approvals/**`
  - `bindings/**`

## Deliverables

1. Compile the workspace against the D.12 enums and remove legacy wrappers and `error_wrapper!` usage from the remaining composition surface.
2. Construct `ExporterSet`, add public re-exports, and wire feature-gated builders to the completed exporter implementations.
3. Apply the 2.0 version bump and update `release/public-api-major-breaks.toml`, API-approval JSON, and `release/release-inventory.json`.
4. Update bindings error-surface integration where required and verify public API evidence.

## This Sprint Does Not Close

Dual-path hermetic conformance and OTLP documentation are closed by D.9; the Python regression guard is closed by D.11.


## Design

## Implementation targets

- `Cargo.lock`: lock the completed workspace dependency graph (deliverable 1).
- `release/release-inventory.json` and `release/public-api-major-breaks.toml`: record the 2.0 release/API evidence (deliverable 3).
- `docs/api-approvals/**`: add the reviewed public API JSON (deliverable 3).
- `bindings/**`: re-export and translate the canonical error surface (deliverable 4).
- Composition modules: construct `ExporterSet`, re-export it, and gate builders by features (deliverables 1–2).

## Acceptance criteria

- `cargo test --workspace` passes with the canonical enums and exporter composition.
- `rg "error_wrapper!|legacy wrapper" crates bindings` returns zero obsolete wrapper constructions.
- `release/public-api-major-breaks.toml`, API approvals, and release inventory exist and are updated.
