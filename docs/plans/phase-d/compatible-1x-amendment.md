# Phase D: compatible 1.x amendment

The user-approved release direction is the next compatible 1.x release. Retain the released API through deprecation, backed by the new implementation. Remove deprecated API only in a future separately authorized 2.0 release. This decision supersedes earlier Phase D instructions to replace root APIs, remove compatibility, or approve breaking changes for this release. Accepted ADR-020 and the governing requirement/ADR amendments land in this planning PR. D22 implements that existing policy and freezes the compiled compatibility contracts; it does not defer the release-policy decision until implementation.

Beads hold authoritative sprint scope, acceptance and file ownership. `sprints.jsonl` holds dependency edges. This document records the release decision and graph rationale, not another sprint checklist. Plans are prepared in `plan/phase-d-compatible-1x`, based on `develop`; implementation stays in the existing phase stack on `integrate/phase-d`. No implementation merge to `develop` is authorized. Local `develop` must remain synchronized with its remote.

## Compatibility boundary

All nine crates have a released 1.4.1 baseline, including `sc-observability-otlp`. Preserve only genuinely changed released contracts. New SDK/signal APIs remain additive; do not manufacture legacy counterparts. Keep clean canonical names under `v2` only where released owners/signatures conflict. Reuse aliases/reexports when they preserve identity; a new wrapper is not automatically required for each API-diff row.

Deprecated owners, legacy errors and conversions belong in dedicated `src/compat/` files wherever practical. Crate roots contain only necessary reexports. Canonical modules must not depend on compatibility modules. Adapters delegate into one runtime/backend/lifecycle; they preserve old signatures, error variants and source chains, behavior, trait/auto-trait guarantees, public config literals and wire formats. Existing released typed and macro-generated helper APIs remain obligations. Keep the released event field shape where internal validation avoids duplicating the event/query graph.

The current audit's 58 affected nominal contracts and 141 retained callable contracts are **not** counts of new wrappers. The shared execution rules below name the one maintained contract/removal inventory and final count owner. No removal occurs now.

The pinned comparison is v1.4.1 `c578912653233c7dc678fefe5af575118dbbaaa1` versus audited candidate `17155bd313c1a0b4b958417b22208b0d4e320f79`. D22 reconciles later D18 repairs without accepting later blanket 2.0 approvals. The existing audit is at `sc-observability-recovery/compatibility-inventory` alongside the repository; D22 maintains the selected contract/removal registry, and D18 reports final implemented counts.

## Scheduling and ownership

There are seven new sprints (D22–D28) and one amended sprint (D18): eight compatibility sprints. Existing D9 remains a separate conformance sprint, giving nine active sprints and 27 graph entries including historical work. The longest execution path is three development stages, maximum stage width five, and four existing developers limit immediate concurrency to four.

Historical phase grouping and execution readiness are different fields. `metadata.phase_wave` and `phase-wave:N` retain group 3 for compatibility/D18 and group 4 for D9. `metadata.wave` and `wave:N` describe the actual execution levels below.

| Execution wave | Historical phase group | Sprint / owner | Prerequisite sprint artifacts | Boundary / output |
|---|---|---|---|---|
| 1 | 3 | D22 / cobs | None | Real compiled shared compatibility contracts and export seams |
| 1 | 3 | D27 / lobs | None | Fixture-backed release validation, versions, manifests/locks and CI |
| 2 | 3 | D23 / cobs | D22 | Core/logger source and tests |
| 2 | 3 | D24 / cobs2 | D22 | Observation facade source and tests |
| 2 | 3 | D25 / lobs | D22 | Log bridge plus thin macro producer/external consumer proof |
| 2 | 3 | D26 / lobs2 | D22 | Released OTLP facade and real exporters |
| 2 | 3 | D28 / cobs2 | D22 | Atomic DTO/binding-runtime/language conversion and example compatibility |
| 3 | 3 | D18 / cobs | D22–D28 | Combined verification, migration docs and final actual count report |
| 3 | 4 | D9 / cobs | D22 + D26 | Real collector conformance against the completed exporter |

These prerequisites resolve to sanity gates in the committed graph. D27 uses accepted policy and release-validator fixtures, so it consumes no D22 implementation artifact and runs alongside D22. D9 consumes D26's actual exporter and D22's compatible contracts; the former D9→D18 edge is removed. D9 need not wait for unrelated binding/release integration. Both D18 and D9 must pass before phase-ending review; neither substitutes for the other.

D22 supplies compiled production exports backed by the existing implementation, never fake mocks. Its bead has one `owned_paths` list and structured `ownership_transfers` selectors: every granted prerequisite import/export/fixture path is sequentially handed to D23–D26 or D28 after D22 sanity. These are not concurrent editing permissions. Shared type source stays D22-owned. Only changed canonical identities move to `types::v2` or peer canonical namespaces; unchanged identities/constants and released-consumer fixtures retain their paths.

D27 owns manifests, locks and release tooling. D23–D26 own their respective facade source/tests. D28 owns DTO/binding-runtime/binding/example source, excluding manifests/locks; its boundary exception is the atomic native-failure-to-wire conversion plus its thin language consumers, which must agree on a single representation. It does not introduce another sprint or runtime. OTLP observation imports remain dev-only.

D18 has no implicit permission to edit facade, binding or release implementation. Combination defects return to D22–D28 owners. D18 owns its explicit combined-test fence, migration docs and final count report. D9's scope lives in its bead; the old sprint markdown projection is not an implementation deliverable.

## Shared execution rules

Accepted ADR-020 and the amended governing requirements/ADRs must be present at the reviewed planning revision before any sprint starts. D22 implements this policy; it does not create a future authorization dependency. The plan gate `obs-phase-d-plan-qa-2` blocks implementation dispatch until quality-mgr approves the amendment. That approval completes planning only; the current user implementation hold remains until explicitly released.

The one compatibility inventory/removal list is D22-owned `docs/compatibility/registry.json`, with signatures in `docs/compatibility/signatures.md`. Other owners submit changes through D22 rather than concurrently editing shared documents or creating separate ledgers. Contract drift is a scoped owner change, then consumers revalidate. After the parallel sanity gates pass, registry.json transfers to D18 for final accounting; signature policy stays D22-owned. D18 derives actual additional handwritten type/method counts once from the combined implementation.

Implementation remains one ordered stack on `integrate/phase-d`. Planned positions are D22–D28 at layers 19–25, D18 at 26 and D9 at 27; these are append intent, not current Git state or execution dependencies. Before provisioning, the lead records the published stack tip and actual targets. A chained `pr_target` or parent WIP affects finalization and rebasing, not starting ready scoped work. Completion order may change the recorded physical layer order without adding execution holds.

Every completed layer proceeds through sanity, rebase onto the ordered stack, then immediate quality-mgr QA at its final reviewed head; changed behavior receives the necessary renewed checks. Important findings and repair beads are closed only by quality-mgr. Contract/boundary checks stay scoped to workspace build, owned tests and boundary/production lint. D27 proves baseline/additive success, breaking/missing-evidence rejection and version/CI coherence using fixtures. D18 alone proves the real combined old/new consumer and all-crate semver behavior; D9 proves real collector conformance.

No implementation merge to `develop` is authorized here. D18 and D9 PASS plus full phase-ending review and explicit user authorization are required. The two requested scope/critical review iterations precede quality-mgr plan review; preserved review reports are evidence, not live scheduling authority.

## Common validation

Each boundary sprint completes its numbered deliverables and inventory rows with real implementations. Run `cargo check --workspace --all-features --locked`, focused owned-crate/consumer tests, and existing boundary and production-lint checks for the changed paths. D18 owns combined workspace behavior and `just validate`; boundary closure does not add unrelated full-phase integration checks.

Use the documented feature/profile matrix, including the release-only static-level fixture in release profile. Preserve the exact failing command and result if blocked; never relabel a failure PASS. Old and canonical clients agree on side effects while retaining their documented result shapes; unchanged released consumers stay unchanged, and migrated examples opt into canonical APIs. Accepted policy, rebase/revalidation and immediate sanity-to-QA routing are defined in Shared execution rules above.

## Planning review record

Iteration 1 scope and critical reports are attached to PR #499 (issuecomment-5902245963). The fix round clarified canonical-import/fixture handoff, D18 owner routing, macro expansion proof, the current phase map and D9 train, and D27 boundary closure. OTLP observation imports remain dev-only. The completed second review used the historical frozen bead snapshot SHA-256 `181dcf0ab9f88544c4d3a6cc5f8c8d381402635f1d7e73a5490d9fe4b6cefc7b`. That digest identifies review-2 evidence, not the current plan after quality-mgr fixes; live beads and the committed graph remain authoritative.
