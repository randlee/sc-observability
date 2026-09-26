---
name: sprint-report
description: Generate a sprint status report for the current phase. Default is --table.
---

# Sprint Report Skill

Run the repository-local report command from the main checkout:

```bash
.claude/skills/sprint-report/scripts/sprint-report --table
```

Use `--detailed` for one block per sprint. The command reads the committed
`docs/plans/phase-<p>/sprints.json`, then refreshes bead state and PR/CI state.
Rows are never hand-typed.

## Data sources

The index supplies sprint identity, layer, branch target, dependencies, planned
assignee, sanity/QA bead IDs, deliverable count, owned paths, requirements and
ADRs. For each row the command uses `bd show --json` for the dev, sanity and
QA beads, counts open finding children of the selected QA bead, and uses
`gh pr list --state all --json ...` to match the dev branch and `pr_target`.
The integration row matches the phase integration branch into `develop`.

The QA cell is explicit per sprint: `R<round> <verdict> (<open> open)`; a
sprint without a QA bead is `not dispatched`. This is the authoritative
round/verdict/open-finding presentation for the table.

## Render command

The template path is relative to the main repository root. The script invokes:

```bash
sc-compose render --file .claude/skills/sprint-report/report.md.j2 --var-file <json>
```

To render manually, write the variables JSON produced by the script to a
temporary file and run the same command from the repository root.

## Table variables

```json
{
  "mode": "table",
  "sprint_rows": "| d-12 | ✅ | R1 FAIL (16 open) | 🏁 | #233 |",
  "integration_row": "| **integrate/phase-d** | | — | 🌀 | — |"
}
```

## Detailed variables

```json
{
  "mode": "detailed",
  "sprint_rows": "Sprint: d-12  types 2.0 contract\nDEV: ✅\nQA: R1 FAIL (16 open)\nCI: 🏁\nPR: #233",
  "integration_row": "Integration: integrate/phase-d → develop\nCI: 🌀\nPR: —"
}
```

## Icon reference

| State | DEV | QA | CI |
|-------|-----|----|----|
| Assigned | 📥 | not dispatched | |
| In progress | 🌀 | IN PROGRESS | 🌀 |
| Done/pass | ✅ | PASS | ✅ |
| Findings/fail | | FAIL | ❌ |
| Blocked | 🚧 | | |
| Merged | | | 🏁 |
