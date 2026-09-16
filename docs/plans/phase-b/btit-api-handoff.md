# BTIT implementation handoff for the destination-owned target API

Status: target public-API contract under review; source implementation not yet
accepted. This supersedes earlier wording that gave BTIT authority to accept or
decline destination API recommendations, or deferred destination design until
after copy.

sc-observability owns the [target public API and disposition matrix](target-bridge-api.md).
We review/lock that contract now so BTIT can finish its initial bridge design and
implementation against a stable adopted design. The new companion crate has no
legacy BTIT API/semver obligation; retain useful behavior and identify intentional
changes explicitly. The current target proposal is not itself an approved freeze.

## Required sequence

1. Apply the authoritative [B.1 entry gate](sprint-b-1-copy.md#goal-and-entry-gate)
   for the accepted destination contract and source/review evidence.
2. B.P1 implements and B.P2 publishes the runtime-level capability; B.P3 owns
   BTIT integration and acceptance against that contract before source handoff.
3. B.1 copies only the accepted generic source and verifies its exported API and
   behavior against that locked target. The copy is mechanical; no second
   destination API redesign is scheduled afterward.
4. B.1a–B.1e add the destination core error API and warning-only legacy adapters,
   preserving the accepted bridge contract; it does not schedule bridge redesign.
   B.2 publishes the core update and companion pair. B.3–B.6 implement the destination-owned
   TypeScript/Python/async binding proposals; B.7 publishes those artifacts.

BTIT retains application authorization, UI, filesystem deletion, and its later
published-dependency switch. This planning record does not itself authorize a
BTIT code change or claim that its implementation/review is complete.

## Current source observations

At inspected `5fd63ca697fb36e91d754610cb631b1ddb9a3a31`, init/flush/shutdown already
use Rust Result with typed errors. Public direct submission and lifecycle/health
ownership need the target contract review; hidden emit/guarded helpers currently
return unit. These observations inform the matrix but are not accepted source
readiness. Neither this SHA nor `f6f69dc` is selected for import.

The target deliberately retains unit-return standard log::Log/macro adapters and
adds a direct result-preserving path. Error/health wire projections belong to
sc-observability's later bindings; BTIT implements the accepted neutral Rust
contract without Tauri/PyO3/exporter dependencies in the generic bridge.
