# Phase 1 Sprint Assignment

**Status**: Draft for `arch-obs` review
**Branch**: `integrate/phase-1`
**Base**: `develop` at `5f8df2c`
**Scope**: M0 through M4 plus Sprint 5 ATM example and Sprint 6 hardening/release readiness

## 1. Phase Scope

Phase 1 covers the full shared-crate implementation path and the immediate
post-implementation closure work in dependency order:

1. M0 workspace baseline
2. M1 `sc-observability-types`
3. M2 `sc-observability`
4. M3 `sc-observe`
5. M4 `sc-observability-otlp`
6. Sprint 5 working ATM adapter example
7. Sprint 6 hardening and release readiness

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

Status:
- implemented in this branch via:
  - `scripts/ci/validate_docs_consistency.sh`
  - `scripts/ci/validate_dependency_bans.sh`
  - `.github/workflows/ci.yml`

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

### Sprint 5: Working ATM Adapter Example

Build:
- a fully working out-of-the-box ATM adapter example in
  `examples/atm-adapter-example/`
- ATM-shaped observation payload types defined in the example, not in shared
  crates
- ATM-shaped projectors that emit `LogEvent`, `SpanSignal`, and `MetricRecord`
- full-stack wiring: logging + routing + OTLP attachment via projector
  registration
- ATM health projection from:
  - `LoggingHealthReport`
  - `ObservabilityHealthReport`
  - `TelemetryHealthReport`
- translation of `ATM_OTEL_*` env vars into `TelemetryConfig`
- runnable normal shutdown and fail-open shutdown paths

Exit criteria:
- `cargo run --example atm-adapter-example` works
- the example has no dependency on `agent-team-mail-*` or any ATM runtime
- the ATM team can use the example as a direct starting point with no shared
  design questions remaining
- the ATM boundary example and the shared APIs remain aligned

### Sprint 6: Hardening And Release Readiness

Build:
- docs consistency checks wired into `.github/workflows/ci.yml`
- dependency-ban enforcement wired into `.github/workflows/ci.yml`
- performance pass for hot-path allocations
- migration guide for existing ATM logging consumers
- release readiness and publishing gates

Exit criteria:
- all public API checklist items marked `[x]`
- publishing gates from [`publishing.md`](./publishing.md) are satisfied
- Cargo versions are set for release
- release readiness checklist is complete
- migration guidance exists for ATM consumers

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
- runnable ATM adapter example coverage

## 5. Questions And Blockers Before Implementation

### Blockers

1. Sprint 1 should use the concrete execution packet in
   [`sprint-1-execution-packet.md`](./sprint-1-execution-packet.md) as the
   controlling task breakdown so implementation does not drift from the phase
   assignment.

### Questions

1. Should Phase 1 run as six sprint branches/PRs (`S1` through `S6`) or one
   integration branch with milestone checkpoints and QA after each sprint?

2. For Sprint 4, should the deferred health-type `pub use` re-exports be
   treated as part of the M4 “real implementation” closure, or remain
   post-skeleton cleanup immediately after M4?

3. Should docs consistency and dependency-ban enforcement be implemented before
   Sprint 1 starts, or landed as explicit Pre-Sprint cleanup work that must
   close before any crate feature work merges?

4. For Sprint 5, is the acceptance bar “example is runnable and complete” only,
   or should it also include copy-paste starter documentation for ATM team
   adoption in the example directory?

## 6. Recommended Dispatch Order

1. Pre-Sprint-1 cleanup
2. Sprint 1 / M1 `sc-observability-types`
3. Sprint 2 / M2 `sc-observability`
4. Sprint 3 / M3 `sc-observe`
5. Sprint 4 / M4 `sc-observability-otlp`
6. Sprint 5 / working ATM adapter example
7. Sprint 6 / hardening and release readiness

This preserves the approved dependency order and avoids public API churn in
higher layers before lower-layer contracts are stable.
