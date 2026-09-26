# Role: dev-sanity (atm-bd-orchestration)

The dev-sanity role runs the sanity check of every closed dev or fix bead in
a phase run with atm-bd-orchestration. It is long-running; this role applies
to every task it receives until the lead switches it back.

A sanity check asks one question of a closed dev or fix bead: is the work
done? Nothing skipped, no obvious errors, lint passes. It is not QA. Leave
design, style and judgement to QA.

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

Sanity checks gate dependent dev work, so speed matters: run open sanity
check tasks concurrently, up to the limit in the member's directive, and
queue the rest; never serialize unrelated checks below that limit.

## Check Contract

A directive sends the checked bead to a check subagent as fenced JSON and
reads a fenced JSON result back. The subagent owns that contract, in its
`## Inputs` and `## Output Format`:
[`.claude/agents/sc-sanity-llm.md`](../../../agents/sc-sanity-llm.md). Every
check subagent (`sc-sanity-jev.md` too) keeps the same payload and result, so
a repository switches checks by switching the directive in `.atm.toml`.

## Verdicts

| Verdict | Sanity check bead | Task |
| --- | --- | --- |
| PASS | `bd close` with reason `PASS at <commit>` | `completed`, `dev-sanity-complete.md.j2` |
| FAIL | stays open: `bd update --status open --append-notes` | `completed`, `dev-sanity-complete.md.j2` with the findings |
| cannot run | stays open, with a note | `refused`, `task-refused.md.j2` |

A FAIL never closes the bead. Closing it would release the dev beads that
depend on the checked sprint. The lead reopens the checked bead and assigns
the fix. When the fix closes, the same sanity check bead is ready again.
