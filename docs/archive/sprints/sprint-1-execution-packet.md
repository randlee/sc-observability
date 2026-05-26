# Sprint 1 Execution Packet

**Sprint**: Phase 1 / Sprint 1
**Milestone**: M0-M1
**Crate focus**: `sc-observability-types`
**Depends on**:
- pre-Sprint cleanup complete
- docs consistency checks green
- dependency-ban enforcement green

## 1. Scope

Sprint 1 turns `sc-observability-types` from a compileable skeleton into the
first implementation-ready layer.

This sprint covers:
- workspace baseline confirmation for the 4-crate stack
- validated value/newtype constructors
- diagnostics and remediation surfaces
- trace/span/observation contracts
- shared health types
- shared open traits
- serialization and lifecycle tests for the public contract layer

This sprint does not cover:
- real sink behavior
- observation routing/runtime behavior
- OTLP/export behavior
- ATM production adapter behavior

## 2. Files And Modules

Expected implementation touch points:
- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/constants.rs`
- `crates/sc-observability-types/src/error_codes.rs`
- `crates/sc-observability-types/Cargo.toml`

Expected supporting updates:
- `docs/public-api-checklist.md`
- `docs/implementation-plan.md`
- `docs/test-strategy.md`

## 3. Deliverables

### M0 Workspace Baseline

- all four crates present as workspace members
- dependency order remains:
  `sc-observability-types <- sc-observability <- sc-observe <- sc-observability-otlp`
- shared constants and error-code modules remain single-source-of-truth

### M1 `sc-observability-types`

- complete constructors and validation for:
  - `ToolName`
  - `EnvPrefix`
  - `ServiceName`
  - `TargetCategory`
  - `ActionName`
  - `MetricName`
  - `TraceId`
  - `SpanId`
- complete remediation and diagnostic surfaces:
  - `ErrorCode`
  - `RecoverableSteps`
  - `Remediation`
  - `Diagnostic`
  - `DiagnosticSummary`
  - `ErrorContext`
- complete observation and telemetry-neutral contracts:
  - `Observation<T>`
  - `LogEvent`
  - `SpanRecord<S>`
  - `SpanEvent`
  - `SpanSignal`
  - `MetricRecord`
  - `TraceContext`
  - `StateTransition`
- complete shared health models and open traits

## 4. Required Tests

Required Sprint 1 tests:
- validation tests for all name newtypes
- `TraceId` and `SpanId` validation tests
- remediation construction tests
- `ErrorContext` rendering tests
- serde round-trip tests for:
  - `Diagnostic`
  - `Observation<T>` with fixture payloads
  - `LogEvent`
  - `SpanSignal`
  - `MetricRecord`
- typestate tests for `SpanRecord<SpanStarted>::end(...)`

## 5. Exit Criteria

Sprint 1 is complete only when:
- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `bash scripts/ci/validate_docs_consistency.sh`
- `bash scripts/ci/validate_dependency_bans.sh`
- `bash scripts/ci/validate_repo_boundaries.sh`

And:
- no placeholder fields remain in the finalized `sc-observability-types` API
- `docs/public-api-checklist.md` reflects the Sprint 1 public surface accurately
- `sc-observability-types` API freeze gate is ready for review at sprint close

## 6. Do-Not-Start Conditions

Do not start Sprint 2 until Sprint 1 has:
- green CI on the required gates above
- updated docs/checklist alignment
- no unresolved public API questions on the `sc-observability-types` surface
