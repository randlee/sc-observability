# Phase D plan

Generated projection of `obs-phase-d`; beads are authoritative.

## Phase D — logging, OTLP, and Python distribution

The corrected plan has nineteen dev sprints. obs-d-11 is folded into obs-d-10; obs-d-19 and obs-d-20 split DTO/schema and language adapter implementation out of integration. Beads are authoritative; sprint documents are review projections. Concrete artifact sanity gates determine execution; layer/pr_target is merge order only. One append-only phase stack targets integrate/phase-d.

## Sprint and wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | lobs/luna | root | 1 | sc-observability-types and OTLP contract |
| 1 | obs-d-13 | cobs/terra | root | 2 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 3 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 4 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 5 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 6 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 7 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 8 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 9 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 10 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 11 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 12 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 13 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 14 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 15 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 16 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 17 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 18 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 19 | OTLP dual-path qualification |

## Boundary map and execution graph

obs-d-12 freezes canonical errors/signals, OTLP config/lifecycle contracts and wire projection specifications. obs-d-13 independently freezes logging shapes/signatures with baseline-only private fixtures. obs-d-1–8/14–17 consume their relevant contract gates and implement bounded modules. obs-d-19 owns DTO/schema/generated models; obs-d-20 owns language extraction/transport adapters. They consume the same frozen wire contract and close independently using local fixtures; integration supplies real cross-layer composition. obs-d-18 activates the completed library and owns release/API closure; obs-d-9 qualifies real collectors. obs-d-10 independently closes native Windows ARM64 preparation and the folded open-ended Python guard/artifact proof, handing the release policy specification/evidence to obs-d-18.

## Rulings

1. obs-d-12 owns Cargo versions/lock/workspace registration, OTLP module roots, boundary records and normative architecture/requirements/API-design docs; obs-d-18 alone owns project-plan.md. The explicit exception is examples/otlp-sdk/Cargo.toml, owned by obs-d-7. Module implementation/test placeholders are handed off to their wave-2 owners; module roots remain read-only.
2. obs-d-12 creates every declared file, including assembly.rs/projectors.rs before handing those to obs-d-5. It owns shared names and registry rows, including types error_codes.rs::otlp; OTLP re-exports those codes. obs-d-7/8 implement distinct backend files and call obs-d-6 lifecycle barriers rather than duplicate ordering/admission control.
3. **ADR-017/018 acceptance: accepted 2026-09-26 via user merge of PR #225**. ADR-012 is superseded only for enumerated 2.0 breaks. PHB-003/004/005 are explicitly 1.x; PHD-001–004 define the reviewed 2.0 requirements. ADR-019 is **Proposed; accepted when the plan-fix PR merges**. The separate retrospective acceptance of ADR-011–016 is treated as Accepted per the lead ruling; no new decision is hidden in those historical records.
4. obs-d-18 alone owns release/**, API approvals, changelog/release notes, migration guidance, final semver baseline and wrapper/classification/adapter removal. Migration beads change call sites and tests only. DTO/schema/generated output belongs to obs-d-19; Python/TS/Tauri adapter implementation belongs to obs-d-20. Their wave-2 fences are disjoint and paired sanity checks gate obs-d-18.
5. PLAN-SCOPE-016 lead ruling: obs-d-13's final concrete signatures are specifications; compiled wave-1 fixtures use private error-parameterized harnesses with baseline types only. No new obs-d-12 error or registry imports occur there. obs-d-1/2/3 bind both contract artifacts in owned runtime/bridge/builder files in wave 2; obs-d-18 activates typed.rs exports. No edge between roots.
6. Same-wave ownership overlaps must be zero. Cross-wave path/artifact handoffs are named by both producer and consumer. Shared files are not a reason to serialize otherwise independent wave-2 work. The two new beads are must_follow consumers of the frozen contract; the stack chain is not an execution edge.
7. **wave 4: accepted 2026-09-26T06:28Z by the lead under user-delegated authority**. Critical path four is accepted. obs-d-9 remains its own conformance/qualification sprint after obs-d-18 with its CI workflow, fixtures and Grafana smoke; folding it into integration would widen that already bounded sprint. obs-d-11 is folded into obs-d-10 and no longer a fourth-wave bead.

## Workspace invariant and replace-versus-coexist sequence

Every sprint closes with `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` green. obs-d-12 introduces canonical errors_v2.rs/signals_v2.rs under an explicit v2 path alongside functioning 1.x exports. Implementations migrate their call sites using temporary compatibility; they do not remove wrappers, classifiers or adapters. obs-d-18 switches canonical public exports and alone removes compatibility after all implementation sanity gates. This is temporary sequencing, not the final API: ADR-017 replacement remains the release contract. Private fixtures and module stubs are contract artifacts only; enabled production construction never reports success through a no-op. obs-d-18 additionally runs all-features release tests, real combined bindings and semver/removal gates. This invariant is stated once here; sprint criteria reference it.

## Parallelism after correction

19 dev sprints, four numbered waves, critical path four dev sprints (obs-d-12 → obs-d-7 → obs-d-18 → obs-d-9), excluding sanity/plan-QA gate nodes. Wave 2 has fourteen boundary beads. Maximum independent width is fifteen when root obs-d-10 overlaps the fourteen released boundary implementations. Adding obs-d-19/20 increases width without adding path length; folding obs-d-11 removes its separate thin closure. Actual handoffs live in the bead designs; no manually counted edge list is published. Wave 4 and critical path four are accepted by the delegated lead ruling above.

## Wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | lobs/luna | root | 1 | sc-observability-types and OTLP contract |
| 1 | obs-d-13 | cobs/terra | root | 2 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 3 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 4 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 5 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 6 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 7 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 8 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 9 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 10 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 11 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 12 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 13 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 14 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 15 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 16 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 17 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 18 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 19 | OTLP dual-path qualification |

## Requirement mapping


Source: the 56 numbered deliverables of D.1 to D.11 as written in develop's `docs/plans/phase-d` sprint docs, mapped by content to the bead items that carry the work after the re-cut. Updated after round-2 splitting/folding on 2026-09-26; each current target is checked against its bead deliverables. Retire this section when phase d closes.

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
| D.4.1 | d-12#2 |
| D.4.2 | d-12#2 |
| D.4.3 | d-4#1, d-14#1, d-14#2, d-15#1, d-15#2, d-16#1, d-17#1, d-18#1, d-19#1, d-20#1, d-20#2 |
| D.4.4 | d-12#6, d-18#3 |
| D.4.5 | d-12#1, d-18#4 |
| D.4.6 | d-13#4, d-18#1 |
| D.4.7 | d-12#6, d-18#3 |
| D.4.8 | d-18#4 |
| D.5.1 | d-12#3 |
| D.5.2 | d-5#1 |
| D.5.3 | d-5#2 |
| D.5.4 | d-18#4 |
| D.5.5 | d-12#3, d-14#1, d-15#1, d-19#1 |
| D.5.6 | d-12#2, d-5#3 |
| D.6.1 | d-12#4, d-6#1 |
| D.6.2 | d-6#2 |
| D.6.3 | d-6#3 |
| D.6.4 | d-6#3 |
| D.6.5 | d-6#3 |
| D.6.6 | d-6#4, d-18#4 |
| D.7.1 | d-12#5, d-7#1 |
| D.7.2 | d-7#2 |
| D.7.3 | d-7#3, d-18#2 |
| D.7.4 | d-7#4 |
| D.8.1 | d-8#1 |
| D.8.2 | d-8#2 |
| D.8.3 | d-8#3 |
| D.8.4 | d-8#4 |
| D.8.5 | d-8#5 |
| D.8.6 | d-12#5, d-8#6 |
| D.9.1 | d-9#1 |
| D.9.2 | d-9#2 |
| D.9.3 | d-9#3 |
| D.9.4 | d-9#4 |
| D.10.1 | d-10#1 |
| D.10.2 | d-10#2 |
| D.10.3 | d-10#3, d-10#7 |
| D.10.4 | d-10#4, d-18#3 |
| D.11.1 | d-10#5 |
| D.11.2 | d-10#6 |
| D.11.3 | d-10#7 |
| D.11.4 | d-10#8 |



## Current deliverable additions

obs-d-19 #1–3 own DTO/schema/generated-model migration formerly included in obs-d-18 #5; obs-d-20 #1–3 own language adapter migration. obs-d-18 #5 now qualifies their real composition with logging and transport artifacts. Former obs-d-11 #1–4 map to obs-d-10 #5–8. Per-sprint document projection is supporting review material, not an independent closure gate.

## Acceptance criteria

- [ ] every sprint of the phase closed
- [ ] every finding closed with a close reason
- [ ] phase PR integrate/phase-d -> develop merged
