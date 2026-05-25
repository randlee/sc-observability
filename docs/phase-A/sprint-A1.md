---
id: A.1
title: Architecture And API Lock
status: planned
branch: feature/thread-optimization-a1-architecture-lock
worktree: ../sc-observability-worktrees/feature/thread-optimization
target: develop
---

# Sprint A.1 — Architecture And API Lock

```yaml
plan_type: sprint_plan
phase: A
sprint: A.1
worktree: ../sc-observability-worktrees/feature/thread-optimization
branch: feature/thread-optimization-a1-architecture-lock
status: planned
estimated_scope: medium
```

## Goal

Lock the normative writer-thread architecture and the public logging contract
before implementation begins.

## Hard Dependencies

- `docs/project-plan.md`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/public-api-checklist.md`
- `docs/performance-pass.md`

## Prerequisites

- retained-log maintenance work is treated as complete historical input rather
  than the current controlling sprint
- no runtime implementation work for thread optimization lands before this
  sprint is accepted

## Exact Targets

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/project-plan.md`
- `docs/public-api-checklist.md`
- `docs/performance-pass.md`
- `docs/phase-A/readiness.md`

## Deliverables

- normative docs updated to define the queue-backed writer-thread architecture
  as the approved runtime model
- explicit statement that retained-log maintenance runs on the same writer
  thread during idle or post-batch windows rather than on a dedicated
  maintenance-only thread
- locked contract for:
  - `Logger::log(...)`
  - `Logger::try_log(...)`
  - deprecated `Logger::emit(...)`
  - `Logger::flush(...)`
  - `Logger::shutdown(...)`
- locked queue and writer health fields for `LoggingHealthReport`
- locked public config surface for queue capacity and batching controls
- explicit public-API governance requirement stating that API changes must be
  documented and machine-checked

## Required Work

- define that producers validate, redact, and queue log events
- define that one writer thread owns sink writes, batching, rotation, pruning,
  and flush
- define that `log()` blocks until queue admission, not durability
- define that `try_log()` is non-blocking and returns explicit queue-full
  failure
- define the compatibility behavior and deprecation guidance for `emit()`
- define the queue depth, capacity, high-water-mark, drop, and writer-state
  health requirements needed by downstream `doctor` commands
- update `docs/performance-pass.md` so it no longer conflicts with the approved
  redesign

## Acceptance Criteria

- `requirements.md`, `architecture.md`, `api-design.md`, and the phase-A sprint
  docs all describe the same writer-thread ownership model
- no normative doc still describes a maintenance-only background worker as the
  target runtime model
- the docs explicitly distinguish queue admission from write durability
- the docs explicitly define queue-full behavior and health reporting
- the docs explicitly record that future public API changes require both docs
  updates and automated API-gate approval

## Non-Closure

- `A.1` does not implement runtime code
- `A.1` does not add CI scripts; it only locks the requirement for them

## Required Validation

- `bash scripts/ci/validate_docs_consistency.sh`
- reviewer-confirmed cross-doc signature and semantics check against this
  sprint doc
