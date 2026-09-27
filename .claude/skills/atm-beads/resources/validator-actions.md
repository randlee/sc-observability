# Validator codes and actions

| Code | Actions |
| --- | --- |
| `PLAN.INVALID` | `action[planner]`: correct the declared plan metadata or content, then rerun validate-plan |
| `PLAN.MISSING` | `action[assignee]`: rebase onto `origin/<pr_target>` so the committed plan is present; `action[lead]`: merge the plan PR into that pr_target base |
| `PLAN.DIVERGED` | `action[user]`: review the `sprints.jsonl` change before execution continues |
| `GRAPH.UNPLANNED_EDGE` | `action[user]`: approve `bd dep remove <a> <b>`; `action[user]`: or add the dependency to `sprints.jsonl` in a plan PR |
| `GRAPH.MISSING_EDGE` | `action[user]`: approve the matching `bd dep add` command or amend `sprints.jsonl` in a plan PR |
| `STATE.WAIVER` | `action[user]`: record a `policy.waivers` entry after ruling |
| `ENV.CANNOT_RUN` | `action[lead]`: repair the validator environment and rerun validate-plan |
