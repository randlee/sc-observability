---
id: A.2
title: Public API Governance Automation
status: planned
branch: feature/thread-optimization-a2-api-governance
worktree: ../sc-observability-worktrees/feature/thread-optimization
target: develop
---

# Sprint A.2 — Public API Governance Automation

```yaml
plan_type: sprint_plan
phase: A
sprint: A.2
worktree: ../sc-observability-worktrees/feature/thread-optimization
branch: feature/thread-optimization-a2-api-governance
status: planned
estimated_scope: medium
```

## Goal

Make public API changes CI-visible before the runtime API additions and
deprecations land.

## Hard Dependencies

- [`sprint-A1.md`](./sprint-A1.md)
- `docs/public-api-checklist.md`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`

## Prerequisites

- `A.1` accepted

## Exact Targets

- `scripts/ci/`
- CI workflow files
- `docs/public-api-checklist.md`
- contributor/review docs that define API-change approval workflow
- `docs/phase-A/readiness.md`

## Deliverables

- CI-visible public API diff gate for workspace crates
- CI-visible semver compatibility gate for breaking changes against the release
  baseline
- repo-owned validation scripts that fail when public API changes are not
  reflected in the required documentation artifacts
- documented approval artifact required for intentional public API changes

## Required Work

- add automation that makes additive and breaking API diffs visible in CI
- add automation that fails when required API-checklist or normative-doc
  updates are missing
- document the approval path for intentional API changes so the gate is not
  reviewer-memory-only

## Acceptance Criteria

- CI can fail for an unapproved public API change without relying on reviewer
  memory
- additive and breaking changes are both visible to QA directly from repo
  artifacts
- the repo documents how an intentional API change is approved and how the gate
  is satisfied

## Non-Closure

- `A.2` does not implement the writer runtime itself

## Required Validation

- `cargo fmt --check --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `bash scripts/ci/validate_docs_consistency.sh`
- every new API-governance validation script added in this sprint
