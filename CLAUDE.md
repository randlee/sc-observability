# Claude Instructions for sc-observability

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
