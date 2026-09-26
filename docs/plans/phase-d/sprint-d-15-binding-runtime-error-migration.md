# d-15: Binding runtime error migration

## Plan metadata

- Wave: 13
- Branch: `sprint/d-15-binding-runtime-error-migration`
- PR target: `sprint/d-14-observe-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-binding-runtime/**`
  - ``

## Deliverables

1. Migrate the Callback, conversion, coordinator, operation, spawn, sync, and timer errors in sc-observability-binding-runtime to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-binding-runtime.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe\n\n| Current wrapper or error | D.12 target | ErrorContext fields |\n| --- | --- | --- |\n| crate-local diagnostic wrapper | canonical non-exhaustive error enum | operation, source, diagnostic code |\n\n## Implementation targets\n\n- Each owned source module: replace local wrapper construction with the D.12 enum variant (deliverable 1).\n- Each owned test module: assert the typed variant and source context (deliverable 3).\n\n

## Acceptance criteria

- 
running 1 test
test tests::contract_matrix ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s


running 2 tests
test bridge_roundtrip_preserves_host_ownership ... ok
test core_roundtrip_and_surviving_read_handles ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 4 tests
test crates/sc-observability-binding-runtime/src/lib.rs - CoreLoggerBackend (line 65) - compile fail ... ok
test crates/sc-observability-binding-runtime/src/lib.rs - BridgeControlBackend (line 91) - compile fail ... ok
test crates/sc-observability-binding-runtime/src/lib.rs - CoreLoggerBackend (line 70) - compile fail ... ok
test crates/sc-observability-binding-runtime/src/lib.rs - CoreLoggerOwner (line 81) - compile fail ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s passes with migrated typed-error assertions.\n-  returns zero remaining legacy constructions.\n- The crate's sprint doc and typed-error tests exist and name the D.12 enum.
