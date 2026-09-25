---
phase: D
status: planned
branch: plan/phase-d
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-d
---

# Phase D — logging, OTLP, and Python distribution

This is a planned implementation; every sprint below is `planned` until
implemented.
Implementation uses independent `gh stack` stacks rooted on `develop`. A stack
merges bottom-to-top with `gh stack merge --yes`; after the stack heads land,
`integrate/phase-d` rebases remaining heads on `develop` and runs final
workspace/release checks. A release-train label is not a code dependency.

## Sprint table

| Sprint | Deliverable | Stack | Agent:model | Depends on / concrete reason |
| --- | --- | --- | --- | --- |
| D.1 | [Startup `LogSettings`](sprint-d-1-log-settings.md) | logging | cobs:terra | — |
| D.2 | [Host logger bridge](sprint-d-2-host-logger-bridge.md) | logging | cobs:terra | D.1: bridge consumes resolved `LogSettings` |
| D.3 | [Typed sink registration](sprint-d-3-typed-sink-registration.md) | logging | cobs:terra | D.2: uses the finalized logger integration boundary |
| D.4 | [2.0 error enums](sprint-d-4-error-enums-2-0.md) | errors | lobs:luna | — |
| D.5 | [OTLP signal model](sprint-d-5-otlp-signal-model.md) | otlp | aobs:astra | —; rebase only if D.4 changes a consumed error type |
| D.6 | [SDK/Tokio exporter](sprint-d-6-otlp-sdk-tokio.md) | otlp | aobs:astra | D.5: consumes neutral signals |
| D.7 | [Legacy HTTP/JSON transplant](sprint-d-7-otlp-http-json-transplant.md) | otlp | aobs:astra | D.6: consumes shared exporter/lifecycle/config code |
| D.8 | [Dual-path conformance](sprint-d-8-otlp-conformance.md) | otlp | aobs:astra | D.7: requires both production paths |
| D.9 | [Windows ARM64 wheel](sprint-d-9-windows-arm64-wheel.md) | python | cobs:terra | — |
| D.10 | [Python ABI/metadata guard](sprint-d-10-python-open-ended-guard.md) | python | cobs:terra | D.9: validates the final wheel matrix |

## Parallel stacks and worktrees

| Stack | Branch order / merge order | Worktrees | Base and integration |
| --- | --- | --- | --- |
| logging | `feature/phase-d-1-log-settings` → `feature/phase-d-2-host-logger-bridge` → `feature/phase-d-3-typed-sink-registration` | `/Users/randlee/github/sc-observability-worktrees/feature/phase-d-{1-log-settings,2-host-logger-bridge,3-typed-sink-registration}` | `develop`; merge bottom-to-top |
| errors | `feature/phase-d-4-error-enums-2-0` | `/Users/randlee/github/sc-observability-worktrees/feature/phase-d-4-error-enums-2-0` | `develop`; runs beside logging and OTLP |
| otlp | `feature/phase-d-5-otlp-signal-model` → `feature/phase-d-6-otlp-sdk-tokio` → `feature/phase-d-7-otlp-http-json-transplant` → `feature/phase-d-8-otlp-conformance` | `/Users/randlee/github/sc-observability-worktrees/feature/phase-d-{5-otlp-signal-model,6-otlp-sdk-tokio,7-otlp-http-json-transplant,8-otlp-conformance}` | `develop`; D.5 rebases only for a consumed D.4 type |
| python | `feature/phase-d-9-windows-arm64-wheel` → `feature/phase-d-10-python-open-ended-guard` | `/Users/randlee/github/sc-observability-worktrees/feature/phase-d-{9-windows-arm64-wheel,10-python-open-ended-guard}` | `develop`; starts when cobs finishes logging and overlaps OTLP |

Start logging, errors, and OTLP together on cobs, lobs, and aobs. The final
integration worktree is `/Users/randlee/github/sc-observability-worktrees/integrate/phase-d`
on `integrate/phase-d`, based on current `develop`.

## Scope and real gates

- D.4 keeps the published-1.4.1 literal scan and reviewed major API comparison;
  the compiler covers ordinary call-site migration, so no per-`ErrorCode` ledger.
- D.5 changes the metric type directly; compiler errors identify old
  `MetricRecord.value` uses, so no inventory ledger.
- D.7 pins source commit/blob/SHA, destination, and disposition. A minimal
  validator compares each transplant destination to its pinned source blob and
  permits only D.7's named source-to-destination deltas.
- D.8 runs ordinary dual-path tests and CI; it retains neither receipt archives
  nor a permanent documentation-restatement CI gate.
- D.10 relies on CI pass/fail and adds no archival artifact.

No `atm-core` implementation, Python OTEL binding, registry publication, or
new transport is included.
