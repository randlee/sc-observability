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
- docs consistency checks
- dependency-ban enforcement

The following can be added once behavior exists:

- focused integration-test job

Phase-A validation additions once the phase starts landing:

- `bash scripts/ci/validate_writer_thread_lock.sh` for the A.1 normative-doc
  lock
- `bash scripts/ci/validate_public_api_diff.sh` for additive public API diffs
- `python3 scripts/ci/validate_public_api_semver.py` for semver-breaking API diffs
- `bash scripts/ci/validate_public_api_docs.sh` for machine-checkable API
  approval/documentation coverage

## 6. Exit Criteria

A sprint is only complete when:

- the implementation milestone code is present
- the tests required by this document are present
- CI gates are green
- the API checklist is updated if any public surface changed

See also: [`implementation-plan.md`](./implementation-plan.md) §5 Cross-Crate
Acceptance Gates.
