---
name: atm-beads
version: 0.1.0
description: Write the phase plan as beads (one epic, one bead per sprint, stack-ordered dependencies) and run each ATM task and its bead as one lifecycle.
depends_on:
  codex-orchestration: 0.x
---

# ATM Beads

Beads are the plan and the work graph; ATM tasks are the dispatch and the
span. There are no plan markdown files: the phase is an epic and each sprint
is a bead. A bead id is the ATM `task_id`, and the two open and close
together.

## Identity

- `ATM_IDENTITY` and `BEADS_ACTOR` are already in every agent's environment
  and are equal: the bare pane name (`cobs`), never an alias (`obs-lead`) and
  never a model class (`terra`).
- A bead's assignee is the recipient's `ATM_IDENTITY`.

## Lifecycle

Every assignment is one ATM task and one bead, opened and closed together:
`atm task start` with `bd update <bead> --claim`, and
`atm task close <bead> completed --template <complete> --vars <file>` with
`bd close <bead>`. A push or progress report closes neither.

## Resources

Read only the one the current job needs.

| Resource | Read when |
| --- | --- |
| [`resources/planning.md`](resources/planning.md) | writing or reviewing the plan: the phase epic, sprint beads, their fields, metadata and stack order |
| [`resources/atm-beads-plan-guidelines.md`](resources/atm-beads-plan-guidelines.md) | shaping the sprints themselves: boundaries, closure, tracks, waves, naming (read "sprint doc" as "sprint bead") |
| [`resources/orchestrating.md`](resources/orchestrating.md) | lead work: checking the plan beads, wiring QA and fix dependencies, dispatching from `bd ready` |
| [`resources/quick-check.md`](resources/quick-check.md) | writing or sending the quick-check assignment (recipient and message) |
| [`resources/troubleshooting.md`](resources/troubleshooting.md) | a claim, close or assignee looks wrong, or `bd ready` misses assigned work |
