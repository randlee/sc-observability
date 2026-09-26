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

| Wave | Track | Sprints | Target boundary | Owned paths | Assignee / model |
| --- | --- | --- | --- | --- | --- |
| 1 | contracts | D.10, D.12, D.13 | wheel policy; types/OTLP and logging contracts | wheel files; contract modules; OTLP declarations/features | D.10 lobs/luna; D.12 lobs/luna; D.13 cobs/terra |
| 2 | bounded implementations | D.1–D.8, D.14–D.17 | one crate or module per sprint | disjoint crate/module fences | D.1 cobs/terra; D.2 lobs/luna; D.3 cobs/terra; D.4 lobs/luna; D.5 cobs/terra; D.6 lobs/luna; D.7 cobs/terra; D.8 lobs/luna; D.14 cobs/terra; D.15 lobs/luna; D.16 cobs/terra; D.17 lobs/luna |
| 3 | library/API | D.18 | composition and release/public API | shared release/API files | D.18 cobs/terra |
| 4 | qualification | D.9, D.11 | dual exporter conformance and Python distribution guard | collectors, conformance tests, docs, and guard scripts | D.9 cobs/terra; D.11 lobs/luna |

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
