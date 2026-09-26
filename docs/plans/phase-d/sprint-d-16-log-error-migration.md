# d-16: Log error migration

## Plan metadata

- Wave: 14
- Branch: `sprint/d-16-log-error-migration`
- PR target: `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-log/src/control.rs`
  - `crates/sc-observability-log/src/error.rs`
  - `crates/sc-observability-log/src/error_codes.rs`
  - `crates/sc-observability-log/src/handle.rs`
  - `crates/sc-observability-log/src/mapping.rs`
  - `crates/sc-observability-log/tests/api_freeze.rs`
  - `crates/sc-observability-log/tests/flush_single_flight.rs`
  - `crates/sc-observability-log/tests/init_runtime_start.rs`
  - `crates/sc-observability-log/tests/shutdown_timeout.rs`
  - `crates/sc-observability-log/tests/static_level_cap.rs`

## Deliverables

1. Migrate `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` uses in the named sc-observability-log modules to D12 named variants.
2. Retype the five named log-crate regression tests to assert the D12 variant, stable code, and `ErrorContext` source identity.
3. Update the log error-migration sprint documentation.

## This Sprint Does Not Close

The bridge API is D2, the typed sink signature is D13, consumer/examples are D17, and workspace integration is D18.

## Design

## Migration recipe

| Current type | D12 target | ErrorContext fields | Count | Tests to retype |
| --- | --- | --- | ---: | --- |
| IdentityError | `IdentityError::Process` | code, source, process | 11 | `tests/init_runtime_start.rs` |
| InitError | `InitError::Configuration` | code, source, config_field | 40 | `tests/api_freeze.rs`, `tests/init_runtime_start.rs` |
| FlushError | `FlushError::Drain` | code, source, queue_depth | 50 | `tests/flush_single_flight.rs` |
| ShutdownError | `ShutdownError::Timeout` | code, source, deadline_ms | 38 | `tests/shutdown_timeout.rs`, `tests/static_level_cap.rs` |

```rust
// crates/sc-observability-log/src/control.rs
// before: Err(FlushError(Box::new(context)))
// after:  Err(FlushError::Drain { context: Box::new(context) })
```

## Implementation targets

- `src/control.rs`, `error.rs`, `error_codes.rs`, `handle.rs`, and `mapping.rs`: replace only the tabled constructions (deliverable 1).
- `tests/api_freeze.rs`, `flush_single_flight.rs`, `init_runtime_start.rs`, `shutdown_timeout.rs`, and `static_level_cap.rs`: assert variant/source/code (deliverable 2).

## Acceptance criteria

- `cargo test -p sc-observability-dto` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-dto` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
