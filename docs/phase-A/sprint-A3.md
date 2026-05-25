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
- `crates/sc-observability/src/query.rs`
- `crates/sc-observability/src/follow.rs`
- `crates/sc-observability/src/jsonl_reader.rs`
- `crates/sc-observability-types/src/health.rs`
- matching tests and consumer-facing rustdoc
- `docs/phase-A/readiness.md`

## Deliverables

- bounded queue between producer calls and writer-owned sink I/O
  Requirements: `LOG-016`, `LOG-041`, `LOG-046`
- one writer thread that owns batching, sink writes, retained-log rotation,
  retained-log pruning, flush, and shutdown drain coordination
  Requirements: `LOG-041`, `LOG-046`
- `Logger::log(...)`
  Requirements: `LOG-014`, `LOG-023`, `LOG-047`
- `Logger::try_log(...)`
  Requirements: `LOG-014`, `LOG-023`, `LOG-048`
- deprecated `Logger::emit(...)` routed through the approved compatibility path
  Requirements: `LOG-023`
- extended `LoggingHealthReport` with queue and writer fields
  Requirements: `LOG-016`
- query/follow and health updates required to preserve active-plus-rotated log
  correctness under the writer-owned runtime
  Requirements: `LOG-016`, `LOG-025`, `LOG-026`, `LOG-029`, `LOG-030`,
  `LOG-031`
- tests covering queue admission, queue-full behavior, batching, shutdown
  drain, retained-log maintenance under writer ownership, and deprecated
  `emit()` compatibility

## Paths To Delete

- dedicated-worker coordination types in
  `crates/sc-observability/src/maintenance.rs`
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
- query/follow behavior remains correct against active and rotated log files
  after the writer-runtime change
- `crates/sc-observability/src/maintenance.rs` no longer carries the removed
  dedicated-worker coordination scaffolding once the writer-owned runtime is
  in place

## Required Validation

- `cargo fmt --check --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `bash scripts/ci/validate_repo_boundaries.sh`
- `bash scripts/ci/validate_docs_consistency.sh`
- `bash scripts/ci/validate_public_api_diff.sh`
- `bash scripts/ci/validate_public_api_semver.sh`
- `bash scripts/ci/validate_public_api_docs.sh`

## Non-Closure

- `A.3` does not add an async runtime dependency
- `A.3` does not redesign OTLP batching or span assembly
- `A.3` does not make query/follow surface queued-but-unflushed records
