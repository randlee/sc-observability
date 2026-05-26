# Phase A Readiness

## Purpose

Authoritative status record for the thread-optimization phase.

Phase A covers the queue-backed writer-thread redesign for `sc-observability`,
the associated public API additions and deprecations, consumer rollout, and
the CI gates needed to keep future API changes explicit.

## Record Schema

Each sprint row must record:

- `sprint`
- `accepted_commit`
- `verdict`
- `current_status`
- `notes`

Sprint planning status convention:

- sprint docs remain `status: planned` until execution closes the sprint on the
  implementation line

## Phase Documents

- [`sprint-A1.md`](./sprint-A1.md)
- [`sprint-A2.md`](./sprint-A2.md)
- [`sprint-A3.md`](./sprint-A3.md)
- [`sprint-A4.md`](./sprint-A4.md)

## Initial State

| Sprint | Accepted Commit | Verdict | Current Status | Notes |
| --- | --- | --- | --- | --- |
| A.1 | `PENDING` | `PENDING` | `implemented` | branch `feature/thread-optimization-a1-architecture-lock` closes the architecture/API lock deliverables; awaiting QA verdict and accepted commit |
| A.2 | `PENDING` | `PENDING` | `implemented` | branch `feature/thread-optimization-a2-api-governance` closes the public-API governance gates and approval workflow deliverables; awaiting QA verdict and accepted commit |
| A.3 | `PENDING` | `PENDING` | `implemented` | branch `feature/thread-optimization-a3-writer-runtime` closes the writer runtime, queue-backed logging API, compatibility emit path, and queue/writer health deliverables; awaiting QA verdict and accepted commit |
| A.4 | `PENDING` | `PENDING` | `planned` | publish consumer rollout and operational guidance for queue-backed logging and doctor-facing health checks |

## Phase-A Compliance Note

- `NFR-010` compliance for phase-A planning is verified by keeping the
  normative thread-optimization lock text aligned across
  `docs/requirements.md`, `docs/architecture.md`, and `docs/api-design.md`,
  with `bash scripts/ci/validate_docs_consistency.sh` and
  `bash scripts/ci/validate_writer_thread_lock.sh` as the active CI checks.
- `NFR-011` compliance for version literals is enforced going forward by
  `python3 scripts/ci/validate_version_literals.py`.
- The `since = "1.2.0"` literal in `docs/api-design.md` §11.6 is the locked
  phase-A target release marker and has been intentionally verified against the
  `project-plan.md` phase-A version designation; before release, the workspace
  version must be advanced to match and the version-literal CI gate must pass.

## Phase Exit Condition

Phase A is ready to close only when:

- `A.1` through `A.4` each record an accepted commit and verdict
- the final accepted runtime uses one writer-owned queue-backed logging model
  rather than a maintenance-only background worker
- public API changes are documented and CI-visible
- consumer docs teach `log()` / `try_log()` as the preferred logging APIs
