# Planning in Beads

The plan is written to beads, not to markdown files. The phase is an epic,
every sprint is a bead under it, and the stack order is the dependency graph.
There is no phase plan or sprint doc to keep in sync: `bd show <bead>` is the
plan.

Shape the sprints with
[`atm-beads-plan-guidelines.md`](atm-beads-plan-guidelines.md); where it says
"sprint doc", write the same content into the sprint bead as below.

## Phase Epic

```bash
bd create "Phase D — <title>" -t epic -l phase:d \
  --body-file <scratch>/phase-d.md \
  --design-file <scratch>/phase-d-design.md \
  --metadata '{"phase":"d","integration_branch":"integrate/phase-d"}' --silent
```

| Field | Content |
| --- | --- |
| title | `Phase <X> — <title>` |
| description | goal, tracks, and one stack rule per stack (what orders its layers) |
| design | phase-level architecture decisions and the boundaries in scope |
| labels | `phase:<x>` |
| metadata | `phase`, `integration_branch` |

## Sprint Bead

```bash
bd create "D.5 — OTLP 2.0 signal model" -t task --parent <epic> -a aobs \
  -l phase:d,stage:dev,stack:otlp,train:2.0 \
  --deps <layer-below bead> \
  --body-file <scratch>/d-5.md --design-file <scratch>/d-5-design.md \
  --acceptance "$(cat <scratch>/d-5-acceptance.md)" \
  --metadata @<scratch>/d-5-meta.json --silent
```

| Field | Content |
| --- | --- |
| title | `<display id> — <title>` (`D.5 — OTLP 2.0 signal model`) |
| description | goal, scope, deliverables, and what the sprint does not close |
| design | public contract, types, code samples, exact targets |
| acceptance | acceptance criteria and the validation commands |
| assignee | the ATM identity that owns it (`aobs`) |
| parent | the phase epic |
| deps | the layer below, plus any other `depends_on` |
| labels | `phase:<x>`, `stage:dev`, `stack:<name>`, `train:<x>` when set |
| metadata | see below |

Sprint metadata (all keys lower case):

| Key | Required | Value |
| --- | --- | --- |
| `sprint` | yes | `d-5` |
| `stack` | yes | stack name |
| `layer` | yes | position in the stack, 1 = bottom |
| `branch` | yes | `sprint/<phase>-<n>-<slug>` |
| `pr_target` | yes | branch of the layer below, or `integrate/phase-<x>` for layer 1 |
| `worktree` | yes | `<repo>-worktrees/<branch>` |
| `relation` | yes | `root`, `must_follow` or `parallel_safe` |
| `closure_type` | yes | from the guidelines' closure types |
| `target_boundary` | yes | the one boundary the sprint closes |
| `owned_paths` | yes | files and crates the sprint owns |
| `model_class` | no | `astra`, `terra`, `luna` |
| `requirements` | no | requirement ids |
| `adrs` | no | ADR ids |

Branch and id naming follows the repository's "Plan Naming" in
`.claude/project/quality-policy.md` and the shared rules in the guidelines'
"Naming" section; the doc file names there do not apply.

## Stacks

- All phase work lands on `integrate/phase-<x>`. Only the final phase PR
  targets `develop`.
- Each independent track is one `gh stack` on `integrate/phase-<x>`; its
  sprints are layers, bottom first.
- A layer is cut from the frozen top of the layer below and its PR targets
  that layer (`pr_target`). A layer freezes when its bead closes; later
  findings are fixed on a new layer at the top of the stack.
- Tracks with disjoint `owned_paths` are separate stacks in parallel. Sprints
  that must share a file are layers of one stack.
- A fork (two sprints on one layer) is a second stack based on that layer's
  branch: `gh stack init --base <parent branch> <child>`.

The stack table is a query, not a document:

```bash
bd list -l phase:d,stage:dev --json | jq -r '.[] | [.metadata.stack, .metadata.layer, .metadata.sprint, .metadata.branch, .metadata.pr_target, .assignee] | @tsv' | sort
```

## Checks

The plan is not ready for dispatch when:

- a sprint bead lacks a required metadata key, an assignee, a description,
  a design or acceptance criteria;
- `pr_target` is not the `branch` of the bead at `layer - 1` in the same
  stack (or `integrate/phase-<x>` for layer 1);
- the bead is not blocked by the bead of the layer below;
- two beads claim the same `stack` and `layer`;
- a sprint bead has no epic parent.
