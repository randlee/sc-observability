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
