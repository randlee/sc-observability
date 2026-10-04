---
name: sprint-review
description: Render a local phase dependency diagram. Optionally view it in Wyvern with --view.
---

# Sprint Review

Run from the working repository or worktree:

```bash
.claude/skills/sprint-review/scripts/sprint-review
# Only when the user requests a viewer:
.claude/skills/sprint-review/scripts/sprint-review --view
```

`--root <bead-id>` and `--index <path>` select a phase when needed.

Every run reads the configured canonical phase plan, queries Beads/ATM for
current state, and renders a local self-contained HTML page.
Historical phases may still use `<plans_dir>/phase-<p>/sprints.jsonl` tuples.

Report and DAG tools never commit or push and never rewrite the plan.
Only the plan gate's `validate-plan --root <root> --refresh` writes the canonical
`<plans_dir>/<phase>-dag.html` beside `<plans_dir>/<phase>.jsonl`.
The author commits the plan and canonical diagram through normal review.
Sprint review renders scratch outputs for inspection; it is not a publication step.

Without `--view`, do not launch a viewer, render an inline image, or otherwise
show the diagram. Report only the saved local path. With `--view`,
open the HTML in Wyvern if available, initially sized to 80% of the screen's
logical width and height. Missing or failing Wyvern must not prevent
local rendering. Do not substitute Preview or a browser without a request.

The page embeds SVG directly with zoom controls and state tooltips; it needs no
server or installed dependencies to remain readable after download. Wyvern is
only the optional viewer. Rendering requires Python, Node and the pinned
[sprint-report renderer](../sprint-report/renderer/package.json):
`npm ci --prefix .claude/skills/sprint-report/renderer --ignore-scripts`.

For report columns, badge meanings and evidence sources, see
[sprint-report](../sprint-report/SKILL.md). Use
`sprint-report --dag --output <prefix>` to choose a local export path.
