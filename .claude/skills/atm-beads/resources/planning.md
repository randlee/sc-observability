# Planning in Beads

The plan is written to beads, not to markdown files. The phase is a root
bead (an epic, or a feature under the Development epic). Every sprint is a
container bead under it that carries the sprint doc and is never dispatched.
At plan complete the lead pours each sprint's chain (dev → sanity → QA) under
it. The dependency edges are the order. There is no phase plan or sprint doc
to keep in sync: `bd show <bead>` is the plan.

Shape the sprints with
[`atm-beads-plan-guidelines.md`](atm-beads-plan-guidelines.md). Where it says
"sprint doc", write the same content into the sprint bead as below. The
stack rules below override the guidelines' track stacks.

To turn an existing markdown plan into beads, use
[`importing-md-plan.md`](importing-md-plan.md) instead.

## Writing The Plan

The planner authors the sprints and their dependencies only. Every bead is
rendered from a template, so a missing field fails the render rather than
reaching an agent:

| Bead | Template | Id |
| --- | --- | --- |
| phase root | [`plan-root.json.j2`](../templates/plan-root.json.j2) | `<prefix>-phase-<x>` |
| sprint | [`sprint-bead.json.j2`](../templates/sprint-bead.json.j2) | `<prefix>-<x>-<n>` |
| chain (poured at plan complete) | [`sprint-chain.formula.toml.j2`](../templates/sprint-chain.formula.toml.j2) | `<sprint>.chain`, `<sprint>.chain.dev`, `.chain.sanity`, `.chain.qa` |

1. Run `bd doctor --json`. Any check with `"status": "error"` stops the
   plan; report it to lead.
2. Write one vars file per sprint (examples in [`../examples/`](../examples/))
   and render each strictly into one JSONL file:

   ```bash
   sc-compose render --file .claude/skills/atm-beads/templates/<t>.json.j2 \
     --var-file <scratch>/<bead>-vars.json --strict --output <scratch>/<bead>.json \
     && jq -e -c . <scratch>/<bead>.json >> <scratch>/plan.jsonl
   ```

   Stop on any failure; never pipe a render straight into `jq`, which drops
   a failed bead silently.

3. Validate. This step is mandatory:

   ```bash
   .claude/skills/atm-beads/scripts/validate-plan --mode plan --file <scratch>/plan.jsonl
   ```

   Add `--root <id> --phase <x>` when the root already exists. Exit 5 lists
   every problem. Fix them all and render again.
4. `bd import --dry-run -i <scratch>/plan.jsonl`, then `bd import -i
   <scratch>/plan.jsonl`. Right away, create the plan-review bead
   (`atm-bd-orchestration` "Plan Gate", step 2). Then write the phase
   definition by hand: `docs/plans/phase-<x>/sprints.jsonl` has one
   `[sprint_name, wave, depends_on]` tuple per sprint. It is authored, never
   exported: the planner edits it in the same commit as the bead changes, and
   `validate-plan --mode plan --root <root>` refuses until the sprints under
   the root are exactly those tuples. Then run
   `.claude/skills/sprint-review/scripts/sprint-review --root <root>`, which
   renders the required initial `docs/plans/phase-<x>/phase-<x>-dag.html`
   with embedded SVG and commits/pushes the HTML on the root bead's integration
   branch. No viewer opens without `--view`. Then run
   `validate-plan --mode plan --root <root>` on the imported beads.

The plan then goes to plan review (`atm-bd-orchestration` "Plan Gate").
Plan complete is plan review PASS; nothing is poured or dispatched before it.

Keep `<scratch>` outside the repository.

## Phase definition (`sprints.jsonl`)

`docs/plans/phase-<x>/sprints.jsonl` is the authored, committed graph authority.
Planning five sprints creates five sprint beads through the validated import
JSONL. The planner records one compact tuple for each sprint. Sprint content
(title, deliverables, acceptance, REQ/ADR, ownership, and state) lives only in
Beads.

```jsonl
["d-12", 1, []]
["d-13", 2, ["d-12"]]
["d-14", 2, ["d-12", ["d-13", "qa"]]]
```

Each tuple is exactly `(sprint_name, wave, depends_on[])`. `wave` is an
integer equal to the sprint bead's `metadata.wave`. Each `depends_on` entry
names a direct prerequisite sprint and blocks the dependent's `.chain.dev`:

| Entry | Blocks on |
| --- | --- |
| `"d-12"` | `<prefix>-d-12.chain.sanity` (default) |
| `["d-12", "qa"]` | `<prefix>-d-12.chain.qa` |
| `["d-12", "sprint"]` | the sprint bead `<prefix>-d-12` |

A declared `qa` or `sprint` dependency is kept as declared, never weakened to
the default. The phase root is derived from the path as `obs-phase-<x>`. No
finding, fix, task, branch, or runtime gate appears in this file.

The file is never generated from beads and beads are never generated from the
file. A plan change is one planner transaction: change the beads, edit the
file, commit both. `validate-plan --root <root>` checks the sprints and, once
poured, their chains against it (see `SKILL.md`, Validation). It must stay
green from plan approval to phase end; every template runs it before a claim.

Hierarchy:

- top level: epics only; the phase root is an epic or a `feature` under epics;
- children of the root: exactly the listed sprints, important and minor
  findings, plus `stage:plan*` beads, `bd gate` beads and sprints closed
  "folded into ...";
- under a sprint: its `<sprint>.chain` and its blocking findings
  (`discovered-from` the QA bead that found them);
- under the chain: `.dev`, `.sanity` and `.qa`, linked dev → sanity → qa by
  `blocks` edges;
- under a finding: its fix sanity and fix QA beads. A blocking sanity-FAIL
  finding is a child of the step it checked, so it is under the sprint too;
  an important or minor one goes under the root.

Important and minor findings carry their provenance in metadata:
`sprint_bead`, `qa_bead`, `found_at_commit`, `pr_number`, `wave`. They hold the
phase, never the sprint or the wave.

Closure:

- dev-complete closes the dev step only;
- the lead closes a sprint after `sprint-closable <sprint>` passes: its dev,
  sanity and QA steps are closed and no blocking finding under it is open. Important and minor
  findings never hold it;
- the phase closes when every sprint and every finding is closed.

The initial `phase-<x>-dag.html` is a required plan-review artifact alongside
`sprints.jsonl`. Live-root validation verifies both files on the remote
integration branch and checks that the HTML embeds SVG for this phase root.
Later `/sprint-review` runs refresh and push the same page; `--view` only
controls optional background viewing in Wyvern. Import JSONL validation runs
before beads exist, so it does not require this generated artifact yet.

## Phase Root

| Var | Content |
| --- | --- |
| `title` | the phase title; the bead title becomes `phase-<x>: <title>` |
| `description` | goal and the sprint table |
| `design` | phase-level architecture decisions, the boundaries in scope, retained gates |
| `acceptance_criteria` | the phase-level gates |
| `plan_scope`, `parent` | `feature` under the Development epic, or `epic` with no parent |
| `integration_branch` | `integrate/phase-<x>` |

## Sprint Bead

| Var | Content |
| --- | --- |
| `title` | the sprint title; the bead title becomes `<sprint>: <title>` |
| `description` | goal, deliverables, required work, and what the sprint does not close |
| `design` | public contract, types, code samples, exact targets |
| `acceptance_criteria` | acceptance criteria and the validation commands |
| `parent` | the phase root |

Its labels (`phase-<x>`, `stage:sprint`, `wave:<n>`, `stack:<stack>`,
`train:<t>` when set) and metadata come from these required vars:

| Var | Value |
| --- | --- |
| `sprint` | `d-5` |
| `wave` | integer; equals the `sprints.jsonl` tuple's wave |
| `stack` | `phase-<x>`: the phase is one stack |
| `layer` | planned position, 1 = bottom |
| `pr_target` | planned: branch of layer n−1, or `integrate/phase-<x>` for layer 1 |
| `difficulty` | `hard`, `normal` or `fast`; the dev step's assignee must match it |
| `relation` | `root`, `must_follow` or `parallel_safe` |
| `closure_type` | from the guidelines' closure types |
| `target_boundary` | the one boundary the sprint closes |
| `owned_paths` | files and crates the sprint owns: its file fence |
| `requirements` | every REQ id that governs the work (`LOG-001`, `OTLP-008`, `ATM-BASE-3`, `NFR-…`), or exactly `["NONE"]` |
| `adrs` | every ADR that governs the work (`ADR-011`), or exactly `["NONE"]` |

Optional: `model_class` (`astra`, `terra`, `luna`), `release_train`,
`priority`. The lead sets each step's assignee, branch and worktree at
dispatch.

`requirements` and `adrs` are never left empty. The dev reads each listed id
before coding, QA checks the change against each one, and plan review
rejects a list that is missing, names an id that does not exist or does not
govern the work, or leaves out one the work touches. `NONE` is a claim the
author makes, and plan review checks it like any other.

### New Ids

This is the one exception to "the id must exist", and every other file
links here. A sprint may list a requirement or ADR id that its governing
document does not have yet only when both of these hold:

1. the document is in the sprint's `owned_paths`;
2. a deliverable in the bead's description names the id and says the sprint
   adds it.

`validate-plan` checks the first and reports the id as a warning. Plan
review checks the second; if it fails, the id is unknown and blocking. QA
checks that the definition landed in the document at the reviewed commit;
if it did not, that is a blocking finding.

Branch and id naming follows the repository's "Plan Naming" in
`.claude/project/quality-policy.md` and the shared rules in the guidelines'
"Naming" section; the doc file names there do not apply.

## Plan Complete: Pour The Chains

After plan review passes, the lead pours every sprint's chain, in
`sprints.jsonl` order:

1. `bd children <sprint> --json`:
   - a complete chain (three steps, two `blocks` edges): skip the bond, go to
     step 4;
   - a partial chain: report it. Only that sprint's missing steps are held;
     existing steps and their work are kept. Never re-bond: a repeated bond
     reopens closed steps (sc-compose #613).
2. Render the chain with `sc-compose bead render` from
   `templates/sprint-chain.formula.toml.j2` into
   `.beads/formulas/sprint-chain.formula.toml` (vars: `sprint_bead`, `phase`,
   `sprint`, `wave`, `stack`, `layer`, `pr_target`, `difficulty`, from the sprint
   bead).
3. Bond it once: `bd mol bond sprint-chain <sprint> --type parallel --ref chain
   --var ...`. `--type sequential` would block the chain on its own sprint.
4. Add or reconcile the cross-sprint `blocks` edges `sprints.jsonl` declares.

Then `validate-plan --mode execution --root <root>` must exit 0 before any
dispatch. The dev-sanity step's assignee is the `dev-sanity` role's member
(`scripts/resolve-role dev-sanity`); the QA step's is quality-mgr. See
[`dev-sanity.md`](dev-sanity.md).

## Stack

- Each wave is one append-only `gh stack` on its base (`integrate/phase-<x>`,
  or the previous wave's top); a landed wave moves the next wave's base. Only
  the final phase PR targets `develop`.
- `layer` and `pr_target` are the plan's intent. Layers really stack in the
  order they complete; the stack writer records the actual `stack_parent`
  and `stack_head` when it integrates each one (`atm-bd-orchestration`
  "Stack Discipline").
- Sprints that can run at once must have disjoint `owned_paths`. Sprints that
  share a path must be ordered: one's sanity step in the other's blocker
  closure.

The stack table is a query, not a document:

```bash
bd list -l phase-d,stage:sprint -n 0 --json | jq -r '.[] | [.metadata.stack, .metadata.layer, .metadata.wave, .metadata.sprint, .metadata.pr_target] | @tsv' | sort
```

## Checks

`validate-plan` is the check: `--mode plan` until plan complete, then
`--mode execution`. The plan is not ready for plan review until plan mode
exits 0. It fails when:

- `bd doctor` reports an error;
- a bead lacks a required field, label or metadata key;
- `requirements` or `adrs` is empty, mixes `NONE` with ids, holds a
  malformed id, or names an id that its governing document does not have;
- a sprint's `wave:<n>` label, `metadata.wave` and `sprints.jsonl` wave
  disagree;
- a `root` sprint has prerequisites, or a `must_follow` sprint has none;
- two beads claim the same `stack` and `layer`, or a layer's `pr_target` is
  not the branch of the layer below;
- a sprint's parent is not the phase root, or its phase does not match;
- execution mode: a sprint lacks exactly one complete chain (chain parent =
  the sprint; dev, sanity and qa parent = the chain; dev → sanity → qa
  `blocks` edges), or a dev step's blockers differ from the declared
  cross-sprint dependencies;
- an assignee is not an ATM member of the team, or a sanity step's assignee
  is not the `dev-sanity` role's member.
