# AGENTS Instructions for sc-observability
<!-- bd-doctor-divergence: ok -->

## Critical Workflow Rule

Do not switch the primary checkout away from `develop` for sprint work.

- Keep the primary repo checkout on `develop`
- Use git worktrees for all feature/sprint branches
- All branches other than `develop` must live in worktrees

## Project Overview

`sc-observability` is a standalone observability workspace.

It contains:
- `sc-observability-types`
- `sc-observability`
- `sc-observability-otlp`

This repo is intentionally independent from ATM. Do not introduce
`agent-team-mail-*` dependencies or ATM spool/socket/runtime assumptions.

## Product Bar

sc-observability is a general-purpose logging library: a new project gets
structured logging and OTel immediately. Every change is held to: consistent,
clean, easy to use, high-performance.

- Never remove a capability because no current consumer uses it. Remove only
  duplicates of a kept item, 1.x surfaces with a canonical replacement, test
  seams (moved to `#[cfg(test)]`), and internal plumbing.
- Deprecate before removing. A public 1.x item first ships behind the
  default-on `v1` feature with `#[deprecated(note = "<2.0 replacement>")]`;
  the next release deletes it. A deprecated item need not keep working: no
  compatibility adapters, tests or baseline comparisons for it.
- `sc-observability-log` replaces the same-named crate in
  beads-task-issue-tracker; keep every capability that app needs.

## Key Documents

- [`docs/requirements.md`](./docs/requirements.md)
- [`docs/architecture.md`](./docs/architecture.md)
- [`docs/project-plan.md`](./docs/project-plan.md)
- [`docs/git-workflows.md`](./docs/git-workflows.md)
- [`docs/cross-platform-guidelines.md`](./docs/cross-platform-guidelines.md)
- [`docs/team-protocol.md`](./docs/team-protocol.md)
- [`.claude/skills/rust-development/guidelines.txt`](./.claude/skills/rust-development/guidelines.txt)

## Boundary Rules

1. No crate in this repo may depend on `agent-team-mail-*`.
2. Shared neutral types belong in `sc-observability-types`.
3. ATM-specific adapters stay outside this repo.
4. Explicit inputs are required for file/output paths; do not derive them from ATM helpers.

## Team Communication

If this repo is being run with ATM team workflow enabled, follow
[`docs/team-protocol.md`](./docs/team-protocol.md) for all ATM messages.

## Execution Rules

Ready means execute. Infer no holds; stop only for an explicit hold or concrete
blocker. Parent WIP or rebasing affects finalization, not starting scoped work.

No silent blocking or completion. Routine execution may be quiet. Report blockers,
meaningful state changes, and completion; omit routine narration. Run required
validation, then report completion promptly.

Delegate status formatting and protocol bookkeeping without delaying work; the
worker remains responsible for evidence. The lead must make the
`just-say-no-to-process-porn-and-ceremony` skill available in every agent's skill
catalog.

Acknowledge only `requires_ack` messages, immediately. Terminal task closes need
no extra acknowledgement. Follow `docs/team-protocol.md` for ATM task lifecycle.

## Honest Work, No Ceremony (binding on agents and humans)

Goal: working, deployable capability. Process serves it and never becomes the product.

- **Process artifacts** (certificates, ledgers, dashboards, matrices, meta-reports, speculative checks) need four things to exist: a named consumer, the feature it gates, the observed defect class that justifies it, and a deletion condition. Missing any of these → don't create it. Test: if running code branches on it, it's product. If only humans or reports read it, it's process and needs the four things. Writing code just to flip that answer is itself the pathology. Exception: a minimal integrity/recovery control (crash-recovery state, provenance snapshot) that prevents a named evidence-loss or corruption mode.
- **Real code + real tests in the same unit of work.** Forbidden: faked tests, mocks/fixtures presented as live proof, weakened assertions, regenerating goldens to force green, hard-coded success paths, placeholder macros in commits, editing the spec instead of implementing it, narrowing scope while claiming full success.
- **No self-certification.** An independent verifier closes the work, citing evidence at an exact revision. In solo sessions, re-verify by re-running, and state what wasn't independently verified.
- **Typed refusal beats fabrication but is worth less than real capability.** Work that ends only in a refusal stays open and says so.
- **Null results are successes** ("checked X, found no material increment"). Unsupported claims are worse than silence.
- **Metrics** predeclare their denominator and countermetric. Agreement between agents can raise confidence but is never independent evidence. Never silence stderr in commands that produce evidence.
- **Name pathologies when they occur:** gate self-weakening, proof-class inflation, golden regeneration, tolerance widening, suppression-pragma laundering, refusal farming, follow-up laundering. The full catalog and countermeasures are in the `just-say-no-to-process-porn-and-ceremony` skill. Ask the operator for it if you can't find it.
