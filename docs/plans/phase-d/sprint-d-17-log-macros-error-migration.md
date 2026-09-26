# d-17: Log consumer error migration

## Plan metadata

- Wave: 15
- Branch: `sprint/d-17-log-macros-error-migration`
- PR target: `sprint/d-16-dto-error-migration`
- Blocked by: `obs-d-12-sanity`, `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability-log-consumer-check/src/lib.rs`
  - `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
  - `examples/atm-adapter-example/src/main.rs`
  - `examples/custom-sink-example/src/main.rs`
  - `examples/tauri-logging/src-tauri/src/main.rs`

## Deliverables

1. Migrate the log-consumer check and the three named consumer examples to D12/D13 typed error signatures.
2. Retype consumer tests and example compile checks to preserve typed error handling.
3. Update the consumer migration sprint documentation.

## This Sprint Does Not Close

Log-crate internal construction is D16; bridge implementation is D2; workspace API/release evidence is D18.

## Design

## Migration recipe

| Current type | Contract target | ErrorContext fields | Count | Tests to retype |
| --- | --- | --- | ---: | --- |
| FlushError | `FlushError::Drain` | code, source, queue_depth | 4 | `crates/sc-observability-log-consumer-check/tests/control_consumer.rs` |
| LogSinkError | `LogSinkError::Write` | code, source, sink | 4 | example compile checks |
| TelemetryError | `ExportError::Lifecycle` | code, source, backend | 3 | `examples/atm-adapter-example/src/main.rs` |
| ShutdownError | `ShutdownError::Drain` | code, source, deadline_ms | 8 | `examples/tauri-logging/src-tauri/src/main.rs` |

```rust
// examples/custom-sink-example/src/main.rs
// before: return Err(LogSinkError(context));
// after:  return Err(LogSinkError::Write { context });
```

## Implementation targets

- `crates/sc-observability-log-consumer-check/src/lib.rs` and `tests/control_consumer.rs`: migrate `FlushError` handling (deliverables 1–2).
- `examples/atm-adapter-example/src/main.rs`, `custom-sink-example/src/main.rs`, and `tauri-logging/src-tauri/src/main.rs`: migrate the tabled consumer types (deliverable 1).

## Acceptance criteria

- `cargo test -p sc-observability-log-macros` passes typed-error assertions (deliverables 1 and 3).
- `rg "error_wrapper!|LegacyError" crates/sc-observability-log-macros` returns zero local legacy constructions (deliverable 2).
- The generated sprint doc names the D12 enum mapping and retargeted tests (deliverable 4).
