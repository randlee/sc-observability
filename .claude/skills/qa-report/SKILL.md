---
name: qa-report
description: Show compact current-phase QA metrics tables from .sc/qa-log/, with the 10 most recent rows by default or all rows with --all.
---

# QA Report

Displays the two QA metrics logs quality-mgr appends to on every QA-bead
close. Read-only; writes nothing.

## Arguments

- `/qa-report`: show at most the 10 most recent rows from each log for the
  current phase.
- `/qa-report --all`: show every row from both logs for the current phase.
  This changes only the row limit, not phase selection or compact formatting.

## Execution

1. Select the phase explicitly requested by the user, otherwise the current
   phase from conversation context. If neither is known, use the phase with
   the latest recorded `snapshot_at` or `completed_at` timestamp. Do not
   combine unrelated phases into the report.
2. Locate that phase's logs under `.sc/qa-log/`: the per-round event log
   (`phase-<p>.jsonl`, "Log B") and its cumulative stats sibling
   (`phase-<p>-stats.jsonl`, "Log A"). If neither exists, say so and stop.
   Label a missing or empty log as such; still show its available sibling.
3. Sort Log A by `snapshot_at` descending and Log B by `completed_at`
   descending, comparing timestamps as instants. Then take the first 10
   rows from each, unless `--all` was supplied. Apply the limit before any
   date grouping; do not take 10 per date or pre-limit the unsorted files.
   Preserve the recorded cumulative totals; never recompute them from the
   displayed subset.
4. Render the selected rows using the compact format below, newest first.

## Compact tables

- **Log A — cumulative phase-stats**: `Time`, `Tot`, `Open`, `B/I/M`, `Trigger`.
  Map these to `snapshot_local`, `tot`, `open`, the three severity counts,
  and `trigger_task`.
- **Log B — per-round events**: `Time`, `Task`, `Sprint`, `Tested`, `Iter`,
  `Verdict`, `B/I/M`, `Fnd`, `Dur`, `PR`. These retain the existing event
  fields; `B/I/M` means blocking/important/minor, e.g. `0/1/2`.
- Show local dates as table subheadings and local times as `HH:MM`, using
  `snapshot_local` / `completed_local`. Keep the timezone in the report
  label. Show commit IDs as eight hexadecimal characters and durations
  compactly (e.g. `7m30s`).
- Keep full task IDs internally, but never print long ancestry chains in
  `Task` or `Trigger`. Use the final QA/sanity PR component and any following
  finding suffix: `...-qa-pr742` becomes `QA742`,
  `...-qa-pr742-f1` becomes `QA742-f1`, and `...-sanity-pr742` becomes
  `sanity742`. The separate `Sprint` column retains `d-23`, for example.
  For other IDs, use a recognizable abbreviated label of at most 24
  characters, with an ellipsis when shortened. Disambiguate colliding
  labels within the report with short numeric suffixes inside that limit.
- Keep Markdown table lines within 120 characters: use short headers,
  minimal padding and shorter display labels as needed. Do not silently
  drop selected rows or numeric values to fit. Do not append a full-ID
  legend or expand IDs under `--all`; provide full IDs only if requested.

## Output

Both tables, labeled Log A and Log B with the selected phase and
`shown/total` row counts. Include `--all for all rows` when rows are hidden.
Do not print additional log dumps or historical phase tables. Write the
tables as markdown directly in your reply text; a command's own stdout is
not reliably shown to the user, so never rely on tool output alone.
