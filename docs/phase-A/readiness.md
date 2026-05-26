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
| A.2 | `PENDING` | `PENDING` | `planned` | add semver/public-API governance gates before runtime API changes land |
| A.3 | `PENDING` | `PENDING` | `planned` | implement bounded queue, writer thread, batching, compatibility deprecation path, and health/runtime updates |
| A.4 | `PENDING` | `PENDING` | `planned` | publish consumer rollout and operational guidance for queue-backed logging and doctor-facing health checks |

## Phase Exit Condition

Phase A is ready to close only when:

- `A.1` through `A.4` each record an accepted commit and verdict
- the final accepted runtime uses one writer-owned queue-backed logging model
  rather than a maintenance-only background worker
- public API changes are documented and CI-visible
- consumer docs teach `log()` / `try_log()` as the preferred logging APIs
