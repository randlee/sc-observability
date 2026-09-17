---
id: B.1e-migration-prep
status: in_progress
branch: feature/phase-b-1e-migration-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1e-migration-prep
parent: feature/phase-b-1d-telemetry-prep
authoritative-sprint-doc: sprint-b-1e-error-adoption.md
contract: error-api-contract.md
---

# B.1e migration preparation — exact errors and warnings

## Scope

This child prepares the B.1e migration guide and source inventory against the
merged B.1d telemetry APIs. It is documentation/inventory-only. It does not
activate `#[deprecated]` attributes, migrate ordinary production call sites,
modify copied crates/workspace/CI, or claim that full B.1e is complete. The
team lead owns the downstream validator, Cargo fixtures, and CI integration;
their pending gates are recorded in the handoff.

The task document was absent when this child started. This file records the
authoritative sprint and contract as the replacement scoped task record rather
than silently assuming a missing plan. The telemetry parent was merged at
`d4997029f83664331c8c4e3cc20005ae656d254a` after its C01 correction removed
the unplanned `with_typed_*` projector builders.

## Deliverables

- `references/migrate-error-api.md` gives the exact old/new methods, nine
  wrapper-to-failure families, prerequisite version policy, typed matching and
  fallback, source/diagnostic retention, explicit trait adapters, rollback and
  narrow warning handling.
- `warning-inventory-b-1e.md` lists every warning candidate, supported method
  exemption, retained API, and source location. It distinguishes future
  warning activation from currently implemented warnings (`Logger::emit` and
  pre-existing deprecations).
- `.claude/skills/sc-observability-adopting/SKILL.md` routes adopters to the
  new reference without importing typed traits into the root glob API.
- `handoff-b-1e.md` records exact current symbols, executable guide evidence,
  pending validator/fixture/CI ownership, and the non-closure boundary.

## Validation boundary

The guide examples use only APIs present at the merged parent head and cover a
successful typed path plus an invalid-input failure. Workspace tests verify
the same typed paths and adapter/source behavior. B.1e implementation selects,
activates and validates the exact warning policy after the B.P2 `1.3.0`
staged prerequisite (`1.4.0` is the next-minor candidate); this scoped prep
leaves that implementation pending. B.2 qualifies the B.1e result and B.7
alone publishes. No release or removal schedule is introduced here.

## Completion evidence

This document must be changed to `status: complete` only after the aobs
completeness PASS, with the final source commit and validation results recorded
in `handoff-b-1e.md`.
