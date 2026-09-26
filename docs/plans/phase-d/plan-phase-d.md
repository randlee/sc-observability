---
phase: D
status: planned
branch: plan/phase-d-layer-recut
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-d-layer-recut
---

# Phase D — logging, OTLP, and Python distribution

This four-wave library plan is derived from the architecture boundaries:
`sc-observability-types`, `sc-observability-log`, `sc-observability-otlp`,
`sc-observe`, `sc-observability-binding-runtime`, `sc-observability-dto`, and
`sc-observability-log-macros`. Shared contracts have one owner; implementation
fences follow those boundaries and never use `crates/**`.

## Wave table

| Wave | Track | Sprints | Target boundary | Owned paths |
| --- | --- | --- | --- | --- |
| 1 | contracts | D.12 `c-types`, D.13 `c-log` | types/OTLP and logging contracts | contract modules, OTLP declarations/features |
| 2 | bounded implementations | D.1–D.8, D.10, D.14–D.17 | one crate or module per sprint | disjoint crate/module fences |
| 3 | library/API | D.18, D.11 | composition, release/public API and Python guard | shared release/API files |
| 4 | qualification | D.9 | dual exporter conformance | collectors, conformance tests and docs |

**Critical path:** 4 sprints. **Width:** 13 implementation sprints.
**Sprint count:** 18. Every `must_follow` edge consumes a named contract;
D.18 consumes implementation gates, and D.9 consumes D.18's exporter-set
composition. Shared-file overlap is never a dependency rationale.

## Retained gates

- Every dev bead has a sanity bead; dependents wait on sanity, not dev.
- D.18 is the sole owner of shared version, release inventory, public API,
  wrapper removal, and composition wiring.
- D.9 owns hermetic dual-path conformance and OTLP docs. D.11 owns the Python
  regression guard after integration.
- No new transport, registry publication, Python OTEL binding, or `atm-core`
  work is in scope.
