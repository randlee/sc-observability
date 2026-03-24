# SC-Observability Test Strategy

**Status**: Draft for review
**Applies to**: all shared workspace crates and the unpublished ATM proving example
**Related documents**:
- [`implementation-plan.md`](./implementation-plan.md)
- [`public-api-checklist.md`](./public-api-checklist.md)
- [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)

## 1. Purpose

This document defines the minimum test coverage required for each
implementation sprint so the project does not defer verification until the end.

## 2. Shared Rules

- Every public constructor or validator gets unit tests.
- Every public error/rendering contract gets tests.
- Every cross-crate lifecycle contract gets at least one integration test.
- Boundary CI must stay green on every sprint branch.
- The unpublished ATM proving example must compile in CI.

## 3. Per-Crate Test Requirements

### 3.1 `sc-observability-types`

Required tests:

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

### 3.2 `sc-observability`

Required tests:

- `LoggerConfig::default_for(...)` defaults
- file path layout generation
- redaction behavior
- sink filtering behavior
- file + console fan-out behavior
- fail-open sink failure accounting
- post-shutdown lifecycle behavior

### 3.3 `sc-observe`

Required tests:

- registration-order routing
- filter acceptance/rejection
- subscriber failure isolation
- projector failure isolation
- routing failure when no eligible path remains
- post-shutdown `ObservationError::Shutdown`
- top-level health aggregation

### 3.4 `sc-observability-otlp`

Required tests:

- `TelemetryConfigBuilder` defaults
- invalid config rejection at `Telemetry::new(...)`
- `SpanAssembler` start/event/end assembly
- incomplete span drop accounting
- exporter failure accounting
- post-shutdown `TelemetryError::Shutdown`

## 4. Integration Test Layers

### Shared Integration

Required shared integration tests:

- logging-only CLI path
- routing + logging path
- full stack path with OTLP attached through projector registration

### ATM Boundary Proof

Required unpublished example/integration coverage:

- ATM-shaped payload type remains outside shared crates
- ATM-shaped projectors can emit `LogEvent`, `SpanSignal`, and `MetricRecord`
- OTLP attachment works through builder registration
- no `agent-team-mail-*` dependency is introduced

## 5. CI Gates

Minimum CI gates per sprint:

- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- `bash scripts/ci/validate_repo_boundaries.sh`
- docs consistency checks
- dependency-ban enforcement

The following can be added once behavior exists:

- focused integration-test job

## 6. Exit Criteria

A sprint is only complete when:

- the implementation milestone code is present
- the tests required by this document are present
- CI gates are green
- the API checklist is updated if any public surface changed

See also: [`implementation-plan.md`](./implementation-plan.md) §5 Cross-Crate
Acceptance Gates.
