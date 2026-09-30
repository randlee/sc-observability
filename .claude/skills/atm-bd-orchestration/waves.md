# Waves

A wave is its sprint beads (`stage:sprint` with `metadata.wave` and a
matching `wave:<n>` label) and everything under them: the chain steps and
the blocking findings. A sprint whose label and metadata disagree belongs to
no wave until the lead rules. Each wave is one gh stack. Important and minor
findings are children of the phase root: the phase pool, in no wave.

All work a parallax agent runs lands in its wave's single stack, including
phase-pool findings that originated in any wave. A finding keeps its origin
provenance (`sprint_bead`, `wave`, `found_on_layer`, `found_at_commit`). At
dispatch the executing wave is recorded beside it: `bd update <finding>
--actor "$ATM_IDENTITY" --set-metadata exec_wave=<n> --set-metadata exec_stack=<stack>
--set-metadata exec_pr_target=<top>`. Origin is never rewritten to match
execution.

- **Assign ready beads only.** A dev is assigned only beads `bd ready`
  lists, never a blocked bead, and only beads whose `metadata.difficulty`
  matches its model level (`DIFFICULTY_MODELS`). Each dev holds two: the
  task it is working and the next one queued. When it finishes, its change
  is integrated, its sanity check starts and another task is queued, so it
  never idles.
- **Next-up slot.** A higher-priority ready bead takes the next-up slot; the
  task it displaces is cancelled with `task-refused.md.j2` (`bead_state`
  open) and returns to ready. Never stop a dev's in-progress task unless it
  is causing harm: a bad assignment, work in the wrong worktree, and the
  like.
- **No idle devs.** Every dev works 100% until told to stop. A dev with no
  ready wave bead at its model level takes the highest-priority unassigned
  phase-pool finding that matches: important, then minor.
- **Findings stay low.** 100% of important and minor findings close before
  phase end. They are worked as they arrive, never saved for the end: minors
  add up fast.

## Wave Transition

Each wave has its own parallax agent, and it works until told to stop. When
wave N's final blocker closes, the lead spins up a parallax agent for wave
N+1 ([`parallax-lifecycle.md`](parallax-lifecycle.md)) and assigns it that wave ("Assigning A Wave"). Once it is running, the
lead tells parallax-N to stop assigning: it gives out no new work, and its
devs finish what is queued. Parallax-N still dispatches the sanity check and
QA that queued work needs. After its `DRAINED` report the lead spins it down.

A wave is eligible to merge when every sprint bead in it is closed (so every
blocking finding is closed) and CI is green at the top of its stack. The
lead coordinates the transition: it merges the wave's stack onto its base
(`/sc-gh-stack`, `recipe-land.md`), which moves the next wave's base. A wave
base is never `develop` before phase-end approval. Important and minor
findings do not hold the merge; open ones continue on the new base.

## Assigning A Wave

The lead may hand one wave to the `parallax` role. Before the
assignment, the lead:

1. Selects the wave's base: `integrate/phase-<x>`, or the top of the
   previous wave's stack while that wave is incomplete; `develop` only on
   the user's word.
2. Creates the wave's worktree with `/sc-git-worktree` from that base.
3. Spins up a parallax agent ([`parallax-lifecycle.md`](parallax-lifecycle.md)) and assigns it the wave with the phase root,
   the wave, the base branch and commit, the stack (none for a new wave),
   and the devs it owns.
   Nothing is substituted for the recorded base.

When the previous wave restacks or lands, nothing moves by itself: the
wave's stack writer retargets the wave's bottom PR to the new base, rebases
the wave onto it with `/sc-gh-stack` (the PRs, and their review history,
stay), tells each owner whose layer moved, and reports the new base commit and the moved layers to the lead;
their evidence follows "Track restacks" in SKILL.md, Stack Discipline.

The parallax agent runs [`wave-loop.md`](wave-loop.md).

## Overseeing A Wave

The parallax agent executes; the lead verifies it from data. The agent's own
report is not verification. At every report, and at least once between
reports, the lead checks:

- **Queue.** Every dev holds a current and a next-up task, each `bd ready`
  when queued, at a matching difficulty, and higher priority first.
- **Handoffs.** Every closed dev step and every fixed finding is a handoff
  that the beads track at an exact head, so it survives its ATM task
  closing: integrated (`stack_head`), sanity PASS at that head, QA
  disposition at that head, blocking findings closed. Dev-complete is not
  reviewed, and reviewed is not landed. `scripts/wave-monitor` lists every
  handoff that is missing a stage, queued for review, or stale after a
  restack, and every finding still open.
- **One stack.** The lead runs `/sc-gh-stack-view --trunk <wave base>
  --json` itself; the parallax agent's copy is never the lead's evidence. The
  result shows one stack for the wave. Every PR the agent opened is a layer
  of it, in integration order, with no competing stack. Each layer's base is
  its parent's head; local, origin and PR heads agree; `needsRebase` is
  false; and CI ran at the head that was reviewed. Use that one result, never
  per-layer `gh pr view` polling.
- **Rebase before review.** Every sanity check and QA round ran at the
  checked bead's `stack_head`, with PR base `stack_parent`, and `git
  merge-base --is-ancestor <stack_parent> <stack_head>` passes.
- **Evidence.** Each moved layer has its `restack:` note, its old evidence
  unchanged, a sanity PASS at the new head and quality-mgr's disposition
  there.
- **Findings.** Blocking findings are under their sprint and important and
  minor ones under the phase root, with origin provenance intact and `exec_*`
  set on those executed. Important findings are closed only by quality-mgr.
- **Transition.** Merge eligibility, the stop, the drain and the handover
  follow Wave Transition.

A failed check is corrected by the lead, which tells the agent the rule it
broke.

## Sanity Monitor

Each monitoring pass, including every scheduled one, prints the sanity table
to the console itself; ATM task-close text does not count. The pass reads
the ledger and never appends to it:

```bash
.claude/skills/atm-bd-orchestration/scripts/wave-monitor --root <phase root> [--wave <n>]
```

Rows are newest completed first, failed and earlier iterations included,
in the columns `S | PR | Find | ✓ | Done | Iter`. Below the table, name the
timezone of Done, then list separately every discrepancy found by
reconciling each closed sanity task of the phase with the ledger:

- **missing**: a completed sanity task with no row for its iteration;
- **stale**: the row's verdict disagrees with the bead's close reason, or a
  PASS bead's `sanity_pass_commit` is not the checked bead's `stack_head`.

A task close is never read as a PASS; only the ledger row and the pinned
commit are. A discrepancy stays an unresolved contradiction until task and
report evidence settle it; never guess its cause. A row without a commit
stays unknown, never backfilled from a current branch tip. Two identical
rows are a duplicate only when their task and iteration match; the ledger
is never edited to remove one.

Under the discrepancies, flag rows by iteration for the lead's judgement:

| Row | Signal |
| --- | --- |
| iteration 1 FAIL | normal; no flag |
| iteration 2 with more than one finding | suspect |
| iteration 3 FAIL | the lead investigates |
| iteration 4 or later | problem; highlighted |

These are oversight signals only. They do not change a finding's severity,
do not stop the parallax role's other ready work, and are separate from the
round caps (`SANITY.ROUND_CAP`, QA `ROUND_CAP`), which still apply as
written.
