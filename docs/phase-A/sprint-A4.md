---
id: A.4
title: Consumer Rollout And Operational Guidance
status: planned
branch: feature/thread-optimization-a4-consumer-rollout
worktree: ../sc-observability-worktrees/feature/thread-optimization
target: develop
---

# Sprint A.4 — Consumer Rollout And Operational Guidance

```yaml
plan_type: sprint_plan
phase: A
sprint: A.4
worktree: ../sc-observability-worktrees/feature/thread-optimization
branch: feature/thread-optimization-a4-consumer-rollout
status: planned
estimated_scope: medium
```

## Goal

Update consumer docs and examples so downstream adopters use the new logging
APIs and can surface queue/writer degradation through health and `doctor`
commands.

## Hard Dependencies

- [`sprint-A3.md`](./sprint-A3.md)
- `CONSUMING.md`
- `README.md`
- examples using `Logger`

## Prerequisites

- `A.3` accepted

## Exact Targets

- `README.md`
- `CONSUMING.md`
- relevant examples
- `docs/phase-A/readiness.md`

## Deliverables

- consumer docs explaining `log()` versus `try_log()`
- migration guidance away from `emit()`
- docs explaining queue admission versus durability
- docs and examples showing queue/writer health inspection
- explicit operator guidance that dropped logs, writer degradation, or sustained
  queue pressure are serious `doctor` findings

## Required Work

- stop teaching `emit()` as the default logging API
- teach `flush()` and shutdown semantics under queue-backed logging
- document how downstream health or `doctor` commands should interpret queue
  depth, high-water mark, drop counts, and writer state
- keep the guidance logging-layer-only and free of ATM-specific wrapper code

## Acceptance Criteria

- consumer docs prefer `log()` / `try_log()` over `emit()`
- the docs clearly distinguish queue admission from durability
- at least one consumer-facing example shows queue/writer health inspection
- the docs tell operators that dropped logs and writer degradation are serious
  issues rather than hidden implementation detail

## Non-Closure

- `A.4` does not implement ATM-specific `doctor` commands in this repo

## Required Validation

- `cargo test --workspace`
- `bash scripts/ci/validate_docs_consistency.sh`
- reviewer-confirmed snippet audit showing new consumer guidance prefers
  `log()` / `try_log()`
