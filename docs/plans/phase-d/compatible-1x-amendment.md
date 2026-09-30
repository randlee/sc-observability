# Phase D: compatible 1.x amendment

The user-approved release direction is the next compatible 1.x release. Retain the released API through deprecation, backed by the new implementation. Remove deprecated API only in a future separately authorized 2.0 release. This decision supersedes earlier Phase D instructions to replace root APIs, remove compatibility, or approve breaking changes for this release. D22 updates the governing requirements and ADRs before implementation consumers start.

Beads hold authoritative sprint scope, acceptance and file ownership. `sprints.jsonl` holds dependency edges. This document records the release decision and graph rationale, not another sprint checklist. Plans are prepared in `plan/phase-d-compatible-1x`, based on `develop`; implementation stays in the existing phase stack on `integrate/phase-d`. No implementation merge to `develop` is authorized. Local `develop` must remain synchronized with its remote.

## Compatibility boundary

All nine crates have a released 1.4.1 baseline, including `sc-observability-otlp`. Preserve only genuinely changed released contracts. New SDK/signal APIs remain additive; do not manufacture legacy counterparts. Keep clean canonical names under `v2` only where released owners/signatures conflict. Reuse aliases/reexports when they preserve identity; a new wrapper is not automatically required for each API-diff row.

Deprecated owners, legacy errors and conversions belong in dedicated `src/compat/` files wherever practical. Crate roots contain only necessary reexports. Canonical modules must not depend on compatibility modules. Adapters delegate into one runtime/backend/lifecycle; they preserve old signatures, error variants and source chains, behavior, trait/auto-trait guarantees, public config literals and wire formats. Existing released typed and macro-generated helper APIs remain obligations. Keep the released event field shape where internal validation avoids duplicating the event/query graph.

D22 audits the existing inventory against the selected implementation head and freezes the exact per-symbol treatment. Its actual additional handwritten type/method counts distinguish existing pairs, restored declarations, adapters, aliases and reexports. The current audit's 58 affected nominal contracts and 141 retained callable contracts are **not** counts of new wrappers. Each implementation sprint records precise removable files and exports for future 2.0; no removal occurs now.

The pinned comparison is v1.4.1 `c578912653233c7dc678fefe5af575118dbbaaa1` versus audited candidate `17155bd313c1a0b4b958417b22208b0d4e320f79`. D22 reconciles later D18 repairs without accepting later blanket 2.0 approvals. The existing audit is at `sc-observability-recovery/compatibility-inventory` alongside the repository; the selected contract and counts become maintained compatibility documentation in D22.

## Seven compatibility sprints

| Stage | Sprint | Owner | Boundary / output |
|---|---|---|---|
| Contracts | D22 `obs-d-22` | cobs | Shared released types plus real canonical facade exports, exact compatibility decisions, governing ADR/requirement amendment |
| Parallel adapters | D23 `obs-d-23` | cobs | Core/logger source and tests |
| Parallel adapters | D24 `obs-d-24` | cobs2 | Observation facade source and tests |
| Parallel adapters | D25 `obs-d-25` | lobs | Log bridge source and tests |
| Parallel adapters | D26 `obs-d-26` | lobs2 | Released OTLP facade source and tests |
| Parallel release work | D27 `obs-d-27` | lobs | Versions, manifests/locks, compatible release validation and CI |
| Integration | Existing D18 `obs-d-18` | cobs | Real combined old/new consumers, bindings, semver and release evidence |

There are six new sprints and one amended sprint. Existing D9 conformance follows D18 and remains separate; it is not an eighth new compatibility sprint. The graph contains 26 entries including historical work. Compatibility critical path is three development stages, theoretical width five; the existing four developer agents limit immediate execution width to four. Including D9, the path is four stages because real collector qualification consumes the integrated library.

D22 supplies **compiled production** canonical exports backed by existing new implementations, not fake mocks. Core/observe/log/OTLP facade roots initially belong to D22 for this narrow publication, then transfer to D23–D26. Frozen canonical signatures let each facade owner consume peers' existing implementation while restoring only its own old surface. Shared type source remains D22-owned. D27 owns manifests, locks and release tooling; the parallel facade sprints own source/tests. Contract drift requires a scoped D22 amendment, not silently serializing unrelated sprints.

All new dev beads and amended D18 depend on `obs-phase-d-plan-qa-2`. D23–D27 also depend on D22 sanity. D18 consumes all six new sanity gates and retained original prerequisite gates. Each sanity PASS routes immediately to quality-mgr QA; D18 closes only after constituent QA and combined QA pass. Important findings and repair beads are closed only by quality-mgr.

## One implementation stack and review sequence

Planned layers 19–24 are D22–D27, after historical D20; D18 and D9 move to 25 and 26. These are planned append positions, not claims about current Git state. Before provisioning, the lead records the actual published stack tip and links layers in completion order. Planned parent WIP/rebase is a finalization concern, not an inferred execution hold. Every completed layer is sanity checked, rebased onto the ordered stack, then QA checked at its final reviewed head. Any changed reviewed behavior gets the necessary renewed checks.

The user requires two review iterations: cobs using `critical-plan-reviewer.md` and a background reviewer using `plan-scope-reviewer.md`, with fixes after each iteration, followed by quality-mgr plan review. The plan gate prevents implementation dispatch until review passes. This amendment does not repeat prior passed runtime work or close important findings based merely on task-completion messages.

Final acceptance includes unchanged released consumers, clean opt-in canonical consumers, both backend paths, bindings/schema/wheels and exact released-package semver comparison. D27 validates the release tooling independently; D18 runs the real combined gate. D9 and the full phase-ending review precede any implementation landing on `develop`, which still requires user authorization.
