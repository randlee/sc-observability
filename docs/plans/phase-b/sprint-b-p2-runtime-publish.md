---
id: B.P2
status: proposed
branch: release/phase-b-p2-runtime-core
base: develop
---

# B.P2 — Qualify the additive runtime-level core capability

## Goal and dependencies

Owner: sc-observability release owner. `must_follow` B.P1's merged, accepted
implementation; B.P3 `must_follow` this sprint's exact staged artifacts, not a
registry release. Apply parent-to-child merge-forward on pushed development
before every child round and merge the parent PR first. Shared artifacts/version
metadata prevent parallel-safe work. B.1 remains the first migration sprint; no
bridge is imported or published here. Live crates.io publication and
registry-only proof are reserved for B.7 at phase end.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Select the next minor workspace release after the current published version,
   retaining the existing four-crate release train and dependency order. Package
   the B.P1 API with changelog, consumer documentation and approved additive
   API/semver evidence. Create immutable candidate packages and record their
   normalized manifests, source commit, versions and checksums; do not publish.
2. Add `scripts/ci/validate_runtime_level_staged_consumer.py --version V`:
   create fresh isolated baseline and candidate projects using exact staged
   packages with no ambient workspace resolution. Construct Logger plus
   LevelOwner, change/reset its threshold, log/query records and shut it down.
   Verify records and coherent level state, including a stale-owner error after
   logger shutdown, on macOS/Linux/Windows.
3. Record the staged artifact inventory, two-leg consumer output, source commit,
   candidate versions/checksums, contract revision and scoped approvals in
   `docs/plans/phase-b/handoff-b-p2.md` for BTIT consumption. B.7 later performs
   the live publication, index wait, and registry-only consumer proof.

## Consumer boundary

```toml
[dependencies]
sc-observability = "=V"
sc-observability-types = "=V"
```

V is replaced by the selected release version, never left as a placeholder in
execution evidence. The consumer exercises B.P1's public signatures directly;
it contains no copied runtime implementation or hidden-module references.

## Acceptance criteria (authoritative)

- AC1: Each selected core package has immutable staged bytes, a checked manifest,
  and recorded source/version/checksum provenance; publication remains pending.
- AC2: The two-leg isolated consumer proves actual runtime behavior on the
  promised platforms and the existing published consumer compatibility check
  stays green.
- AC3: The handoff identifies the exact staged capability version BTIT must
  resolve. A merged PR, local package, or attempted publish alone cannot close
  B.P2; the required package and consumer evidence must be recorded.

## Required validation (authoritative)

Run the existing release preflight, workspace tests, API diff/semver/docs checks,
publication-order and version-literal checks, and package-content verification.
Run the staged two-leg consumer:

```sh
python3 scripts/ci/validate_runtime_level_staged_consumer.py --version "$RELEASE_VERSION"
```

The new script is implemented in this sprint and rejects version placeholders,
ambient workspace resolution, missing platform results and skipped assertions.
Record actual qualification commands and checksums in the handoff. B.7 must
later run a separate registry-only consumer after real publication.

## Paths to delete

None.

## Non-closure

No live publication, BTIT dependency edits, binding implementation or #92 API
migration. Release approval/credential blocks are phase-end B.7 concerns, not
permission to publish early.
