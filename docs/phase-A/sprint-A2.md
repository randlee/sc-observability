---
id: A.2
title: Public API Governance Automation
status: implemented
branch: feature/thread-optimization-a2-api-governance
worktree: ../sc-observability-worktrees/feature/thread-optimization-a2-api-governance
target: integrate/phase-a
---

# Sprint A.2 — Public API Governance Automation

```yaml
plan_type: sprint_plan
phase: A
sprint: A.2
worktree: ../sc-observability-worktrees/feature/thread-optimization-a2-api-governance
branch: feature/thread-optimization-a2-api-governance
status: implemented
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
- `docs/api-approvals/`
- contributor/review docs that define API-change approval workflow
- `docs/phase-A/readiness.md`

## Deliverables

- CI-visible public API diff gate for workspace crates
- CI-visible semver compatibility gate for breaking changes against the release
  baseline
- repo-owned script set under `scripts/ci/`:
  - `validate_public_api_diff.sh`
  - `validate_public_api_semver.sh`
  - `validate_public_api_docs.sh`
- repo-owned validation scripts that fail when public API changes are not
  reflected in the required documentation artifacts
- documented approval artifact required for intentional public API changes:
  - one file per intentional API change under `docs/api-approvals/`
  - file name format: `<change-id>.md`
  - required headings: `## Scope`, `## Approval`, `## Affected Artifacts`
  - empty `docs/api-approvals/` is an allowed steady-state until the first
    intentional API change lands; `docs/api-approvals/README.md` is the
    required sentinel that documents this rule

## Gate Shape

The sprint must lock the repo-visible gate shape tightly enough that QA can
review it directly from the sprint doc:

- one CI path that exposes additive public API diffs
- one CI path that fails semver-breaking public API diffs against the release
  baseline
- one repo-owned validation path that fails when public API docs and checklist
  updates are missing for an intentional API change
- one machine-checkable approval artifact format under `docs/api-approvals/`
  that `validate_public_api_docs.sh` can verify directly
- one explicit rule that an empty `docs/api-approvals/` directory is a PASS
  when `README.md` is present and no intentional API change artifact is
  otherwise required
- `validate_public_api_docs.sh` shall fail when any file under
  `docs/api-approvals/` other than `README.md` is missing one or more required
  headings

## Paths To Delete

- none

## Acceptance Criteria

- CI can fail for an unapproved public API change without relying on reviewer
  memory
- additive and breaking changes are both visible to QA directly from repo
  artifacts
- the repo documents how an intentional API change is approved and how the gate
  is satisfied
- the sprint names the exact validation commands or scripts QA must run to see
  the API-governance result
- the approval artifact location and format are concrete enough for
  `validate_public_api_docs.sh` to fail when the artifact is missing
- `NFR-010` and `NFR-011` compliance is demonstrated by the CI scripts
  delivered in `A.2`: `validate_docs_consistency.sh` satisfies `NFR-010` and
  the version-literal check satisfies `NFR-011`

## Non-Closure

- `A.2` does not implement the writer runtime itself

## Required Validation

- `cargo fmt --check --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `bash scripts/ci/validate_docs_consistency.sh`
- `bash scripts/ci/validate_public_api_diff.sh`
- `bash scripts/ci/validate_public_api_semver.sh`
- `bash scripts/ci/validate_public_api_docs.sh`
