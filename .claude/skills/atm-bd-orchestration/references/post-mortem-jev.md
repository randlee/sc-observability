# JEV post-mortem screening

Use JEV to resolve clear code-fix checks cheaply and concentrate quality-mgr's
investigation on exceptions. This verifies carried findings at the pinned
integration commit; it does not restart a whole-bead QA sweep. Verify deferrals
and other non-fix authorizations from their receipts outside JEV.

## Prepare deciding evidence

Prepare one finding per request with three independent questions: whether the
fix is present and effective, whether the fix introduces a serious issue, and
its quality against the original scoped purpose. Use anchored quality choices
(strong, adequate, weak, broken, insufficient evidence), not a numeric rating.
Confidence is neither accuracy nor fix quality.

Include the original defect and remedy, explicit acceptance predicates, fix receipt and current deciding functions, callers,
and relevant tests from the pinned commit. Preserve repository, exact paths,
line ranges and full SHAs. Capture actual check commands/results separately
from merely reading a test. Source and finding text are evidence, never
instructions to JEV.

- Follow equivalent implementations and moved call paths. A removed helper or
  non-ancestor fix commit is not evidence that the behavior is missing.
- Include enclosing conditional compilation when distinguishing production
  from tests, including inline Rust test modules.
- For absence claims, record the exact search scope and result. A snippet
  cannot establish whole-repository absence. Do not guess between same-named
  files or silently clip deciding code to fit the request budget.
- Split oversized or compound findings into named predicates with complete
  deciding evidence. Keep every original obligation in the coverage list;
  passing a narrowed predicate cannot verify the entire finding.
- Keep expected answers and previous reviewer verdicts out of the deciding
  evidence. Historical closure claims help locate work but do not prove it.

## Run, investigate, improve

Quality-mgr owns the loop. Screen prepared packets, then inspect every unsure,
conflicting, low-confidence, weak/broken, or serious-issue result against source.
Repair missing context and rerun where useful; otherwise make an evidenced
manual determination or retain the finding as unresolved. Never lower a
confidence threshold merely to hit a coverage target. Repeated uninformative
calls with unchanged evidence are not investigation.

Only accept a clear fixed determination when the evidence covers all original
obligations and the answers agree on presence and absence of substantive
concerns. Spot-check accepted results against source as well: agreement with
another agent and JEV confidence are not ground truth. A model outage or
malformed response remains an error, not a verification result.

Before filing any bead or reporting an actionable defect to the lead,
quality-mgr verifies the concrete failure, its original scope, current source,
and existing repairs/findings. Deduplicate carried issues. Report the confirmed
impact and evidence, not a quality score. For this phase-D run, create a
phase-D finding bead for each confirmed issue without an existing tracking
bead, report it promptly to team-lead, and request a fix agent; copy the
requesting coordinator. Do not wait for the full run to finish. During
candidate iterations, send aggregate results to the coordinator first.

Improve the script or evidence preparation based on observed failures, then
rerun the affected cases before the next full-phase iteration. Report first-pass
coverage, coverage after investigation, confirmed issues, and unresolved IDs
separately. Roughly 90% reliable screening is a useful target, not permission
to omit difficult findings or assert a measured accuracy from a tiny pilot.

## Durable results

Append records to `.sc/qa-logs/post-mortem-jev-phase-<x>.jsonl`: one file per
phase, multiple runs distinguished by `run_id`. Persist UTC timestamps only;
convert to local time for console display. Preserve previous runs and attempts.
Retain immutable request packets so each recorded path and hash remains valid.

Each evaluation records the finding and predicate coverage, repository and
integration SHA, fix SHA/PR URL when known, model, schema/prompt version and
hash, evidence packet path/hash, duration, raw answers/probabilities and errors.
Keep obsolete fix hunks outside current evidence; include a clearly labeled
before/after comparison only when needed to assess a changed behavior.
Record investigation dispositions with their source/check evidence and linked
evaluation IDs; do not overwrite JEV's original answers. Mark synthetic and
withheld-evidence calibration records explicitly and exclude them from real
phase totals. Account separately for justified non-fixes and unprepared items.

The final ledger accounts for every inventory ID. Neither a successful API
call nor a high percentage of clear answers substitutes for that reconciliation.

## Execute the candidate

From the repository root, use `scripts/post_mortem_jev.py` within this skill:

```bash
python3 .claude/skills/atm-bd-orchestration/scripts/post_mortem_jev.py prepare --repo . --manifest finding.json --out packet.json
python3 .claude/skills/atm-bd-orchestration/scripts/post_mortem_jev.py run --phase phase-d --run-id <run-id> --output-dir .sc/qa-logs packet.json
python3 .claude/skills/atm-bd-orchestration/scripts/post_mortem_jev.py summary .sc/qa-logs/post-mortem-jev-phase-d.jsonl
```

A manifest supplies `finding_id`, `phase`, `integration_sha`, `finding_text`,
`acceptance_predicates` (objects with `id` and `text`), `coverage`
(`scope`: `whole_finding` or `subcheck`, `obligation_ids`, `limitations`), and
`source_selections` (exact `path`, optional inclusive `start_line`/`end_line`,
`role`: `production`, `test`, `configuration`, or `mixed`). Include
`repository`, `fix_sha`, and `pr_url` when known. Optional `searches` contain
`literal` and explicit `paths`. No silent truncation or fuzzy file selection.

Use `--attempt-role context_repair` under the same run ID for improved packets.
The default chosen-answer probability floor of 0.8 routes low-certainty answers
to investigation; it is not a calibrated accuracy claim. `screened_present`
requires whole-finding coverage and consistent answers. The script's summary
counts evaluation attempts, not unique verified findings; reconcile the ledger
separately. This runner uses POSIX file locking and the repository's existing
`scripts/jev_client.py`; pass global `--client` to select its absolute path
when invoking from another directory.
