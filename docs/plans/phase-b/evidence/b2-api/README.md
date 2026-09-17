# B.2 API review packet

Candidate API source: `6bb5cc126c728bdf10abd449eb292c87b864005b`.
The `public-api-diff.json` report pins that source; raw per-crate diff logs are
retained here. The four core crates compare to published `1.2.0`; the two
companion crates are verified absent from crates.io and export their initial
public baseline. Diff exit 1 means actionable changes, not an execution failure.

Semver logs came from the same production API at `f8c8233` (the later `6bb5cc1`
changes only the external consumer fixture and historical staging helper).
All four existing crates pass the minor-release semver gate. The two first
release crates have no registry baseline; their public API generation succeeded.
No semver failure or tool execution failure is waived by any approval record.

`docs/api-approvals/phase-b-1.4.0.json` is a **pending** per-crate review record.
Each named crate requires reviewer identity, evidence and explicit approved
status before the docs gate passes. This packet grants no publication approval
and does not resolve the owner's separately deferred runtime-level acceptance.
