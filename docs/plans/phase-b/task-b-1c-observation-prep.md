---
id: B.1c-observation-prep
status: complete
branch: feature/phase-b-1c-observation-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1c-observation-prep
parent: feature/phase-b-1b-logger-prep
authoritative-sprint-doc: sprint-b-1c-observation-errors.md
contract: error-api-contract.md
---

# B.1c observation preparation — typed construction and lifecycle operations

This scoped preparation layer implements the B.1c observation facade over the
logger and neutral typed subscriber/projector contracts. The sprint and error
contract remain authoritative for exact behavior and compatibility.

## Scope and checklist

- [x] Add typed configuration defaults and service-name validation.
- [x] Add typed construction through the existing builder and runtime.
- [x] Add typed `emit`, flush, and idempotent shutdown paths without changing
  `ObservationError`, registration methods, filtering, ordering, aggregation,
  health, or legacy signatures.
- [x] Register real typed subscriber/projector implementations through the
  existing legacy registration boundaries and preserve one invocation per
  route.
- [x] Cover invalid names, empty routes, logger startup errors, filtering,
  routing, projector output families, custom/wrong-family codes, lifecycle,
  source/context retention, and concurrent shutdown.
- [x] Record the observation inventory and exact validation evidence in
  `handoff-b-1c.md`.

## Boundaries

No changes to `sc-observability-types`, the logger parent, OTLP transport,
`ObservationError`, public registration traits, serializer shapes, publication,
warning activation, or bridge APIs. Full B.1c integration and independent QA
remain pending source import and copied-bridge acceptance.

## Validation

The final handoff records the authoritative B.1c validation matrix and both
contract-completeness and behavior/source-integrity passes.
