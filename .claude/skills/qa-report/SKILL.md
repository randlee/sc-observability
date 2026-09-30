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
     `blk`, `imp`, `min`, `fnd`, `duration`, `pr_number`, `replaced_lines`.
3. Resolve Log B per (`task`, `iteration`) in file order before rendering
   it; never drop a raw row.
   - A row with `correction: true` replaces every earlier row of its round.
     Its `supersedes_verdict` and `supersedes_counts`, when present, must
     equal the preceding row's verdict and `blk`/`imp`/`min`; otherwise the
     round is `conflict`.
   - Show a resolved round once with its latest row's values. Its verdict
     reads `<latest> (corrected from <first>)` when the verdict changed, else
     `<verdict> (corrected)`. `replaced_lines` lists the file line numbers of
     every replaced row across the chain.
   - A round repeated with no later `correction: true` row is
     `legacy-correction-needs-resolution` when a repeated row carries
     `superseded`, `superseded_reason`, `corrects` or `correction_reason`,
     else `conflict`. Show all of its rows under that label.
   - Totals and current verdicts count resolved rounds only. Report the
     unresolved rounds and their raw `fnd` beside the totals.

## Output

Both tables, labeled Log A and Log B, per phase. Nothing else. Write the
tables as markdown directly in your reply text; a command's own stdout is
not reliably shown to the user, so never rely on tool output alone.
