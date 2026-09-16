---
id: B.3b
status: proposed
branch: feature/phase-b-3b-native-runtime
base: develop
---

# B.3b — Shared native binding backends and bounded operations

## Goal and dependencies

Implement the runtime adapter used by both language bindings. `must_follow`
B.3's DTO contract; B.3a `must_follow` B.3b. This identifier is inserted before
B.3a to preserve the existing TypeScript references. Shared conversions and
operation ownership preclude parallel_safe work. Merge pushed parent changes
before each child dev/fix round; parent PR merges before child completion.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Implement `crates/sc-observability-binding-runtime/` and every exact signature
   in [the native runtime contract](native-binding-runtime.md#public-rust-contract):
   HostLoggingBackend, provided core/bridge backends, unique core owner,
   Operation/OperationState and subscription handling. Update dependency and
   structural CI allowlists, architecture §6 dependency diagram and the
   document-coverage entry: permitted workspace dependencies are exactly core,
   types, DTO and sc-observability-log; no Tauri/PyO3 edge is allowed.
   Third-party support dependencies are reviewed separately, never a bypass
   for a forbidden workspace/runtime edge. Centralize all runtime
   conversions and protected provenance stamping here; hosts consume supplied
   implementations instead of repeating mapping logic.
2. Implement the contract's fixed three-helper coordinator, shared timer service,
   bounded query/flush slots, operation observers, spawn rollback, shutdown
   ownership and final health. Preserve core and accepted bridge public APIs;
   the bridge's pre-copy coordinator remains its own implementation.
3. Add `scripts/ci/validate_binding_runtime.sh`, fault-injection/conformance tests
   and a clean packaged Rust consumer. Use the prepublication source-bundle
   procedure in B.4a, including this unpublished crate. Record dependency/API
   evidence and thread/slot/race results in `handoff-b-3b.md`.

## Contract

```rust
pub fn create_core_backend(config: sc_observability::LoggerConfig)
    -> Result<(CoreLoggerOwner, CoreLoggerBackend), Failure>;
pub fn bridge_backend(control: sc_observability_log::LogControl)
    -> Result<BridgeControlBackend, Failure>;
```

The linked contract is incorporated in full, including every trait method,
operation signature, coordinator bound and error case; no topology choice is
left to downstream Tauri/Python implementation.

## Acceptance criteria (authoritative)

- AC1: Both provided backends pass the same event/query/health/flush conversion
  fixtures; native codes and diagnostics survive and producer provenance cannot
  be forged through either backend. No Tauri/PyO3 dependency enters this crate.
- AC2: All helper-spawn/rollback, slot saturation, timeout/late completion,
  callback/waiter, shutdown and failed-helper cases listed in the native runtime
  contract pass deterministically; worker counts obey the fixed bounds.
- AC3: Read-only handles cannot shut down or mutate owners; surviving handles
  never prevent final shutdown. Bridge-native timeout releases only the adapter slot; a later request may
  pass native overlap from its own prior call or an external caller. Adapter
  overlap uses the binding registry code. Test both modes, native versus observer
  timeout, zero/maximum/invalid durations, eventual bridge completion and a new
  barrier, without retry or false success. Ignored failures remain nonfatal.
- AC4: The packaged external consumer runs both backends using public signatures,
  with unchanged published-core and accepted-bridge API fixtures passing.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_binding_runtime.sh
cargo test --locked -p sc-observability-binding-runtime
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new script runs the complete contract fixture list in debug/release on
macOS/Linux/Windows, captures thread/slot counts, and compiles/runs the isolated
consumer. It fails on skipped cases, overwritten golden diagnostics or missing
platform evidence. Assert the crate’s resolved workspace dependency set equals
the four-crate allowlist and rejects injected Tauri/PyO3 edges. Include the
N=32 producer registration/shutdown fixture with no contention DISPATCH_FULL.
No throughput benchmark substitutes for boundedness tests.

## Paths to delete

None.

## Non-closure

No language API, Tauri registration, PyO3 attachment, new core lifecycle API,
bridge redesign, or registry publication. B.3a/B.4 consume this runtime; B.7
publishes it. This closes native behavior rather than a placeholder trait.
