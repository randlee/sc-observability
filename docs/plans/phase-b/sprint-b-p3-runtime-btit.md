---
id: B.P3
status: proposed
owner: BTIT team
repository: beads-task-issue-tracker
---

# B.P3 — Accept BTIT runtime-level bridge integration before copy

## Goal and dependencies

Owner: BTIT team implements in its repository; sc-observability reviews the
public contract and accepts handoff. `must_follow` B.P2's exact staged core
artifacts, the target-bridge contract that B.P3 must accept before source
handoff, and the owner-deferred runtime contract. B.1 `must_follow` this
sprint's accepted immutable source. No parallel-safe relationship spans these
shared contracts. Within BTIT, merge pushed parent development before each child
round and merge parent PR first; cross-repository dependency uses B.P2's
recorded staged package version/checksum and immutable accepted commits rather
than an impossible Git merge.

The existing released `1.2.0` core remains the published compatibility baseline;
the additive runtime-level capability is B.P2's distinct staged `1.3.0`
candidate, not a pre-B.P3 release. B.7 alone turns qualified staged artifacts
into live published artifacts after the phase-end gates.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. BTIT resolves B.P2's staged core capability and implements LogGuard owner
   operations against its single LevelOwner. Use the shared core filter for
   direct/facade/macro producers; keep LogControl read-only. Remove independent
   threshold policy, retain a conservative fixed Trace runtime facade ceiling,
   and reject requests beyond resolved log::STATIC_MAX_LEVEL before mutation.
   Reject an unsupported configured baseline before installing the global logger
   with typed InitError::UnsupportedLevel.
2. BTIT proves its supported debug/release feature graph retains desired
   Debug/Trace callsites and tests a deliberately capped build returning
   UnsupportedLevel. Populate bridge health with configured/effective levels
   and revision, preserving one coherent core snapshot and nonfatal diagnostics.
3. BTIT completes initial design, implementation, target-API inventory and
   critical review/re-review. sc-observability records the accepted target-contract
   commit, the owner-deferred runtime-contract reference, B.P2's exact staged
   core package version/checksum, accepted source SHA, exact removed threshold
   symbols, feature graphs and review/test evidence in
   `docs/plans/phase-b/handoff-b-p3.md`. Source fixes stay in BTIT before copy.

## Bridge boundaries

Use [the target bridge API](target-bridge-api.md) and the complete
[runtime contract](runtime-level-contract.md), including:

```rust
impl LogGuard {
    pub fn elevate_level(&mut self, level: LevelFilter, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
    pub fn reset_level(&mut self, source: LevelChangeSource)
        -> Result<LevelChange, LevelChangeError>;
}
```

LogControl gains no mutation authority. Only the installed bridge sets global
facade state; unrelated independent core loggers neither read nor mutate it.
External application writes to log::set_max_level after installation are outside
the bridge consistency contract and must be documented as unsupported usage.

## Acceptance criteria (authoritative)

- AC1: Core, direct bridge, macros and facade use one effective state; synchronized
  post-return/change/shutdown tests show no extra transition-induced drops.
  Queue saturation retains explicit existing failure/drop accounting.
- AC2: Actual release executable retains Debug/Trace statements; capped-build
  failures leave state unchanged; an unsupported baseline fails before any
  global installation and cannot produce misleading effective-level health. Health and change diagnostics agree with
  committed revisions; diagnostic failure never rolls back a successful change.
- AC3: Read-only handles cannot mutate levels or retain shutdown ownership; all
  target exports match the accepted inventory, and critical findings are closed
  by accepted re-review of the exact handoff SHA.

## Required validation (authoritative)

BTIT runs its complete bridge/macro/API/UI consumer checks, core-sharing
integration tests, deterministic races and fault injection, plus release-mode
logging and capped-build tests. Record resolved Cargo feature graphs and exact
commands/results rather than assuming debug tests prove release behavior.
sc-observability checks the handoff against the accepted target design, the
owner-deferred runtime contract reference, and B.P2's exact staged core
package version/checksum, not a registry version. B.1's independent
provenance verification is still required. B.P3's staged consumption is not
live registry proof; B.7 owns that proof at phase end.

Before B.1 copy, B.P3 must retain an immutable source-repository inspection:
the exact BTIT commit, complete root/export and hidden-macro-support inventory,
target disposition for every observed export, source-side commands/results, and
the BTIT critical review/re-review path and commit. This repository's workspace
CI cannot prove BTIT workspace tests, exports, or boundary rules; B.P3 records
BTIT-owned evidence rather than claiming that coverage. The final inventory and
source-review verdict remain a B.P3/B.1 gate, not a pre-implementation completion
claim. B.P3 compile-fail coverage proves LogControl cannot acquire mutation
authority. B.3a/B.4 add binding host-routing proof once those bindings exist.

## Paths to delete

BTIT removes its independent THRESHOLD policy and threshold setter helpers after
replacing every callsite with shared-state filtering. Record exact symbols/files
at the accepted source SHA in the handoff; no published sc-observability API is
removed and no BTIT files are deleted by sc-observability during this sprint.

## Non-closure

No destination copy/live publication, post-copy redesign, Tauri/Python adapter
work, BTIT switch to a future published companion crate, or claim that contract
approval alone closes source implementation/review.
