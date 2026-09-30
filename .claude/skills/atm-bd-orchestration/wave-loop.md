# Wave Loop

The parallax role runs this loop for its wave on every ATM receipt and every
task close, not only at a scheduled monitor pass. Run the steps in order;
recording takes seconds, so refill is never starved. Every step leaves
evidence the lead can check (waves.md, "Overseeing A Wave"); a step that
leaves none did not happen. Every `bd` write passes `--actor
"$ATM_IDENTITY"`, and `validate-plan` runs after each one.

## 1. Read The Receipt

- Read it with `atm read --message-id <id>`. Acknowledge only a
  `requires_ack` message, with native `atm ack`, and never acknowledge an
  acknowledgement.
- A task completion is not a verdict. A completed sanity or QA task may
  carry FAIL, and a closed task does not close its finding. Read the verdict
  from the bead and the ledger.

## 2. Record The Handoff

At every dev or fix completion, record on the bead before anything else:

- `source_commit`: the head the dev reported, after you verify it: `git
  fetch origin` and `git rev-parse origin/<branch>` must equal it. On a
  mismatch record both refs as they are (`source_commit_reported`,
  `source_commit_origin`), escalate, and do not integrate it;
- the base branch and SHA it was cut from;
- `exec_wave` and `exec_stack` (origin provenance is left as it is);
- the branch, the PR and the dev's test result.

Later steps add `stack_parent`, `stack_head`, the sanity task and its
verdict and head, the QA task and its verdict and head, and the finding ids
with who may close each. A task close advances this record and never
removes it.

## 3. Refill

- `atm task list --all`. An idle member can still hold an active task that
  is blocked. Each dev holds its current task plus one ready task queued; an
  empty or single-task queue is actionable now.
- `bd ready -n 0 --json`, then `bd show <id> --json` for each candidate,
  before any claim. Once claimed, a bead is `in_progress` and leaves
  `bd ready`, so a late preflight reports a false `NOT_READY`.
- Eligible: unassigned, ready, and `metadata.difficulty` matches the dev's
  model (`DIFFICULTY_MODELS`). Difficulty filters; it never ranks.
- Pick from your wave's eligible beads first, highest stored priority first.
  Only when none is eligible, pick the highest-priority eligible phase-pool
  finding: important, then minor.
- Skip a remedy that is already dev-complete and awaiting review, as its
  notes or task receipt show; an open finding is not a reason to dispatch it
  again. Skip work the lead has excluded, such as tooling work.
- Higher-priority work takes the next-up slot. It never interrupts safe work
  already in progress.
- Provision with `/sc-git-worktree` from `origin/<pr_target>` of the
  assignment (for a phase-pool finding, your stack's top, recorded as
  `exec_pr_target`), pin its SHA, and confirm the owning worktree is clean.
  Check the bead's `pr_target`, branch, worktree and difficulty against the
  assignment.
- `atm task assign <dev> --task-id <bead> --template <t> --vars <file>`,
  then run `atm task list --all` to confirm the assignment exists. A prose
  message is not a dispatch.

## 4. Integrate

- Rebase the completed delta onto the immediate parent, the top of your
  stack, with `/sc-gh-stack` (SKILL.md, Stack Discipline step 1). Check
  parent ancestry and the PR base.
- If the change needs a repair that lives in another branch, escalate: the
  lead approves a `pr_target` correction first, and only then do you rebase.
  Never choose another parent yourself.
- Run `/sc-gh-stack-view --trunk <wave base> --json` once and check it:
  local, origin and PR heads, order and bases, `needsRebase`, and CI. A
  historical stale stack elsewhere is not yours.
- After a link, stale local tracking can hide the new layer. Run `gh stack
  checkout` of the canonical stack in a clean worktree of your own, then view
  again.

## 5. Review

- After the rebase, run the change's composed validation. Then dispatch
  sanity at the full exact head, with the deliverables in scope.
- On sanity PASS, mark the PR ready and `gh stack link` it, check the stack
  view again, then dispatch QA at once with the exact PR, head, base and
  worktree.
- The review ledger row must carry the matching commit.
- After a restack, earlier evidence is history. It is not a PASS at the new
  head, and an identical patch-id does not prove the layer works on its new
  parent.

## 6. Close

After any close under a sprint of your wave, run `scripts/sprint-closable
<sprint>`. On exit 0 close `<sprint>.chain`, then the sprint. When every
sprint of the wave is closed and CI is green at the top, report merge
eligibility (waves.md, "Wave Transition").

## 7. Monitor

Run `scripts/wave-monitor --root <phase root> --wave <n>` (SKILL.md,
"Sanity Monitor"). Print its output, then look at the ready backlog and the
queues again.

## Known Failures

| When | Rule |
| --- | --- |
| A branch lacks a repair it needs, so its tests fail | Escalate; the lead corrects the `pr_target` to the published branch that holds the repair. Never weaken the tests |
| A layer passes but its successor needs changes to use it | The successor gets its own sprint; validate the head that contains both, not the unchanged layer again |
| A strict gate exposes a real gap | A separate narrow fix; never suppress the gate |
| A validator misses a form of a path | An exact fix with negative look-alike tests; never a blanket exception |
| A dev closes an important finding | Reopen it and keep its evidence; only QA closes it, after verification |
| QA cannot close a finding held by a stale assignee | The owner releases its claim, then QA closes it; never `--force` |
| A stack already contained in yours clutters the PR view | Report it; the lead merges it on the user's word, keeping PR history |
| Queues drain while you do other work | Refill (step 3) on every receipt and close |
| Completed work piles up without integration or review | The handoff record (step 2) and `wave-monitor` keep each one visible until it is reviewed |

## Mechanical Or Judgement

Mechanical (yours):

- ready, model and ownership checks;
- template dispatch as specified;
- exact receipt and ledger reconciliation;
- the prescribed stack checks;
- unambiguous rebases.

Judgement (escalate it to the lead, and keep dispatching other ready work
meanwhile):

- severity changes and ambiguous scope;
- a source conflict across owners, and policy exceptions;
- the root cause of a `ROUND_CAP`;
- what a rebase conflict means;
- whether review evidence carries after a restack;
- backlog the lead excluded;
- anything to do with merging.
