# Orchestrating with Beads

For the lead: check the plan in beads, keep the dependency graph current, and
dispatch from `bd ready`. The plan itself (epic, sprint beads and their
poured chains) is written as in [`planning.md`](planning.md).

Never block on bureaucracy. Dev waits only on its prerequisites' sanity checks; QA, triage and
fixes run beside it. 100% of findings are closed before the phase closes, and
one or two fast agents keep up with the important and minor ones.

## Before Dispatch

1. The plan has passed plan review (`atm-bd-orchestration` "Plan Gate"),
   every chain is poured ([`planning.md`](planning.md) "Plan Complete"), and
   `.claude/skills/atm-beads/scripts/validate-plan --mode execution --root <root>`
   exits 0 (it runs `bd doctor` and every check in "Checks").
2. `bd ready -l phase-<x> -n 0` lists exactly the dev steps of sprints with no
   prerequisites.
3. `bd ready --explain` shows every other dev step blocked by the steps its
   prerequisites declare (the sanity step by default).
4. The dispatched agent reads its assignment from its step and the sprint
   bead (`metadata.sprint_bead`): description, design, acceptance criteria
   and metadata (`pr_target`, difficulty). The lead records branch and
   worktree on the step at dispatch.

## Dependencies

Plan time creates the sprint beads; plan complete pours each chain. At
runtime quality-mgr creates the finding beads, and the lead creates the fix
sanity and fix QA beads under each finding.

| Edge | Type | Meaning |
| --- | --- | --- |
| dev step → sanity step | `blocks` | sanity runs on the closed dev step |
| sanity step → QA step | `blocks` | QA runs after sanity PASS |
| prerequisite's sanity step → dependent dev step | `blocks` | default cross-sprint dependency; a plan may declare the QA step or the sprint instead |
| `parallel_safe` | none | no edge |
| finding → QA | `discovered-from` | records the QA that found it; blocks nothing |
| finding → finding | `blocks` | only when one fix needs another first |

Parents carry membership and closure, never blocking. `bd close` refuses a
parent with an open child: a sprint stays open while its chain or a blocking
finding is open, and the phase stays open while any sprint or finding is.

## Dev Sanity Check

Every dev step is followed by its sanity step, which blocks the QA step and
every dev step that declares it. Its assignment is
[`dev-sanity.md`](dev-sanity.md).

1. The dev agent completes the dev step and closes it; lead receives the
   dev-task completion. The sprint stays open.
2. Lead assigns the sanity step.
3. If sanity check fails, the sanity member files every blocking failure as a
   child finding of the checked step, and every important or minor one under
   the phase root with provenance. It adds `blocks` edges only between those
   new beads, where one fix depends on another. Lead reviews those findings,
   then reopens the dev step and gives the dev agent a dev-fix assignment:

   ```bash
   bd reopen <dev-step-id> --reason "<what sanity check found>"
   ```

4. Steps 1 to 3 repeat until the task is done.

The dev agent may instead declare failure, with the reason the work cannot be
completed. It sets the step `blocked` with a `failed:` note and does not close
it: a closed dev step would release its sanity step.

## QA And Findings

Nothing waits on QA unless a plan declares it. A dev step is released by its
prerequisites' sanity steps by default, so dev keeps moving while a layer is
reviewed.

1. Sanity step N passes: its QA step is ready. Lead records the pinned head
   SHA and `layer` on it and assigns it to quality-mgr.
2. quality-mgr runs its review agents as background agents, as it does today,
   triages their results, and screens them with the `ceremony-finding-screen`
   procedure (`.claude/agents/ceremony-finding-screen.md`), also run as a
   background agent.
3. quality-mgr files one finding bead per finding, screened or not,
   `discovered-from` the QA bead, metadata `severity`, `stack`,
   `found_on_layer` and the provenance in [`planning.md`](planning.md)
   ("Hierarchy"). A blocking finding's parent is the sprint; an important or
   minor finding's parent is the phase root. The screen verdict decides what
   happens to it:
   - `ceremony`: filed and closed at once, not fixed:
     `bd close <finding> --reason "ceremony: <reason>"`;
   - `concern_valid_remedy_ceremony`: filed open, with the remedy rewritten
     to the existing mechanism the screen names;
   - `keep` or `not_applicable`: filed open as reported.
   Every finding closes with a close reason, fixed or not.
4. If lead disagrees with a ceremony closure, lead reopens the finding:
   `bd reopen <finding> --reason "<why>"`.
5. Findings are `parallel_safe` by default. Add a `blocks` edge between two
   findings only when one fix needs the other first.
6. Each fix lands on the stack of the wave that runs it, one fix per layer.
   Fix layers can land between dev layers of the same stack.
7. A fix is assigned to a dev like a dev task: the finding bead is the task,
   followed by a fix sanity bead and then a fix QA bead, both children of the
   finding. Each QA round after the first reviews one small fix layer, so it
   is quick. Only quality-mgr closes an important finding.
8. When the QA step and every blocking finding under the sprint are closed,
   lead runs `sprint-closable <sprint>`, closes the chain, then the sprint.

Priority is the queue order. `bd ready` sorts by it, so a blocking finding is
next up for its assignee, ahead of the next dev step.

| Severity | Priority | Typical assignee |
| --- | --- | --- |
| blocking | P1 | frontier / high-value dev |
| (planned dev steps) | P2 | frontier / high-value dev |
| important | P2 | fast agent |
| minor | P4 | fast agent |

Lead picks the team member for each finding. The assignee column is the usual
case, not a rule: an important but difficult finding may go to a frontier dev,
and a blocking doc fix may go to a fast agent.

## Passing A Bead To A Background Agent

This works the same for Claude and Codex agents. A background agent (a
subagent or child agent, whichever the harness provides) is never assumed to
run `bd`. The agent that starts it fetches the bead and pipes the content into
the background agent's prompt file:

```bash
{ printf '%s\n\n' "<instruction>"
  printf 'The bead:\n\n```json\n'
  bd show <bead> --json | jq '.[0] | {id, title, description, design, acceptance_criteria, metadata}'
  printf '```\n'
} > <scratch>/<bead>-prompt.md
```

1. `bd show --json` returns a one-element array; `.[0]` is the bead.
2. Start one background agent per bead with the contents of
   `<scratch>/<bead>-prompt.md` as its prompt. Start them together when
   there are several, up to the harness's concurrency limit; the rest start
   as slots free up.
3. The background agent reads the bead from the fenced JSON block, does the
   work against the worktree named in the instruction, and returns its result
   as text. It does not write to beads or ATM.
4. The agent that started it applies the result: closes or reopens beads,
   files findings, closes its own ATM task.

Keep `<scratch>` outside the repository and never commit the prompt files.

## Stacking

Dev and fix work runs in parallel and is stacked when it completes: the dev
completes the work, rebases it onto the current top of its stack, and the
stack writer (lead) links it. Layers therefore stack in the order they
complete, and lead records each bead's actual `layer` and `pr_target` when it
is linked; the plan-time values are the plan's intent. The mechanics are the
`sc-gh-stack` skill (`/sc-gh-stack` in Claude): its `workflow.md`,
`recipe-cut-layer.md` and `recipe-link.md` are plain markdown any agent can
follow.

## Lifecycle

Each assignment is one ATM task and one bead, opened and closed together; the
task id is the bead id. The pairing (ready check, claim, start, close,
refusal) is defined once, in the `atm-bd-orchestration` skill ("Dispatch").

## Dispatch Loop

1. `bd ready -l phase-<x> -n 0` lists the dev, sanity and QA steps, and the
   finding, fix sanity and fix QA beads, whose blockers are closed, highest
   priority first. Sprint beads and chains are never dispatched.
2. Assign each ready bead to its assignee with the matching assignment
   template, using the bead id as `task_id`.
3. When a task closes, its bead closes with it, except after a failed sanity check or plan review, which leaves the bead open. Run `bd sync`, then `bd ready` again. After a
   triaged QA, file and wire the finding beads first, then run `bd ready`.
