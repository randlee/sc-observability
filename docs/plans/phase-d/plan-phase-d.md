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

Source: the 56 numbered deliverables of D.1 to D.11 as written in develop's `docs/plans/phase-d` sprint docs, mapped by content to the bead items that carry the work after the re-cut. Verified by the lead on 2026-09-26 against each bead's numbered deliverables. Retire this section when phase d closes.

| Original item | Bead#item(s) |
| --- | --- |
| D.1.1 | d-13#1 |
| D.1.2 | d-1#1 |
| D.1.3 | d-1#2 |
| D.1.4 | d-1#3 |
| D.2.1 | d-13#2 |
| D.2.2 | d-2#1 |
| D.2.3 | d-2#2 |
| D.2.4 | d-2#3 |
| D.2.5 | d-2#4 |
| D.2.6 | d-2#5 |
| D.3.1 | d-13#3, d-3#1 |
| D.3.2 | d-13#3, d-3#2 |
| D.3.3 | d-3#3 |
| D.3.4 | d-3#3 |
| D.4.1 | d-12#1 |
| D.4.2 | d-12#1 |
| D.4.3 | d-4#1, d-14#1, d-14#2, d-15#1, d-15#2, d-16#1, d-17#1, d-18#1, d-18#5 |
| D.4.4 | d-18#3 |
| D.4.5 | d-12#4, d-18#4 |
| D.4.6 | d-13#4, d-4#1, d-4#2 |
| D.4.7 | d-18#3 |
| D.4.8 | d-18#4 |
| D.5.1 | d-12#2 |
| D.5.2 | d-5#1 |
| D.5.3 | d-5#2 |
| D.5.4 | d-5#3 |
| D.5.5 | d-5#4 |
| D.5.6 | d-5#5 |
| D.6.1 | d-12#3, d-6#1 |
| D.6.2 | d-6#2 |
| D.6.3 | d-6#3 |
| D.6.4 | d-6#3 |
| D.6.5 | d-6#3 |
| D.6.6 | d-6#4, d-18#3 |
| D.7.1 | d-7#1 |
| D.7.2 | d-7#2 |
| D.7.3 | d-7#3, d-18#2 |
| D.7.4 | d-7#4 |
| D.8.1 | d-8#1 |
| D.8.2 | d-8#2 |
| D.8.3 | d-8#3 |
| D.8.4 | d-8#4 |
| D.8.5 | d-8#5 |
| D.8.6 | d-8#6 |
| D.9.1 | d-9#1 |
| D.9.2 | d-9#2 |
| D.9.3 | d-9#3 |
| D.9.4 | d-9#4 |
| D.10.1 | d-10#1 |
| D.10.2 | d-10#2 |
| D.10.3 | d-10#3 |
| D.10.4 | d-10#4 |
| D.11.1 | d-11#1 |
| D.11.2 | d-11#2 |
| D.11.3 | d-11#3 |
| D.11.4 | d-11#4 |

Current bead items no original item names; each exists because the re-cut split contracts, migrations and composition out of the original sprints:

| Current bead item not referenced | Reason |
| --- | --- |
| d-12#5 | OTLP module and feature declarations hoisted into the contract layer so every implementation sprint compiles against one registration (from D.6/D.7 crate setup). |
| d-14#3, d-14#4 | sc-observe migration tests and sprint documentation (evidence for D.4.3 in that crate). |
| d-15#3, d-15#4 | binding-runtime migration tests and sprint documentation (evidence for D.4.3 in that crate). |
| d-16#2, d-16#3 | sc-observability-log migration tests and sprint documentation (evidence for D.4.3 in that crate). |
| d-17#2, d-17#3 | consumer-check and example migration tests and sprint documentation (evidence for D.4.3 in those targets). |
