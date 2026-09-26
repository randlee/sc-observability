# Phase D plan

Generated projection of `obs-phase-d`; beads are authoritative.

## Phase D — logging, OTLP, and Python distribution

The plan has twenty dev sprints. obs-d-12 is the types root; obs-d-21 consumes it as the OTLP contract stage. Types-only consumers release after obs-d-12-sanity, while obs-d-5–8 release after obs-d-21-sanity. obs-d-11 remains folded into obs-d-10. Beads are authoritative; documents project them. Concrete artifact gates determine execution; layer/pr_target records merge order only. One append-only phase stack targets integrate/phase-d.

## Sprint and wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | aobs/astra | root | 1 | sc-observability-types 2.0 contract |
| 1 | obs-d-21 | lobs/luna | must_follow | 2 | sc-observability-otlp contract and workspace registration |
| 1 | obs-d-13 | cobs/terra | root | 3 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 4 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 5 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 6 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 7 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 8 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 9 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 10 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 11 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 12 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 13 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 14 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 15 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 16 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 17 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 18 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 19 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 20 | OTLP dual-path qualification |

## Boundary map and execution graph

obs-d-12 freezes canonical errors/signals and wire projection specifications; obs-d-21 consumes them to freeze OTLP config/lifecycle contracts and workspace registration. obs-d-13 independently freezes logging shapes/signatures with baseline-only private fixtures. obs-d-1–8/14–17 consume their relevant contract gates and implement bounded modules. obs-d-19 owns DTO/schema/generated models; obs-d-20 owns language extraction/transport adapters. They consume the same frozen wire contract and close independently using local fixtures; integration supplies real cross-layer composition. obs-d-18 activates the completed library and owns release/API closure; obs-d-9 qualifies real collectors. obs-d-10 independently closes native Windows ARM64 preparation and the folded open-ended Python guard/artifact proof, handing the release policy specification/evidence to obs-d-18.

## Rulings

1. obs-d-12 owns shared types/error/model source, its Cargo manifest, dedicated non-OTLP registry/constants files and shared normative architecture/requirements/API-design documents. obs-d-21 owns root Cargo.toml/Cargo.lock, other workspace manifests/features, OTLP module roots/stubs, boundary allowlists and atomic workspace version activation. The SDK example Cargo.toml stays with obs-d-7; project-plan.md stays with obs-d-18.
2. obs-d-12 publishes the reviewed normative specification and section6 allowlist as read-only input to obs-d-21; OTLP exact implementation pins live in obs-d-21 manifests/lock/boundaries. obs-d-12 owns all ConfigFailure/ExportError variants and types error_codes.rs::otlp; obs-d-21 supplies re-exports and validation/conversion only. obs-d-21 creates every declared OTLP file before handing assembly/projector/lifecycle/backend implementation stubs to obs-d-5–8. Module roots remain read-only to implementations.

3. **ADR-017/018 acceptance: accepted 2026-09-26 via user merge of PR #225**. ADR-012 is superseded only for enumerated 2.0 breaks. PHB-003/004/005 are explicitly 1.x; PHD-001–004 define the reviewed 2.0 requirements. **ADR-019 acceptance: accepted 2026-09-26 via PR #227**. The separate retrospective acceptance of ADR-011–016 is treated as Accepted per the lead ruling; no new decision is hidden in those historical records.
4. obs-d-18 alone owns release/**, API approvals, changelog/release notes, migration guidance, final semver baseline and wrapper/classification/adapter removal. Migration beads change call sites and tests only. DTO/schema/generated output belongs to obs-d-19; Python/TS/Tauri adapter implementation belongs to obs-d-20. Their wave-2 fences are disjoint and paired sanity checks gate obs-d-18.
5. PLAN-SCOPE-016 lead ruling: obs-d-13's final concrete signatures are specifications; compiled wave-1 fixtures use private error-parameterized harnesses with baseline types only. No new obs-d-12 error or registry imports occur there. obs-d-1/2/3 bind both contract artifacts in owned runtime/bridge/builder files in wave 2; obs-d-18 activates typed.rs exports. No edge between roots.
6. Same-wave ownership overlaps must be zero. Cross-wave path/artifact handoffs are named by both producer and consumer. Shared files are not a reason to serialize otherwise independent wave-2 work. The two new beads are must_follow consumers of the frozen contract; the stack chain is not an execution edge.
7. **wave 4: accepted 2026-09-26T06:28Z by the lead under user-delegated authority**. The four-stage baseline was accepted; the subsequent scoped obs-d-12/21 split produces the measured five-sprint path recorded below. obs-d-9 remains its own conformance/qualification sprint after obs-d-18 with its CI workflow, fixtures and Grafana smoke; folding it into integration would widen that already bounded sprint. obs-d-11 is folded into obs-d-10 and no longer a fourth-wave bead.

8. Scoped split authorized by the user; no plan-review round is restarted. Lead ruling 01M3E7Z46AA9DNNMC73BDE37JG assigns obs-d-12 to aobs and obs-d-21 to lobs. Numbered wave 1 contains independent roots obs-d-12/13/10 plus obs-d-21 as a second contract stage following obs-d-12-sanity. Only the four OTLP implementation consumers (obs-d-5–8) move from types sanity to OTLP sanity; obs-d-18 explicitly waits on obs-d-21-sanity too. Layers 1..20 are unique; layer2 targets the types branch, layer3 targets the OTLP-contract branch.
9. Version-line exception: the types manifest fence stays with obs-d-12, which stages the v2 API at the current Cargo version. A handoff in both designs permits obs-d-21 to edit only the types manifest version literal during the atomic workspace bump. This explicit exception is serialized by obs-d-12-sanity and does not duplicate path fences. Shared normative documents remain read-only to obs-d-21.

## Workspace invariant and replace-versus-coexist sequence

Every sprint closes with `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` green. obs-d-12 introduces canonical errors_v2.rs/signals_v2.rs under an explicit v2 path alongside functioning 1.x exports at the current Cargo version. obs-d-21 later activates all workspace 2.0 versions atomically using the explicit types-version-line handoff; types-only consumers need not wait for that activation. Implementations migrate their call sites using temporary compatibility; they do not remove wrappers, classifiers or adapters. obs-d-18 switches canonical public exports and alone removes compatibility after all implementation sanity gates. This is temporary sequencing, not the final API: ADR-017 replacement remains the release contract. Private fixtures and module stubs are contract artifacts only; enabled production construction never reports success through a no-op. obs-d-18 additionally runs all-features release tests, real combined bindings and semver/removal gates. This invariant is stated once here; sprint criteria reference it.

## Parallelism after correction

20 dev sprints and four numbered waves; numbered wave 1 has two dependency stages. Measured critical path is five dev sprints: obs-d-12 → obs-d-21 → obs-d-7 → obs-d-18 → obs-d-9 (sanity/plan-QA gates excluded). Do not label it four merely because both contracts are in wave 1. The scoped split releases seven direct types-contract consumers independently of OTLP work: obs-d-4/14/15/16/17/19/20; obs-d-17 also retains its logging-contract gate. obs-d-1/2/3 keep their existing logging-contract gates. Wave 2 has fourteen beads; maximum dependency-independent width remains fifteen with root obs-d-10 alongside released implementations. The useful gain is earlier release of types-only work, not a claimed shorter OTLP critical path.

## Wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | aobs/astra | root | 1 | sc-observability-types 2.0 contract |
| 1 | obs-d-21 | lobs/luna | must_follow | 2 | sc-observability-otlp contract and workspace registration |
| 1 | obs-d-13 | cobs/terra | root | 3 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 4 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 5 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 6 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 7 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 8 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 9 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 10 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 11 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 12 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 13 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 14 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 15 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 16 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 17 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 18 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 19 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 20 | OTLP dual-path qualification |

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
| D.4.4 | d-12#1, d-21#3, d-18#3 |
| D.4.5 | d-12#1, d-18#4 |
| D.4.6 | d-13#4, d-18#1 |
| D.4.7 | d-12#1, d-21#3, d-18#3 |
| D.4.8 | d-18#4 |
| D.5.1 | d-12#3 |
| D.5.2 | d-5#1 |
| D.5.3 | d-5#2 |
| D.5.4 | d-18#4 |
| D.5.5 | d-12#3, d-14#1, d-15#1, d-19#1 |
| D.5.6 | d-12#2, d-5#3 |
| D.6.1 | d-21#1, d-6#1 |
| D.6.2 | d-6#2 |
| D.6.3 | d-6#3 |
| D.6.4 | d-6#3 |
| D.6.5 | d-6#3 |
| D.6.6 | d-6#4, d-18#4 |
| D.7.1 | d-21#2, d-7#1 |
| D.7.2 | d-7#2 |
| D.7.3 | d-7#3, d-18#2 |
| D.7.4 | d-7#4 |
| D.8.1 | d-8#1 |
| D.8.2 | d-8#2 |
| D.8.3 | d-8#3 |
| D.8.4 | d-8#4 |
| D.8.5 | d-8#5 |
| D.8.6 | d-21#2, d-8#6 |
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

Measured against the 56 original deliverables (lead, 2026-09-26, fix round 2): nine items are additions with no original row: d-4#2, d-14#3, d-15#3, d-16#2 and d-17#2 retype the local tests of each migration boundary; d-18#5 qualifies the composed d-19/d-20 outputs; d-19#2 and d-19#3 update the schema generator, conformance corpus and typing checks; d-20#3 updates transport/runtime/typing fixtures and examples. Every other item in every bead has an original row above.
Split accounting: old obs-d-12 #1 → obs-d-12 #1 plus obs-d-21 #2 (boundary records); old #2 → obs-d-12 #2; old #3 → obs-d-12 #3; old #4 → obs-d-21 #1 plus obs-d-12 #3 (neutral tests); old #5 → obs-d-21 #2; old #6 → obs-d-12 #1 (types manifest/specification) plus obs-d-21 #3 (atomic workspace activation). No numbered deliverable is dropped. The new sprint projection is the only added owned path; the old path/REQ/ADR unions are preserved.

## Acceptance criteria

- [ ] every sprint of the phase closed
- [ ] every finding closed with a close reason
- [ ] phase PR integrate/phase-d -> develop merged
