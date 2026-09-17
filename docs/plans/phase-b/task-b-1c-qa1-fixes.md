---
id: B.1c-QA1-fixes
status: in_progress
branch: fix/phase-b-1c-qa1
worktree: /Users/randlee/github/sc-observability-worktrees/fix/phase-b-1c-qa1
parent: feature/phase-b-1e-migration-validation
authoritative-sprint-doc: sprint-b-1c-observation-errors.md
---

# B.1c QA1 reconciled fixes

## Scope

Apply and verify FTQ-001, FTQ-002, RBP-F002, ATM-QA-001, and ATM-QA-002 only.
Preserve public signatures, routing/lifecycle behavior, health accounting, and
the success-only shutdown outcome. Full B.1c integration and independent QA are
separate gates.

## Required outcomes

- Bound concurrent shutdown and flush fixture completion without busy-spinning
  or unbounded joins, and prove in-flight shutdown remains safe for emit,
  flush, and health.
- Move blocking logger shutdown outside its mutex while keeping a coherent
  transition that never exposes `None` to current callers.
- Exercise custom and wrong-family diagnostic retention in subscriber and all
  projector adapters, both legacy-to-typed and typed-to-legacy.
- Retain actual workspace/doctest evidence at the tested SHA or correct stale
  counts in the handoff.

## Validation

Run affected observation tests and repeated concurrency fixtures, then format,
workspace clippy/doctests, public API semver/docs, and documentation checks.
This task remains open until coordinator completeness PASS.
