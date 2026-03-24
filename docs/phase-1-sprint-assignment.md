# Phase 1 Sprint Assignment

**Status**: Draft for `arch-obs` review
**Branch**: `integrate/phase-1`
**Base**: `develop` at `5f8df2c`
**Scope**: M0 through M4 only
**Out of scope**: M5 ATM adapter work, M6 hardening/release work

## 1. Phase Scope

Phase 1 covers the full shared-crate implementation path in dependency order:

1. M0 workspace baseline
2. M1 `sc-observability-types`
3. M2 `sc-observability`
4. M3 `sc-observe`
5. M4 `sc-observability-otlp`

No ATM-owned adapter implementation work starts in this phase.

## 2. Pre-Sprint-1 Cleanup

These should land before Sprint 1 starts:

1. Change sealed emitter traits to `pub(crate)` instead of `pub`:
- `crates/sc-observability/src/lib.rs`: `LogEmitter`
- `crates/sc-observe/src/lib.rs`: `ObservationEmitter<T>`
- `crates/sc-observability-otlp/src/lib.rs`: `SpanEmitter`, `MetricEmitter`

2. Checklist cosmetic cleanup from QA-9:
- add `[~]` markers to `SpanEmitter` and `MetricEmitter` in
  [`public-api-checklist.md`](./public-api-checklist.md) §5 internal-only list

3. Confirm Sprint 1 CI gates are actually implemented, not just documented:
- docs consistency checks
- dependency-ban enforcement

## 3. Sprint Assignment By Milestone

### Sprint 1 / M0-M1: `sc-observability-types`

Build:
- workspace baseline remains green with all four crates present
- validated name newtypes
- `ErrorCode`, `Diagnostic`, `Remediation`, `ErrorContext`
- `TraceId`, `SpanId`, `TraceContext`
- `Observation<T>`, `LogEvent`, `SpanRecord<S>`, `SpanSignal`, `MetricRecord`
- shared health models
- shared open traits
- serde coverage for public contracts

Exit criteria:
- `cargo check --workspace --all-targets`
- unit tests for validation, error rendering, serialization
- typestate span lifecycle tests
- API checklist frozen for `sc-observability-types`

### Sprint 2 / M2: `sc-observability`

Build:
- `LoggerConfig::default_for(...)`
- `Logger`
- `LogSink`, `LogFilter`, `SinkRegistration`
- JSONL file sink
- human-readable console sink
- redaction pipeline
- rotation and retention behavior
- logging health reporting

Exit criteria:
- file sink path/default behavior matches docs
- file-only and file+console fan-out verified
- validation failures return `EventError`
- sink failures remain fail-open and are counted
- logging-only integration path passes without `sc-observe`
- API checklist frozen for `sc-observability`

### Sprint 3 / M3: `sc-observe`

Build:
- `ObservabilityConfig`
- `ObservabilityBuilder`
- `Observability`
- subscriber registration
- projector registration
- per-type filtering and routing
- top-level health aggregation

Exit criteria:
- one typed observation fans out to:
  - one or more subscribers
  - one or more log projectors
  - one or more span/metric projectors
- deterministic registration-order routing verified
- post-shutdown emission returns `ObservationError::Shutdown`
- no runtime registration after `build()`
- API checklist frozen for `sc-observe`

### Sprint 4 / M4: `sc-observability-otlp`

Build:
- `TelemetryConfigBuilder`
- `Telemetry`
- `OtelConfig`, `OtlpProtocol`
- `SpanAssembler`
- `CompleteSpan`
- `LogExporter`, `TraceExporter`, `MetricExporter`
- telemetry health
- OTLP attachment through builder projector registration

Exit criteria:
- invalid OTLP config rejected eagerly in `Telemetry::new(...)`
- logs, spans, and metrics attach through builder registration
- emit methods return `TelemetryError::Shutdown` after shutdown
- in-flight spans are flushed or explicitly dropped and counted before shutdown completes
- telemetry lifecycle matches documented behavior
- API checklist frozen for `sc-observability-otlp`

## 4. Required Test Gates Per Sprint

Every sprint in this phase must keep these green:

- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- `bash scripts/ci/validate_repo_boundaries.sh`
- docs consistency checks
- dependency-ban enforcement

Required integration layers across the phase:

- logging-only CLI path
- routing + logging path
- full stack path with OTLP attached through projector registration
- ATM boundary example compile coverage

## 5. Questions And Blockers Before Implementation

### Blockers

1. The required Sprint 1 CI gates for docs consistency and dependency-ban
   enforcement are documented but do not appear to be implemented yet in
   `scripts/ci/` or `.github/workflows/ci.yml`.

2. The pre-Sprint cleanup items above are still open in this worktree and
   should be completed before Sprint 1 begins.

### Questions

1. Should Phase 1 run as four sprint branches/PRs (`M1` through `M4`) or one
   integration branch with milestone checkpoints and QA after each sprint?

2. For Sprint 4, should the deferred health-type `pub use` re-exports be
   treated as part of the M4 “real implementation” closure, or remain
   post-skeleton cleanup immediately after M4?

3. Should docs consistency and dependency-ban enforcement be implemented in the
   shared repo before Sprint 1 starts, or landed as explicit Sprint 1 kickoff
   work that must close before any crate feature work merges?

## 6. Recommended Dispatch Order

1. Pre-Sprint-1 cleanup
2. Sprint 1 / M1 `sc-observability-types`
3. Sprint 2 / M2 `sc-observability`
4. Sprint 3 / M3 `sc-observe`
5. Sprint 4 / M4 `sc-observability-otlp`

This preserves the approved dependency order and avoids public API churn in
higher layers before lower-layer contracts are stable.
