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

| Current type | D12 target | ErrorContext fields | Count | Tests to retype |
| --- | --- | --- | ---: | --- |
| FlushError | `FlushError::Drain` | code, source, queue_depth | 4 | `crates/sc-observability-binding-runtime/src/tests.rs` |
| LogSinkError | `LogSinkError::Write` | code, source, sink | 3 | `crates/sc-observability-binding-runtime/src/tests.rs` |
| TryLogFailure | `LogSinkError::Write` | code, source, sink | 2 | `crates/sc-observability-binding-runtime/src/conversion.rs` |

```rust
// crates/sc-observability-binding-runtime/src/tests.rs:675
// before: crate::conversion::bridge_flush(FlushError::Logger { .. })
// after:  crate::conversion::bridge_flush(FlushError::Drain { context })
```

## Implementation targets

- `src/conversion.rs`, `src/lib.rs`, and `src/tests.rs`: replace the tabled construction/conversion paths (deliverables 1–2).
- `src/tests.rs`: assert variant, code, and source context (deliverable 3).

## Acceptance criteria

- `cargo test -p sc-observability-binding-runtime` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-binding-runtime` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
