---
phase: D
status: planned
branch: plan/phase-d
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-d
---

# Phase D — logging, OTLP, and Python distribution

All eleven sprints remain planned. The four work streams retain their reviewed
scope. Beads are the execution plan after import; these markdown files become
historical source. The phase uses one append-only stack on `integrate/phase-d`.
Sprint branches are `sprint/d-<n>-<slug>` in matching worktrees beneath
`/Users/randlee/github/sc-observability-worktrees/sprint/`. Planned stack layers
are ordered by dependency depth, then sprint number; the lead records actual
completion order when linking. Final integration remains merge-only.

D.1–D.3 remain parallel and document their additive 1.x surfaces in separate
`docs/logging/d-<n>-<slug>.md` files with separate API approvals. D.4 links those
documents from the shared API design and remains the sole owner of 2.0
requirements, API-design, and release-baseline edits. D.4 follows D.1–D.3 because
its canonical error migration edits their core, types, and bridge source files.

## Sprint table

| Sprint | Deliverable | Agent:model | Relation and concrete reason |
| --- | --- | --- | --- |
| D.1 | [Startup `LogSettings`](sprint-d-1-log-settings.md) | cobs:terra | Parallel-safe with D.2/D.3; no bridge or sink API consumes it. |
| D.2 | [Host logger bridge](sprint-d-2-host-logger-bridge.md) | cobs:terra | Parallel-safe with D.1/D.3; it owns only the `log` facade bridge. |
| D.3 | [1.x typed-sink bridge](sprint-d-3-typed-sink-registration.md) | cobs:terra | Parallel-safe with D.1/D.2; a temporary 1.x-only compatibility release. |
| D.4 | [2.0 error migration and release baseline](sprint-d-4-error-enums-2-0.md) | lobs:luna | Must follow D.1/D.2/D.3: shares their core/types/bridge code; sole 2.0 baseline owner. |
| D.5 | [OTLP signal model](sprint-d-5-otlp-signal-model.md) | aobs:astra | Must follow D.4: the model’s breaking API uses D.4’s 2.0 approval/baseline. |
| D.6 | [OTLP lifecycle core](sprint-d-6-otlp-lifecycle-core.md) | aobs:astra | Must follow D.4 and D.5: uses canonical errors and neutral signals. |
| D.7 | [Official SDK/Tokio adapter](sprint-d-7-otlp-sdk-tokio.md) | aobs:astra | Must follow D.6: injects the SDK through the shared lifecycle core. |
| D.8 | [Legacy HTTP/JSON transplant](sprint-d-8-otlp-http-json-transplant.md) | aobs:astra | Must follow D.6 and D.7: shared OTLP manifest/factory and Cargo.lock; retains the transplant scope. |
| D.9 | [Dual-path conformance](sprint-d-9-otlp-conformance.md) | aobs:astra | Must follow D.7 and D.8: requires both operational backends. |
| D.10 | [Windows ARM64 wheel](sprint-d-10-windows-arm64-wheel.md) | cobs:terra | Must follow D.4: shared release/release-inventory.json; owns Python platform policy independently of OTLP. |
| D.11 | [Open-ended Python guard](sprint-d-11-python-open-ended-guard.md) | cobs:terra | Must follow D.10: checks the final matrix and metadata. |

## Branches, worktrees, and merge order

| Stream | Dependency order | Worktrees |
| --- | --- | --- |
| logging | D.1, D.2, D.3 parallel; cobs works them serially | matching `sprint/d-{1,2,3}-<slug>` paths |
| errors | D.1 + D.2 + D.3 → D.4 | matching `sprint/d-4-error-enums-2-0` path |
| OTLP | D.4 → D.5 → D.6 → D.7 → D.8 → D.9; retain explicit D.4→D.6, D.6→D.8 and D.7→D.9 edges | matching `sprint/d-{5,6,7,8,9}-<slug>` paths |
| Python | D.4 → D.10 → D.11; independent of D.5–D.9 | matching `sprint/d-{10,11}-<slug>` paths |

Only shared code/version-bearing files force the new edges: D.4 migrates
`crates/sc-observability/src/lib.rs`, `crates/sc-observability-types/src/errors.rs`
and the D.2 log bridge; D.7/D.8 share `Cargo.lock`,
`crates/sc-observability-otlp/Cargo.toml`, factory wiring and dependency allowlists;
D.4/D.10 share `release/release-inventory.json`. Documentation-only overlaps in
D.1–D.3 are resolved by separate owned documents, not serialization.
D.3 implements its same-crate inherent `SinkRegistration::typed` in `builder.rs`,
so it does not edit D.1's `lib.rs` module/re-export surface.

## Scope and retained gates

- D.4 alone owns the 2.0 version bump, `release/public-api-major-breaks.toml`,
  the reviewed API comparison, and the literal `1.4.1` scan. It does not own
  Python platform policy.
- D.3 is a 1.x bridge shipped before the 2.0 train. D.4 may remove it only in
  that later major release; no D.3 API is added to the 2.0 baseline.
- D.5 changes the metric type directly; compiler errors identify old
  `MetricRecord.value` uses, so no inventory ledger.
- D.8 retains `legacy-otlp-provenance.json` to prove that the working legacy
  implementation and tests are copied, not rewritten. Its validator compares
  each copied file to the pinned source blob and accepts only named adaptation
  deltas. Translation/reference rows require only their disposition.
- D.9 runs ordinary dual-path tests. It adds no receipt archive or permanent
  documentation-restatement CI gate.
- D.11 relies on CI pass/fail and adds no archival artifact.

No `atm-core` implementation, Python OTEL binding, registry publication, or
new transport is included.
