# Orchestrating with Beads

For the lead: check the plan in beads, keep the dependency graph current, and
dispatch from `bd ready`. The plan itself (epic and sprint beads) is written
as in [`planning.md`](planning.md).

Never block on bureaucracy. Dev waits only on its prerequisites' quick-checks; QA, triage and
fixes run beside it. 100% of findings are closed before the phase closes, and
one or two fast agents keep up with the important and minor ones.

## Before Dispatch

1. Every sprint bead passes the checks in [`planning.md`](planning.md)
   ("Checks").
2. `bd ready -l phase:<x>` lists exactly the dev beads with no prerequisites.
3. `bd ready --explain` shows every other dev bead blocked by the
   quick-check beads of its prerequisites.
4. The dispatched agent reads its assignment from the bead: `bd show <bead>`
   gives the description, design, acceptance criteria and metadata (branch,
   `pr_target`, worktree).

## Dependencies

Plan time creates the dev and quick-check beads. At runtime lead creates the QA
beads and quality-mgr creates the finding beads.

| Edge | Type | Meaning |
| --- | --- | --- |
| dev → quick-check | `blocks` | quick-check runs on the closed dev bead |
| quick-check → dependent dev | `blocks` | a dev bead waits on the quick-check of each required prerequisite |
| `parallel_safe` | none | no edge |
| QA → dev | `validates` | records the layer the QA reviewed; blocks nothing |
| finding → QA | `discovered-from` | records the QA that found it; blocks nothing |
| finding → dev | `caused-by` | records the layer it was found on; blocks nothing |
| finding → finding | `blocks` | only when one fix needs another first |

The phase feature is the parent of every bead, not a blocker. It cannot close
while any child, including a finding, is open.

## Quick-Check

Every dev bead is followed by a quick-check bead, which is blocked by the dev
bead and blocks every dev bead that requires it. Its assignment is
[`quick-check.md`](quick-check.md).

1. The dev agent completes its dev task and closes it; lead receives the
   dev-task completion.
2. Lead assigns the quick-check.
3. If quick-check fails, lead reopens the dev bead and gives the dev agent a
   dev-fix assignment with the bead id and the findings (this may change once
   the quality-mgr process is worked out):

   ```bash
   bd reopen <dev-bead-id> --reason "<what quick-check found>"
   ```

4. Steps 1 to 3 repeat until the task is done.

The dev agent may instead close the bead and declare failure, with the reason
the work cannot be completed.

## QA And Findings

Nothing waits on QA. A dev bead is released by its prerequisites'
quick-checks alone, so dev keeps moving while a layer is reviewed.

1. Quick-check N is green: lead creates the QA bead (parent the phase
   feature, `validates` dev N, metadata `sprint`, `layer` and the pinned head
   SHA) and assigns it to quality-mgr.
2. quality-mgr runs its reviewer subagents as today, triages the results and
   screens them with its `ceremony-finding-screen` subagent.
3. quality-mgr files one finding bead per finding, screened or not: parent
   the phase feature, `discovered-from` the QA bead, `caused-by` dev N,
   metadata `severity`, `stack` and `found_on_layer`. The screen verdict
   decides what happens to it:
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
6. Each fix lands on the stack the finding was found on, one fix per layer.
   Fix layers can land between dev layers of the same stack.
7. A fix is assigned to a dev like a dev task: the finding bead is the task,
   followed by a quick-check bead and then QA, as for a dev bead. Each QA
   round after the first reviews one small fix layer, so it is quick.

Priority is the queue order. `bd ready` sorts by it, so a blocking finding is
next up for its assignee, ahead of the next dev bead.

| Severity | Priority | Typical assignee |
| --- | --- | --- |
| blocking | P1 | frontier / high-value dev |
| (planned dev beads) | P2 | frontier / high-value dev |
| important | P3 | fast agent |
| minor | P4 | fast agent |

Lead picks the team member for each finding. The assignee column is the usual
case, not a rule: an important but difficult finding may go to a frontier dev,
and a blocking doc fix may go to a fast agent.

## Stacking

Dev and fix work runs in parallel and is stacked when it completes: the dev
completes the work, rebases it onto the current top of its stack, and the
stack writer (lead) links it. Layers therefore stack in the order they
complete, and lead records each bead's actual `layer` and `pr_target` when it
is linked; the plan-time values are the plan's intent. The mechanics are
`/sc-gh-stack`: its `workflow.md`, `recipe-cut-layer.md` and `recipe-link.md`.

## Lifecycle

Each assignment is one ATM task and one bead, opened and closed together.

| Step | ATM | Bead |
| --- | --- | --- |
| dispatch (lead) | `atm task assign <agent> --task-id <bead> --template <assignment> --vars <file>` | assignee already set |
| start (assignee) | `atm task start <bead> "<one line>"` | `bd update <bead> --claim` |
| done (assignee) | `atm task close <bead> completed --template <complete> --vars <file>` | `bd close <bead>` |
| refused (assignee) | `atm task close <bead> refused "<reason>"` | stays open; `bd update <bead> --notes "<reason>"` |

The complete templates are listed in `docs/team-protocol.md` (Close
Templates). A push or progress report closes neither the task nor the bead.

## Dispatch Loop

1. `bd ready -l phase:<x>` lists the dev, quick-check, QA and finding beads
   whose blockers are closed, highest priority first.
2. Assign each ready bead to its assignee with the matching assignment
   template, using the bead id as `task_id`.
3. When a task closes, its bead closes with it; run `bd ready` again. After a
   green quick-check or a triaged QA, create and wire the QA or finding beads
   first, then run `bd ready`.
