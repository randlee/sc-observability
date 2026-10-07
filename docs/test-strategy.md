# SC-Observability Test Strategy

**Status**: Draft for review
**Applies to**: all shared workspace crates and the unpublished ATM proving example
**Related documents**:
- [`implementation-plan.md`](./implementation-plan.md)
- [`public-api-checklist.md`](./public-api-checklist.md)
- [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)

## Phase H applicability (planned)

H-001..H-008 and ADR-023 supersede the historical custom OTel model,
projector, durable-store and lifecycle test requirements below only for the
explicitly removed OTel code. Retained logging tests remain required.

- h-1 owns native construction (`send_log`, `send_span`, `send_metrics`),
  sink mapping, timeout/failure results, provider ownership and feature isolation.
- h-2 owns CLI parsing/result mapping and Clap-generated manual/site freshness.
- h-3 owns Python argument/result mapping, imports/types and GIL behavior.
- h-4 owns the single real local Collector readback proof using installed CLI
  and wheel, native Tokio setup, destination modes and macro/tracing composition;
  it checks frontend failure equivalence and existing installer documentation
  consumption. It also verifies protected logging consumers and net handwritten
  code deletion. It does not duplicate h-1 timeout/provider unit suites.

Existing cross-platform guidelines require bounded waits, teardown and temporary
output directories. Planning feasibility probes do not qualify release artifacts.
No workflow or release-gate changes are authorized by this plan.

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
- `LogQuery` validation tests
- remediation construction tests
- `ErrorContext` rendering tests
- `QueryError` to `SC_LOG_QUERY_*` mapping tests
- serde round-trip tests for:
  - `Diagnostic`
  - `Observation<T>` with fixture payloads
  - `LogEvent`
  - `LogQuery`
  - `LogSnapshot`
  - `QueryError`
  - `QueryHealthReport`
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

Phase-A additions for the queue-backed writer runtime:

- `Logger::log(...)` queue-admission behavior
- `Logger::try_log(...)` non-blocking queue-full behavior
- deprecated `emit()` compatibility behavior while `log()` / `try_log()` are
  preferred
- queue depth, queue capacity, queue high-water mark, and queue-full drop
  accounting on `LoggingHealthReport`
- writer-state and last-writer-error health behavior
- batching behavior under burst load
- shutdown-drain completion behavior, including timeout-threshold degradation
  without detached writer continuation
- retained-log maintenance execution on the writer thread during idle or
  post-batch windows
- query/follow parity against active and rotated files after the writer-runtime
  change
- platform-specific regression coverage for macOS, Linux, and Windows around
  file identity, rotation, truncate/recreate, and follow continuity

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
- rustdoc missing-doc checks
- dependency-ban enforcement

The following can be added once behavior exists:

- focused integration-test job

Phase-A validation additions once the phase starts landing:

- `python3 scripts/ci/stock_public_api.py check --target-dir <target>` for
  accepted Rust public API baselines

## 6. Exit Criteria

A sprint is only complete when:

- the implementation milestone code is present
- the tests required by this document are present
- CI gates are green
- the API checklist is updated if any public surface changed

See also: [`implementation-plan.md`](./implementation-plan.md) §5 Cross-Crate
Acceptance Gates.

## 7. Proposed Phase B Validation Ownership

Phase B supplements these baseline tests with the authoritative validation lists
in its [18 sprint records](plans/phase-b/plan-phase-b.md). The
[document map](plans/phase-b/document-coverage.md) routes each affected crate to
its requirements, architecture and contract. B.P1/B.P3 own runtime/bridge races
and fault injection; B.1a–B.1e own compatibility and typed-failure fixtures;
B.3 owns schema conversion; B.3b owns shared native backends/coordinator;
B.3a owns real Tauri IPC; B.4 owns Python runtime;
B.4a owns all 25 Python/platform cells; B.5/B.6 extend those installed-artifact
checks for integration/async behavior. Release sprints rerun consumer checks
against their immutable published artifacts. No planning pass claims those
future implementation checks have already run.
