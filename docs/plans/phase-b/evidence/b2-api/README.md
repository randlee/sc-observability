# B.2 API review packet

Current qualification API source: `67e715d5be90aaf60f02289bfc3dbb4255ccd94b`.
The report pins that merged source; raw per-crate stdout/stderr and exact stdout
SHA-256 digests are retained. All six digests equal the independently reviewed
API at `6bb5cc126c728bdf10abd449eb292c87b864005b` despite the later observation
implementation corrections. The lead confirmed this equivalence.

The four core crates compare to published `1.2.0`; each minor semver check passed.
The two companions are verified absent from crates.io; public API generation
succeeded and establishes their initial surface. Diff exit 1 is an actionable
API change report, not an execution failure. Tool failures return 2 and cannot
be waived; semver failures cannot be waived by an approval.

`docs/api-approvals/phase-b-1.4.0.json` contains the actual aobs lead approval,
dated 2026-09-17T09:06:54.905317+00:00, with the original reviewed source and six
API digests unchanged. Its scope is API surface only: it does not resolve any
behavior finding, replace independent QA, grant publication, or resolve the
owner's separately deferred runtime-contract acceptance.
