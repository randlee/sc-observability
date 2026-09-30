---
name: qa-report
description: Show the two QA metrics logs (per-round event log and cumulative phase-stats) from .sc/qa-log/.
---

# QA Report

Displays the two QA metrics logs quality-mgr appends to on every QA-bead
close. Read-only; writes nothing.

## Execution

1. Locate the logs under `.sc/qa-log/`: the per-round event log
   (`phase-<p>.jsonl`, "Log B") and its cumulative stats sibling
   (`phase-<p>-stats.jsonl`, "Log A"), one pair per phase found. If neither
   exists, say so and stop.
2. For each phase, render two tables, newest row first, every row included
   (no truncation, no summarizing). Sort Log A by `snapshot_at` descending
   and Log B by `completed_at` descending; display the local-time columns:
   - **Log A — cumulative phase-stats** (`phase-<p>-stats.jsonl`): columns
     `snapshot_local`, `tot`, `open`, `blk`, `imp`, `min`, `trigger_task`.
   - **Log B — per-round events** (`phase-<p>.jsonl`): columns
     `completed_local`, `task`, `sprint`, `tested`, `iteration`, `verdict`,
     `blk`, `imp`, `min`, `fnd`, `duration`, `pr_number`.

## Output

Both tables, labeled Log A and Log B, per phase. Nothing else. Write the
tables as markdown directly in your reply text; a command's own stdout is
not reliably shown to the user, so never rely on tool output alone.
