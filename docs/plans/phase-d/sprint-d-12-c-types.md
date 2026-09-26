# d-12: Types and OTLP contract

## Plan metadata

- Wave: 1
- Branch: `sprint/d-12-c-types`
- PR target: `integrate/phase-d`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `crates/sc-observability-types/src/errors.rs`
  - `crates/sc-observability-types/src/events.rs`
  - `crates/sc-observability-types/src/metric.rs`
  - `crates/sc-observability-types/src/span.rs`
  - `crates/sc-observability-types/src/validation.rs`
  - `crates/sc-observability-types/tests/neutral_contracts.rs`
  - `crates/sc-observability-otlp/src/lib.rs`
  - `crates/sc-observability-otlp/Cargo.toml`

## Deliverables

1. Add the nine `#[non_exhaustive]` typed error enums carrying `Box<ErrorContext>` from D.4, with the public 2.0 error contract.
2. Add neutral signal model types, serde contracts, and validation errors from D.5.
3. Add `PositiveDuration`, `LifecycleBounds`, `RetryPolicy`, `BoundedPercent`, transport bounds, lifecycle config fields, the exporter trait, `ExporterSet`, factory, and fake exporter fixture from D.6.
4. Hoist OTLP `lib.rs` module declarations and Cargo feature declarations required by all implementation modules.

```rust
pub trait Exporter: Send + Sync { fn export(&self, batch: &[Signal]) -> Result<(), ExportError>; }
pub struct ExporterSet { /* configured exporters */ }
```

## This Sprint Does Not Close

No production exporter adapter, signal projection, wrapper migration, or public API composition is implemented here.


## Design

Contract owner for shared types and OTLP module declarations.

## Acceptance criteria

boundary:sc-observability-types: contract types and fixture compile; run workspace build and boundary validation.
