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

## Integration targets

- `Cargo.lock`: resolve the completed workspace graph for deliverable 1.
- `release/release-inventory.json` and `release/public-api-major-breaks.toml`: record the 2.0 API/release inventory for deliverable 3.
- `docs/api-approvals/**`: approve every public D12/D13/D18 surface for deliverable 3.
- `bindings/**`: retype binding error surfaces and public exports for deliverable 4.
- Workspace re-exports: export `ExporterSet`, typed error enums, `LogSettings`, `AttachmentOptions`, and `TypedLogSink`; gate SDK and legacy builders with `otlp-sdk` and `legacy-http-json` features for deliverable 2.

## Composition procedure

1. Delete compatibility wrappers and `error_wrapper!` only after every consumer compiles against D12.
2. Construct `ExporterSet` from the feature-selected builder and expose it through the public facade.
3. Update API approvals, release inventory, and binding fixtures in the same public-surface review.


## Acceptance criteria

- `cargo test --workspace` passes with canonical enums and exporter composition (deliverable 1).
- `rg "error_wrapper!|legacy wrapper" crates bindings` returns zero remaining obsolete composition constructions (deliverable 1).
- `cargo check -p sc-observability-otlp --features otlp-sdk,legacy-http-json` passes the feature-gated builder/re-export surface (deliverable 2).
- `test -f release/public-api-major-breaks.toml && test -f release/release-inventory.json && find docs/api-approvals -name "*.json" -print -quit | grep -q .` finds the release and approval evidence (deliverable 3).
- `rg "ErrorContext|ExportError|InitError" bindings` reports the binding migration evidence (deliverable 4).
