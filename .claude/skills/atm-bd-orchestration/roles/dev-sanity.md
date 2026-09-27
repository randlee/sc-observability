# Role: dev-sanity (atm-bd-orchestration)

The dev-sanity role runs the sanity check of every closed dev or fix bead in
a phase run with atm-bd-orchestration. It is long-running; this role applies
to every task it receives until the lead switches it back.

A sanity check asks one question of a closed dev or fix bead: is each numbered
deliverable written? It is not QA: requirements and quality belong to QA. Lint
is a separate mechanical gate. The checker receives only deliverable text,
owned paths, changed files, and a pinned commit; that evidence must let a
luna-class agent answer `written: yes/no, file:line` correctly.

## Who Fills It

The skill names the role, never a member or an agent. The repository
decides both:

| Setting | Where | sc-observability |
| --- | --- | --- |
| member | `roles.dev-sanity` in `.claude/agents/registry.yaml`; print it with `.claude/skills/atm-beads/scripts/resolve-role dev-sanity` | `obs-sanity` |
| directive | that member's `[startup.<member>]` prompt in `.atm.toml` | `.claude/agents/dev-sanity-llm.md` |

The member name is unique to the team, because Herdr agent names are global
on the host. It is never a dev or fix agent, which would make sanity checks
wait behind their work. Switching how checks run (an LLM check, a jev check)
changes the directive in `.atm.toml`, not this skill.

## Tasks

Every task is a sanity check bead rendered from `dev-sanity-template.xml.j2`,
and the task id is the bead id. The template carries the task values and the
bead and task lifecycle; the member's agent prompt (its directive) says how
the check runs.

Sanity checks gate dependent dev work, so speed matters: the member starts
every open sanity check task at once, each with its own team of check
subagents, and closes them in whatever order their verdicts arrive. Nothing
waits on another check.

## Check Contract

## Pre-claim refusals

Before claim, run the shared admission gate for **every request**, once per
sanity coordinator (both `dev-sanity-llm` and `dev-sanity-jev`), before children
or lint. Locate the canonical `gh_stack_view.py` using `/sc-gh-stack-view` and
pass its absolute path; the gate invokes it read-only with `--json`, from the
assigned worktree. Keep the report outside that worktree.

```bash
python3 .claude/skills/atm-bd-orchestration/scripts/assignment-gates.py sanity \
  --bead "$TASK_ID" --checked-bead "$CHECKED_BEAD" --worktree "$WORKTREE" \
  --branch "$BRANCH" --commit "$COMMIT" --pr-number "$PR_NUMBER" \
  --pr-target "$PR_TARGET" --stack-view "$STACK_VIEW_SCRIPT" \
  --stack-report "$SCRATCH/$TASK_ID-stack-view.txt"
```

Require a PR number and URL (`SANITY.PR_REQUIRED`). Only `READY` from the gate
permits claiming. The gate verifies:

1. Actual open PR base equals the checked bead's declared `pr_target`, and
   its branch/head equal the assigned branch/pinned commit.
2. A dependent PR belongs to exactly one coherent `/sc-gh-stack` with known
   local/origin/PR heads and base coherence for every open layer. The bottom
   layer may be behind a moving trunk if the canonical view permits it.
   **Only actual PR bases `develop` or `integrate/phase-*` exempt membership**;
   unrelated stacks do not gate a direct PR. Missing/incoherent/unknown stack
   evidence refuses with `SANITY.STACK_REQUIRED`, `SANITY.STACK_INCOHERENT`, or
   `SANITY.STACK_UNVERIFIED`. Unavailable tools/invalid output cannot pass.
3. Worktree branch/HEAD and freshly fetched origin head equal the assignment
   (`SANITY.HEAD_MISMATCH`), with nonzero delta against the PR target
   (`SANITY.ZERO_DELTA`) and a clean tree including untracked files
   (`SANITY.DIRTY_TREE`; ignore only existing gate/compose scratch paths).
4. Task history contains no earlier PASS (`SANITY_FROZEN`).

These checks apply to direct PRs too; their exception is membership only.
The gate's non-READY code is a refusal, never a best-effort check. Prefix its
code with `SANITY.` for the refusal, except the existing `SANITY_FROZEN` code.
Neither red lower-layer CI nor a missing overall landing approval is a sanity
quality finding. Children retain their narrow done/not-done contract.

Sanity never edits, amends, rebases, or force-pushes frozen layers and never
runs stack mutations. The lead's designated stack writer owns repairs; later
fixes append above the current top. Read-only canonical stack view is allowed
and required, including when an earlier prompt prohibited all `gh stack` use.

For every refusal, before the refusal message or task close, strictly render
`templates/workflow-issue-bead.json.j2` with id `$TASK_ID-wf-$CODE`, import it
with `bd import <scratch>/$TASK_ID-wf-$CODE.json`, and name that workflow issue
id in the refusal.

One check is one closed bead at one pinned commit, split per deliverable:

- `scripts/sanity-split` reads the bead, parses the numbered list under
  `## Deliverables`, pins the commit, lists the changed files against the
  bead's `owned_paths`, starts the lint command in the background, and
  renders one assignment per deliverable from
  `templates/dev-sanity-assignment.json.j2`.
- The directive sends each assignment to its own check subagent as fenced
  JSON, all at once, and reads one fenced JSON result per deliverable back.
  The subagent owns that contract, in its `## Inputs` and `## Output Format`:
  [`.claude/agents/sc-sanity-llm.md`](../../../agents/sc-sanity-llm.md).
  Every check subagent (`sc-sanity-jev.md` too) keeps the same assignment
  and result, so a repository switches checks by switching the directive in
  `.atm.toml`.
- `scripts/sanity-merge` accepts exactly one result per deliverable at the
  pinned SHA, checks that the worktree is still at that SHA and clean,
  folds in the lint exit code and diagnostics, and writes the verdict and
  the report vars.

The check leaves nothing in the repository: `sanity-split` writes only the
lint log and the lint exit file under `--scratch`, the renderer's transient
input file is deleted once each assignment is rendered, and sc-compose keeps
its own log under `.sc-compose/`, which is gitignored.

There is no fallback. A bead whose `## Deliverables` is not a numbered list
cannot be split; the check is refused with `SANITY.PLAN_INVALID` and the
lead is told that planning failed for that bead. Every deliverable appears
in the report by number, done or with its findings, so closure is explicit.

## Verdicts

| Verdict | Sanity check bead | Task |
| --- | --- | --- |
| PASS | `bd close` with reason `PASS at <commit>` | `completed`, `dev-sanity-complete.md.j2` |
| FAIL | stays open: `bd update --status open --append-notes` | `completed`, `dev-sanity-complete.md.j2` with the findings |
| cannot run | stays open, with a note | `refused`, `task-refused.md.j2` |

A FAIL never closes the bead. Closing it would release the dev beads that
depend on the checked sprint. The sanity member creates one child finding bead of
the checked bead per undone deliverable, never one per lint diagnostic. The parent/child
hierarchy is the closure gate; a parent-to-child
`blocks` edge is invalid. Each child is blocking at `clamp(parent priority - 1, P1, P4)`, records
the same structured JSON finding data as the sanity report, and copies the
checked bead's phase/sprint/stack/layer provenance. The lead reviews those
children and may overrule or modify them, but does not recreate their report
data. The lead then follows its existing process to reopen the parent and
assign the dev fix. It adds `blocks` edges only between those new beads, where one fix depends on another. The parent cannot close until all children close. That closure makes the
same sanity check bead ready again.

After the second FAIL for the same checked bead, the sanity member reports
`SANITY.ROUND_CAP` to the lead with the undone deliverable numbers. No third
round is dispatched without the lead's ruling.
