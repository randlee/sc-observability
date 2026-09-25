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
and the task id is the bead id. Follow its steps in order.

Sanity checks gate dependent dev work, so speed matters: run every open
sanity check task at once and never serialize unrelated checks on purpose.

The template writes one payload file per check. Your directive says how the
check runs on it; whatever runs it never runs `bd` or `atm`. You check its
result, then close the task and the bead yourself.

## Payload

```json
{
  "sanity_bead": "obs-d-4-sanity",
  "dev_bead": {"id": "obs-d-4", "title": "...", "description": "...", "design": "...",
               "acceptance_criteria": "...", "metadata": {}},
  "worktree_path": "/absolute/path/to/worktree",
  "branch": "sprint/d-4-slug",
  "commit": "0123abcd",
  "base": "integrate/phase-d",
  "lint_command": "just lint"
}
```

## Result

Every directive returns this envelope:

```json
{
  "success": true,
  "data": {
    "sanity_bead": "obs-d-4-sanity",
    "dev_bead": "obs-d-4",
    "commit_checked": "0123abcd",
    "verdict": "PASS | FAIL",
    "findings": [{"kind": "skipped | error | lint", "file": "...", "line": 42, "issue": "..."}],
    "lint": {"command": "just lint", "exit_code": 0, "summary": "..."}
  },
  "error": null
}
```

`success: false` carries `error` = `{code, message, recoverable,
suggested_action}` and means the check did not run.

## Verdicts

| Verdict | Sanity check bead | Task |
| --- | --- | --- |
| PASS | `bd close` with reason `PASS at <commit>` | `completed`, `dev-sanity-complete.md.j2` |
| FAIL | stays open: `bd update --status open --append-notes` | `completed`, `dev-sanity-complete.md.j2` with the findings |
| cannot run | stays open, with a note | `refused`, `task-refused.md.j2` |

A FAIL never closes the bead. Closing it would release the dev beads that
depend on the checked sprint. The lead reopens the checked bead and assigns
the fix. When the fix closes, the same sanity check bead is ready again.
