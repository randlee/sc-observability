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
- `examples/custom-sink-example/`
- `docs/phase-A/readiness.md`

## Deliverables

- consumer docs explaining `log()` versus `try_log()`
  Requirements: `DOC-001`, `DOC-002`
- migration guidance away from `emit()`
  Requirements: `DOC-002`
- docs explaining queue admission versus durability
  Requirements: `DOC-002`, `LOG-016`
- docs and examples showing queue/writer health inspection
  Requirements: `DOC-003`, `DOC-004`, `LOG-016`
- explicit operator guidance that dropped logs, writer degradation, or sustained
  queue pressure are serious `doctor` findings
  Requirements: `DOC-002`, `DOC-004`, `LOG-016`

## Paths To Delete

- none

## Acceptance Criteria

- consumer docs prefer `log()` / `try_log()` over `emit()`
- the docs clearly distinguish queue admission from durability
- at least one consumer-facing example shows queue/writer health inspection
- the docs tell operators that dropped logs and writer degradation are serious
  issues rather than hidden implementation detail
- the consumer guidance remains logging-layer-only and does not require
  ATM-specific wrapper code in this repo

## Non-Closure

- `A.4` does not implement ATM-specific `doctor` commands in this repo

## Required Validation

- `cargo fmt --check --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `bash scripts/ci/validate_docs_consistency.sh`
- reviewer-confirmed snippet audit showing new consumer guidance prefers
  `log()` / `try_log()`
