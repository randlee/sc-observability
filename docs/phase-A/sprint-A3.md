---
id: A.3
title: Writer Runtime And Logging API Implementation
status: planned
branch: feature/thread-optimization-a3-writer-runtime
worktree: ../sc-observability-worktrees/feature/thread-optimization
target: develop
---

# Sprint A.3 — Writer Runtime And Logging API Implementation

```yaml
plan_type: sprint_plan
phase: A
sprint: A.3
worktree: ../sc-observability-worktrees/feature/thread-optimization
branch: feature/thread-optimization-a3-writer-runtime
status: planned
estimated_scope: large
```

## Goal

Replace the current caller-thread write path plus maintenance-only worker model
with one queue-backed writer runtime and the locked public logging API surface.

## Hard Dependencies

- [`sprint-A1.md`](./sprint-A1.md)
- [`sprint-A2.md`](./sprint-A2.md)
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`

## Prerequisites

- `A.1` accepted
- `A.2` accepted

## Exact Targets

- `crates/sc-observability/src/lib.rs`
- `crates/sc-observability/src/runtime.rs`
- `crates/sc-observability/src/sinks.rs`
- `crates/sc-observability/src/maintenance.rs`
- `crates/sc-observability-types/src/health.rs`
- matching tests and consumer-facing rustdoc
- `docs/phase-A/readiness.md`

## Deliverables

- bounded queue between producer calls and writer-owned sink I/O
- one writer thread that owns batching, sink writes, retained-log rotation,
  retained-log pruning, flush, and shutdown drain coordination
- `Logger::log(...)`
- `Logger::try_log(...)`
- deprecated `Logger::emit(...)` routed through the approved compatibility path
- extended `LoggingHealthReport` with queue and writer fields
- tests covering queue admission, queue-full behavior, batching, shutdown
  drain, retained-log maintenance under writer ownership, and deprecated
  `emit()` compatibility

## Required Work

- remove the dedicated maintenance-only concurrency model as the primary
  logging runtime shape
- ensure `log()` blocks until queue admission and never drops silently
- ensure `try_log()` returns immediately and reports queue saturation
- ensure queue-full drops and writer failures are counted and surfaced through
  `logger.health()`
- ensure `flush()` is the queue-drain and sink-flush barrier
- ensure `shutdown()` stops new work, drains remaining queued work under the
  approved contract, and preserves final health state
- keep query/follow behavior correct against active and rotated files after the
  runtime change

## Paths To Delete

- dedicated-worker coordination types in
  `crates/sc-observability/src/maintenance.rs` if superseded by the final
  writer-owned runtime
- any caller-thread maintenance signaling paths that exist only to wake the
  removed dedicated worker

## Acceptance Criteria

- the caller hot path no longer performs built-in file-sink writes directly
- the runtime no longer dedicates a thread solely to maintenance while keeping
  file writes synchronous on producer threads
- `log()` and `try_log()` obey the locked semantics from `A.1`
- queue-full drops and writer degradation are visible through
  `LoggingHealthReport`
- deprecated `emit()` remains callable and clearly compatibility-scoped
- retained-log rotation and pruning still work correctly under the writer-owned
  runtime

## Required Validation

- `cargo fmt --check --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `bash scripts/ci/validate_repo_boundaries.sh`
- `bash scripts/ci/validate_docs_consistency.sh`
