---
name: atm-beads
version: 0.3.0
description: Plans written as beads. Use when writing, validating or importing a phase plan into beads, or when pairing an ATM task with its bead (claim, start, close).
requires:
  cli:
    - name: bd
      minimum_version: 1.3.0
    - name: atm
    - name: sc-compose
    - name: jq
depends_on:
  atm-bd-orchestration: 0.x
---

# ATM Beads

Beads are the plan and the work graph; ATM tasks are the dispatch and the
span. There are no plan markdown files: the phase is an epic and each sprint
is a dev bead followed by a sanity check bead. A bead id is the ATM
`task_id`, and the two open and close together.

## Step 1 — Verify CLI Installation

Run this before anything else in the skill:

```bash
for c in bd atm sc-compose jq; do command -v "$c" >/dev/null && echo "ok $c" || echo "MISSING $c"; done
bd version    # 1.3.0 or newer
```

If anything is missing or too old, **read
[`references/installation-and-troubleshooting.md`](references/installation-and-troubleshooting.md)
before proceeding.**

## Identity

- `ATM_IDENTITY` and `BEADS_ACTOR` are already in every agent's environment
  and are equal: the bare pane name (`team-lead`), never an alias (`obs-lead`) and
  never a model class (`terra`).
- A bead's assignee is the recipient's `ATM_IDENTITY`.

## Lifecycle

Every assignment is one ATM task and one bead, opened and closed together,
and the task id is the bead id: `bd update <bead> --claim` then
`atm task start`, and `bd close <bead>` with
`atm task close <bead> completed --template <complete> --vars <file>`. The
templates and the not-ready rule are in the `atm-bd-orchestration` skill. A
push or progress report closes neither.

## Resources

Read only the one the current job needs.

| Resource | Read when |
| --- | --- |
| [`resources/planning.md`](resources/planning.md) | writing or reviewing the plan: the phase root, dev and sanity check beads, their fields, metadata and stack order |
| [`resources/atm-beads-plan-guidelines.md`](resources/atm-beads-plan-guidelines.md) | shaping the sprints themselves: boundaries, closure, tracks, waves, naming (read "sprint doc" as "sprint bead") |
| [`resources/orchestrating.md`](resources/orchestrating.md) | lead work: checking the plan beads, wiring QA and fix dependencies, dispatching from `bd ready` (templates: the `atm-bd-orchestration` skill) |
| [`resources/importing-md-plan.md`](resources/importing-md-plan.md) | importing an existing markdown plan into beads: one sprint doc, or a whole phase; includes the missing-info checks |
| [`resources/dev-sanity.md`](resources/dev-sanity.md) | writing or sending the sanity check assignment (recipient and message) |
| [`resources/troubleshooting.md`](resources/troubleshooting.md) | a claim, close or assignee looks wrong, or `bd ready` misses assigned work |

Every phase plan has a tracked `.atm-bd/<phase>.toml` naming `root`,
`sprints = "<plans_dir>/<phase>.jsonl"`, and `integration_branch`.
Each plan line is `{"sprint":"<id>"}` with optional `"depends_on":["<id>"]` and no other fields.
Sprint additions or removals require replanning.
Plan difficulty as `hard`, `normal`, or `fast`; name agents only at dispatch.
Optimize for parallel execution; record only dependencies that completely block the dependent sprint.
Default the dependent dev bead to blocking on predecessor sanity; use its sprint bead only when the user requests it.
Validation accepts either predecessor edge; guidance edges and waves stay out of the plan.

## Validation

Create the phase TOML before pre-import validation; run from the repository root:

```bash
.claude/skills/atm-beads/scripts/validate-plan --file <beads.jsonl> --root <root>  # pre-import; no writes
.claude/skills/atm-beads/scripts/validate-plan --root <root> --refresh  # plan gate; regenerate DAG
.claude/skills/atm-beads/scripts/validate-plan --root <root> --scope <bead>  # assignment; no writes
```

Only the plan gate uses `--refresh`; assignment checks need no renderer.
`--file` and `--beads` validate pre-import input without generating HTML.
The live DAG is `<plans_dir>/<phase>-dag.html`, beside the plan; commit both with the phase TOML.
The phase's own TOML is authoritative; `current-phase.toml` never overrides its plan path.
`--ci` checks plan schema and committed HTML sprint membership without Beads or ATM.

Live validation exits 5 for these contract problems on stdout as `<bead>: <problem>`:
- invalid plan schema;
- planned sprint without a bead;
- sprint bead under the root absent from the plan;
- configured integration branch differs from root `metadata.integration_branch`;
- a declared dependency lacks an edge to predecessor sanity or sprint.

Offline `--ci` also exits 5 for invalid HTML structure or a plan/HTML sprint-set mismatch.
Exit 2 means validation cannot run, including unavailable Beads, unreadable files/output, or missing/malformed phase TOML.
Every nonzero exit blocks plan approval; report every printed contract problem.
Other checks, including bead schema, sanity labels/count, doctor, ATM evidence, and DAG rendering, emit nonfatal warnings.
The implementation and shared contract are in `scripts/validate-plan` and `scripts/plan_contract.py`.
