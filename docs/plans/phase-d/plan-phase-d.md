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

## Requirement mapping

| Original item | Bead#item(s) |
| --- | --- |
| D.1.1 | D1#1 |
| D.1.2 | D1#2 |
| D.1.3 | D1#3 |
| D.1.4 | D1#4 |
| D.2.1 | D2#1 |
| D.2.2 | D2#2 |
| D.2.3 | D2#3 |
| D.2.4 | D2#4 |
| D.2.5 | D2#5 |
| D.2.6 | D2#6 |
| D.3.1 | D3#1 |
| D.3.2 | D3#2 |
| D.3.3 | D3#3 |
| D.3.4 | D3#4 |
| D.4.1 | D12#1 |
| D.4.2 | D12#1 |
| D.4.3 | D18#1 |
| D.4.4 | D18#3 |
| D.4.5 | D12#4, D18#4 |
| D.4.6 | D13#4, D4#2 |
| D.4.7 | D18#3 |
| D.4.8 | D18#4 |
| D.5.1 | D5#1 |
| D.5.2 | D5#2 |
| D.5.3 | D5#3 |
| D.5.4 | D5#4 |
| D.5.5 | D5#5 |
| D.5.6 | D5#6 |
| D.6.1 | D6#1 |
| D.6.2 | D6#2 |
| D.6.3 | D6#3 |
| D.6.4 | D6#4 |
| D.6.5 | D6#5 |
| D.6.6 | D6#6 |
| D.7.1 | D7#1 |
| D.7.2 | D7#2 |
| D.7.3 | D7#3 |
| D.7.4 | D7#4 |
| D.8.1 | D8#1 |
| D.8.2 | D8#2 |
| D.8.3 | D8#3 |
| D.8.4 | D8#4 |
| D.8.5 | D8#5 |
| D.8.6 | D8#6 |
| D.9.1 | D9#1 |
| D.9.2 | D9#2 |
| D.9.3 | D9#3 |
| D.9.4 | D9#4 |
| D.10.1 | D10#1 |
| D.10.2 | D10#2 |
| D.10.3 | D10#3 |
| D.10.4 | D10#4 |
| D.11.1 | D11#1 |
| D.11.2 | D11#2 |
| D.11.3 | D11#3 |
| D.11.4 | D11#4 |

| Current bead item not referenced | Reason |
| --- | --- |
| D12#5 | OTLP module/feature registration is a contract split from D6. |
| D14#4–D17#3 | per-crate migration evidence and documentation. |
