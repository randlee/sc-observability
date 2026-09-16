---
id: B.P2
status: proposed
branch: release/phase-b-p2-runtime-core
base: develop
---

# B.P2 — Publish the additive runtime-level core capability

## Goal and dependencies

Owner: sc-observability release owner. `must_follow` B.P1's merged, accepted
implementation; B.P3 `must_follow` this registry release. Apply parent-to-child
merge-forward on pushed development before every child round and merge the
parent PR first. Shared artifacts/version metadata prevent parallel-safe work.
B.1 remains the first migration sprint; no bridge is imported or published here.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Select the next minor workspace release after the current published version,
   retaining the existing four-crate release train and dependency order. Package
   the B.P1 API with changelog, consumer documentation and approved additive
   API/semver evidence. Follow the existing main-based release process.
2. Add `scripts/ci/validate_runtime_level_registry_consumer.py --version V`:
   create a fresh temporary project with exact registry-only dependencies, no
   path/git/patch overrides, construct Logger plus LevelOwner, change/reset its
   threshold, log/query records and shut it down. Verify records and coherent
   level state, including a stale-owner error after logger shutdown.
3. Publish the approved immutable artifacts, wait for index availability and
   run the consumer on macOS/Linux/Windows. Record source commit/tag, crate
   versions/checksums, contract revision, scoped approvals and actual command
   evidence in `docs/plans/phase-b/handoff-b-p2.md` for BTIT consumption.

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

- AC1: Each selected core package is retrievable and matches tested bytes and
  immutable release source; dependent publication waits for registry visibility.
- AC2: The external consumer proves actual runtime behavior on the promised
  platforms and the existing published consumer compatibility check stays green.
- AC3: The handoff identifies the minimum released capability version BTIT must
  resolve; a merged PR, local package or attempted publish cannot close B.P2.

## Required validation (authoritative)

Run the existing release preflight, workspace tests, API diff/semver/docs checks,
publication-order and version-literal checks, and package-content verification.
After registry availability, run:

```sh
python3 scripts/ci/validate_runtime_level_registry_consumer.py --version "$RELEASE_VERSION"
```

The new script is implemented in this sprint and rejects version placeholders,
local dependency overrides, missing platform results and skipped assertions.
Record actual release-process commands and checksums in the handoff.

## Paths to delete

None.

## Non-closure

No bridge release, BTIT dependency edits, binding implementation or #92 API
migration. Release approval/credential blocks leave this sprint pending.
