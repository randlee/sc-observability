# Repository Quality Policy

This file contains repository-specific QA policy. Reusable agents and skills
must read this file rather than embedding repository names, commands,
interfaces, approval authorities, or temporary architectural exceptions in
their own prompts.

## Repository Baseline

- Default comparison branch: `develop`
- Requirements index: `docs/requirements.md`
- Architecture index: `docs/architecture.md`
- Project plan: `docs/project-plan.md`
- Team protocol: `docs/team-protocol.md`
- Crate docs: `crates/<crate>/docs/` when present
- Rust TODO finder (todo-triage): `python3 .claude/skills/todo-triage/scripts/find_todos.py`
- Developer roster: no roster file. Developers are `cobs` (terra), `lobs`
  (luna), `aobs` (astra); `.atm.toml` and the ATM roster are authoritative
  for live identities.

## Reviewer Policy

- Initial implementation review (sprint QA-1): `req-qa`, `arch-qa`,
  `ruthless-boundary-qa`, `rust-qa-agent`, `rust-best-practices-agent`, and
  `rust-service-hardening-agent`
- Fix verification (sprint QA-2 and later): `req-qa`, `arch-qa`, and
  `rust-qa-agent`; re-dispatch `ruthless-boundary-qa`,
  `rust-best-practices-agent`, or `rust-service-hardening-agent` only to
  verify its own QA-1 findings, verification-locked to those ids; a reviewer
  that raised no QA-1 findings is not re-run
- Plan review QA-1: `plan-scope-reviewer`, `req-qa`, `arch-qa`,
  `ruthless-boundary-qa`, `rust-best-practices-agent`,
  `rust-service-hardening-agent`, and `ceremony-qa`. `plan-scope-reviewer`
  judges the plan's shape against the architecture's boundary map: the wave
  table, the critical path and width it recomputes from the bead graph,
  `owned_paths` disjointness within a wave, and the contract artifact named
  by every `must_follow` edge. Its assignment is rendered with
  `plan-scope-reviewer-assignment.json.j2` (codex-orchestration for a plan in
  markdown, atm-bd-orchestration for a plan in beads) over the same plan
  files as `req-qa` and `arch-qa`, plus the phase plan or root
- Plan review QA-2 and later: `req-qa` and `arch-qa` scoped to the
  dispatched findings, and `plan-scope-reviewer` in full again every round:
  its checks are recomputed from the graph, not carried, so a fix round that
  lengthens the critical path, adds an ordering rule or moves a shared file
  into a layer sprint fails the round even when every carried finding is
  fixed. Re-dispatch `ruthless-boundary-qa`, `rust-best-practices-agent`,
  `rust-service-hardening-agent`, or `ceremony-qa` only to verify its own
  QA-1 findings, verification-locked to those ids. Plan QA is capped at 3
  rounds (`plan_qa_cycle_limit`); a round that leaves only minor findings
  reports `PASS — minor fixes required, no re-QA`
- Plan review rulings: the remedy for shared types, shared files or a shared
  version baseline is to hoist the artifact into the contract or integration
  sprint (plan guidelines, "Ownership And Dependency Relations"), never an
  edge. `quality-mgr` lists every finding whose remedy would add a
  `must_follow` edge, an ordering rule or a merge-order clause as a proposed
  `hoist` ruling for the lead (`quality-mgr.md`, "Hoist Rulings"), in the
  same way as its proposed `rejected: ceremony` rulings. The lead accepts an
  edge only with a recorded reason naming the artifact that could not be
  hoisted, and recomputes the wave table's critical path, width and sprint
  count after each round of rulings. An edge that lengthens the critical
  path is the user's ruling, not the lead's: the lead puts the proposed edge,
  the artifact and the new critical path to the user and records the answer
  in the ruling; without that record the round is redone. The baseline
  critical path is the architecture's layer count (contract, the layers
  `docs/architecture.md` defines, integration), which differs per repository;
  a plan is not held to a fixed number of waves, but every wave past that
  baseline carries a user-approved reason
- `ceremony-finding-screen`: every sprint or plan QA round that has
  findings, over all of them, before the report is posted
- Phase-end review: the initial implementation set plus `flaky-test-qa`
- Run `flaky-test-qa` earlier when tests changed or instability is suspected
- `schema-reviewer`: not in the reviewer set; no governed interface is
  defined below, so it returns `SKIPPED` if dispatched
- Every QA round, plan or sprint, requires an open PR; the rendered report
  is posted to it every round

## Validation Policy

Phase-end artifact command: `just validate`. quality-mgr passes it to
`rust-qa-agent` as `artifact_commands` with
`artifact_regeneration_required: true` on every `phase_end` assignment. Its
result, reported under `executed_checks.artifacts`, is the required phase-end
execution proof. Phase-end PASS is blocked without it. quality-mgr never runs
it in the foreground.

- Boundary validation command: `just lint` (runs
  `scripts/ci/validate_repo_boundaries.sh`)
- Contract sprint validation: `cargo build --workspace` and `just lint`
- Boundary sprint validation: `cargo test -p <crate>`,
  `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo build --workspace`, `just lint`
- Integration / phase-end validation: `just validate`

## Boundary Enforcement

- Boundary validator: `scripts/ci/validate_repo_boundaries.sh`, run by
  `just lint`; manifests under `boundaries/<crate>/*.toml`
  (`allowed_dependents`, `allowed_dependencies`,
  `allowed_test_double_paths`), index `boundaries/planning.toml`
- Additional boundary docs `ruthless-boundary-qa` must read:
  `docs/architecture.md` (§6 Crate Boundary Table, ADR-002, ADR-006, ADR-009),
  `docs/api-design.md`, `docs/public-api-checklist.md`
- Additional enforcement surfaces (mandatory evidence):
  - `scripts/ci/validate_dependency_bans.sh`
  - `.github/scripts/release_artifacts.py validate-publish-order`
  - `scripts/ci/validate_public_api_diff.sh`
  - `scripts/ci/validate_public_api_semver.py`
- Boundary-relaxation rule: `arch-qa` RULE-007 below. quality-mgr rejects as
  BLOCKING any change that bypasses `scripts/ci/validate_repo_boundaries.sh`
  or `scripts/ci/validate_dependency_bans.sh`.

## Architectural Rules

`arch-qa` enforces these in addition to binding ADRs.

### RULE-001: No `agent-team-mail-*` dependency or import
Severity: BLOCKING

This repo must remain fully independent from ATM crates.

### RULE-002: `sc-observability-types` must remain the leaf crate
Severity: BLOCKING

`sc-observability-types` must not depend on higher-level local crates or ATM
adapters.

### RULE-003: No ATM-specific constants or path/runtime assumptions in generic crates
Severity: BLOCKING

ATM spool/socket/runtime semantics do not belong in this repo.

### RULE-004: Generic config loading must not be hard-wired to ATM-only naming
Severity: IMPORTANT

Prefix-parameterized config APIs are preferred over ATM-only generic APIs.

### RULE-005: Files over 1000 lines of non-test code warrant modularization review
Severity: IMPORTANT

A file exceeding 1000 non-test lines is a signal that a module may be doing too
much or that related concerns have not been separated. Flag it and describe what
logical groupings exist that could become sub-modules. The goal is genuine
simplification — not a mechanical re-export split to hit a line count.

### RULE-006: No hardcoded `/tmp/` paths in production code
Severity: IMPORTANT

### RULE-007: Boundary requirements must not be loosened
Severity: CRITICAL

Any change that weakens an established boundary constraint is a blocking
violation regardless of functional justification. This includes:
- Reordering or widening the crate dependency order in
  `docs/architecture.md` §6 without a lead ruling and ADR
- Adding a banned dependency or an ATM adapter edge without updating the
  boundary record and lead approval
- Removing or bypassing enforcement layers: `scripts/ci/validate_repo_boundaries.sh`,
  `scripts/ci/validate_dependency_bans.sh`, `.github/scripts/release_artifacts.py validate-publish-order`,
  or CI checks

The correct path for any boundary relaxation is:
1. lead ruling
2. ADR or documented decision record
3. boundary record update
4. lint verification

Do not accept `it compiles` or `tests pass` as justification for loosening a
boundary. Reject.

## Plan Naming

Everything in file names, branch names, front matter `id` values and
template variables is lower case. Prose may write "Phase D" and "D.2".

| Thing | Form | Example |
|---|---|---|
| Phase id | next unused single letter, lower case | `d` |
| Sprint id | `<phase>-<n>`, `n` from 1 | `d-2` |
| Plan directory | `docs/plans/phase-<phase>/` | `docs/plans/phase-d/` |
| Phase plan | `docs/plans/phase-<phase>/plan-phase-<phase>.md` | `plan-phase-d.md` |
| Sprint doc | `docs/plans/phase-<phase>/sprint-<phase>-<n>-<slug>.md` | `sprint-d-2-otlp-sync-exporter.md` |
| Plan branch | `plan/phase-<phase>`, PR to `develop` | `plan/phase-d` |
| Phase branch | `integrate/phase-<phase>`, cut from `develop` | `integrate/phase-d` |
| Sprint branch | `sprint/<phase>-<n>-<slug>` | `sprint/d-2-otlp-sync-exporter` |
| Fix layer | `fix/<phase>-<n>-<slug>`; phase-level: `fix/<phase>-<slug>` | `fix/d-2-qa1` |

Phase plans live only in `docs/plans/phase-<phase>/`. Phases A-C keep their
existing names (`sprint-A1.md` etc.); do not rename merged history.
Stack mechanics: `docs/development/gh-stack-guidelines.md`.

## Governed Interfaces

No governed interface policy is currently defined. Until this section names
an interface, its compatibility rule, evidence paths, and approval authority,
`schema-reviewer` returns `SKIPPED`.

## Repository Exceptions

- `arch-qa` / `req-qa`: this repository has no `docs/<crate>/architecture.md`
  or `docs/<crate>/requirements.md` files, and `docs/requirements.md` /
  `docs/architecture.md` do not yet use `## REQ-/NFR-/ADR-<DOMAIN>-nnnn`
  sections. Until they do, treat `docs/requirements.md` and
  `docs/architecture.md` as the sole sources, check against the numbered
  requirements and ADRs they contain, and do not raise missing-file or
  missing `adrs`/`requirements` list findings for sprint docs.
