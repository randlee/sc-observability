---
id: B.1-integration
status: in_progress
branch: feature/phase-b-1-integration
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1-integration
parent: fix/phase-b-1c-qa1
---

# B.1a–B.1d combined integration

Complete the integration left open by the preparation layers. The four full
sprint documents and error-api-contract.md remain authoritative; this task
adds evidence, not a replacement contract. B.1e owns warning rollout.

## Deliverables

1. Reconcile every B.1a–B.1d deliverable and acceptance criterion against the
   merged implementation. Fix genuine integration gaps. Preserve the accepted
   bridge API and all retained lifecycle/serialization behavior.
2. Replace the preliminary error-api-inventory.md with a checked inventory of
   all nine legacy families: production constructors, public signatures,
   feature-gated paths, open traits, private exporters and copied bridge uses.
   Each occurrence has a typed production or named compatibility disposition.
   Add an executable workspace parity test against every owning code registry
   constant; neutral types must not depend on runtime crates.
3. Prove the combined core and accepted bridge through unchanged default,
   test_hooks, release runtime-level, static-cap, macro/consumer and UI suites.
   Retain warning allowances required by accepted bridge signatures separately
   from the immutable historical B.1 import manifest.
4. Update B.1a–B.1d handoffs, full sprint status and project-plan entries from
   actual evidence. Distinguish implementation complete, independent QA,
   unmerged PRs and owner-deferred acceptance. Never invent approvals or claim
   prep QA covered later integration. B.7 alone publishes.

## Validation

Run workspace tests/all-targets, doctests, strict clippy and fmt; bridge feature
and release suites; migration validation when the parent supplies it; API,
semver, docs, dependency and provenance checks. Retain exact commands, source
SHA and raw output, and reference actual three-platform CI results. Preserve
historical import bytes and explicitly separate later warning accommodations.

## Ownership and handoff

Cobs owns observation QA corrections; lobs owns warnings and migration fixtures.
Work on integration tests/inventory/evidence, merge their pushed parent changes
before final validation, and report production overlaps before editing them.
Use a local checklist: implement every item, then verify every item. Send a
progress push promptly. Close only after coordinator completeness PASS.
