---
phase: D
status: planned
branch: plan/phase-d
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-d
---

# Phase D — logging, OTLP, and Python distribution

All sprints are planned. The plan has four independent work streams, with only
real code-consumption edges. Each branch starts from `develop`; a
`must_follow` child merge-forwards its parent’s development branch before a
work or fix round. Final release integration happens in
`/Users/randlee/github/sc-observability-worktrees/integrate/phase-d` on
`integrate/phase-d` after the listed parent PRs merge.

## Sprint table

| Sprint | Deliverable | Agent:model | Relation and concrete reason |
| --- | --- | --- | --- | --- |
| D.1 | [Startup `LogSettings`](sprint-d-1-log-settings.md) | cobs:terra | Parallel-safe with D.2/D.3; no bridge or sink API consumes it. |
| D.2 | [Host logger bridge](sprint-d-2-host-logger-bridge.md) | cobs:terra | Parallel-safe with D.1/D.3; it owns only the `log` facade bridge. |
| D.3 | [1.x typed-sink bridge](sprint-d-3-typed-sink-registration.md) | cobs:terra | Parallel-safe with D.1/D.2; a temporary 1.x-only compatibility release. |
| D.4 | [2.0 error migration and release baseline](sprint-d-4-error-enums-2-0.md) | lobs:luna | Root; sole owner of the 2.0 version bump, API-break approval, and canonical error enums. |
| D.5 | [OTLP signal model](sprint-d-5-otlp-signal-model.md) | aobs:astra | Must follow D.4: the model’s breaking API uses D.4’s 2.0 approval/baseline. |
| D.6 | [OTLP lifecycle core](sprint-d-6-otlp-lifecycle-core.md) | aobs:astra | Must follow D.4 and D.5: uses canonical errors and neutral signals. |
| D.7 | [Official SDK/Tokio adapter](sprint-d-7-otlp-sdk-tokio.md) | aobs:astra | Must follow D.6: injects the SDK through the shared lifecycle core. |
| D.8 | [Legacy HTTP/JSON transplant](sprint-d-8-otlp-http-json-transplant.md) | aobs:astra | Must follow D.6: adapts copied transport code to the shared core. |
| D.9 | [Dual-path conformance](sprint-d-9-otlp-conformance.md) | aobs:astra | Must follow D.7 and D.8: requires both operational backends. |
| D.10 | [Windows ARM64 wheel](sprint-d-10-windows-arm64-wheel.md) | cobs:terra | Root; owns the independent six-platform Python matrix. |
| D.11 | [Open-ended Python guard](sprint-d-11-python-open-ended-guard.md) | cobs:terra | Must follow D.10: checks the final matrix and metadata. |

## Branches, worktrees, and merge order

| Stream | Branches / merge order | Worktrees |
| --- | --- | --- |
| logging | `feature/phase-d-{1-log-settings,2-host-logger-bridge,3-typed-sink-registration}` are parallel-safe; cobs works them serially | matching `/Users/randlee/github/sc-observability-worktrees/feature/phase-d-*` paths |
| errors | `feature/phase-d-4-error-enums-2-0` | matching feature worktree |
| OTLP | `feature/phase-d-4-error-enums-2-0` → `feature/phase-d-5-otlp-signal-model` → `feature/phase-d-6-otlp-lifecycle-core` → two children, `feature/phase-d-7-otlp-sdk-tokio` and `feature/phase-d-8-otlp-http-json-transplant` → `feature/phase-d-9-otlp-conformance` | matching feature worktrees; D.7 and D.8 are parallel-safe after D.6 |
| Python | `feature/phase-d-10-windows-arm64-wheel` → `feature/phase-d-11-python-open-ended-guard` | matching feature worktrees; cobs begins after logging |

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
