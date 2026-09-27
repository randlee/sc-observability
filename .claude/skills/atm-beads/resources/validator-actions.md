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
| `STATE.OPEN_FINDINGS` | `action[lead]`: reopen the parent or close every open sanity finding as not valid |
| `STATE.REOPENED_PASS` | `action[user]`: record a `policy.waivers` entry after ruling |
| `STATE.BLOCKER_ORDER` | `action[lead]`: stop work and restore prerequisite closure before redispatch |
| `QA.MISSING` | `action[lead]`: create and dispatch the required QA bead |
| `QA.ROUND_CAP` | `action[lead]`: stop dispatch and determine root cause before another round |
| `PR.TARGET` | `action[assignee]`: retarget the PR to `metadata.pr_target` and rerun validation |
| `FINDING.PRIORITY` | `action[lead]`: set finding priority from its severity policy |
| `FINDING.SEVERITY` | `action[planner]`: set a valid finding severity label |
| `SANITY.BASE` | `action[assignee]`: set `metadata.base` to the branch name, not a SHA |
| `SANITY.COMMIT` | `action[assignee]`: set `metadata.commit` to the reviewed 40-hex SHA |
