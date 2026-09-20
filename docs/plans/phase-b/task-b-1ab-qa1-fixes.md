---
id: B.1ab-QA1-fixes
status: complete
branch: fix/phase-b-1ab-qa1
worktree: /Users/randlee/github/sc-observability-worktrees/fix/phase-b-1ab-qa1
parent: feature/phase-b-1e-migration-prep
---

# B.1a/B.1b QA1 reconciled fixes

## Scope

Apply and verify only FTQ-001, FTQ-002, RBP-F002, RBQA-F002, FTQ-003, and
FTQ-004 from the reconciled QA1 report. Preserve public configuration,
shutdown behavior, root `LogFailure`/`TryLogFailure` exports, and all source
copy boundaries. Record the already-approved B.1d preparation task as complete
on this QA layer, citing its frozen revision and coordinator receipt, without
changing the frozen telemetry implementation branch.

## Required outcomes

- Bound fixture waits/receives and ensure failure/timeout cleanup cannot park a
  worker. The test-only maintenance wait uses one absolute deadline, reports a
  timeout explicitly, and normal fixtures assert that it did not time out.
- Replace sleep-based concurrency ordering with explicit gates and prove the
  active state during the concurrent operation.
- Correct standalone sink remediation without changing diagnostic codes or
  native sources.
- Use the canonical same-crate identity code in the typed failure definition.
- Search the reviewed fixture set for equivalent patterns and record each
  finding as fixed or not reproducible before final review.

## Validation

Run focused tests, each corrected concurrency fixture three times, affected
types/logger package suites, formatting, strict clippy, doctests, and API/docs
gates.

## Completion

Coordinator completeness PASS accepted the complete six-finding correction at
`57176ba4c263a6cf603ef4d78a7e3be94b5d1568`. The clean revision passed all 79
logger unit tests three consecutive times (including explicit timeout and
Drop-cleanup controls), logger all-target tests (79 unit and 2 integration),
types all-target tests (48 unit and 4 integration), workspace doctests, format,
strict workspace clippy, public API semver/docs, documentation consistency, and
`git diff --check`. This is lead completeness only; independent QA remains a
separate gate.
