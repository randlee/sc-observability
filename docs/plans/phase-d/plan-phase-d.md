# Phase D plan

Generated projection of `obs-phase-d`; beads are authoritative.

## Phase D — logging, OTLP, and Python distribution

The plan has eighteen dev sprints. Beads are authoritative; generated sprint documents are review projections. Actual blocker edges are sanity gates; layer/pr_target records merge order only. One append-only phase stack targets integrate/phase-d. No production work may bypass pending user rulings.

## Sprint and wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | lobs/luna | root | 1 | sc-observability-types and OTLP contract |
| 1 | obs-d-13 | cobs/terra | root | 2 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 3 | Windows ARM64 Python distribution |
| 2 | obs-d-1 | cobs/terra | must_follow | 4 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 5 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 6 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 7 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | parallel_safe | 8 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 9 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 10 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 11 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 12 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 13 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 14 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 15 | log consumer migration |
| 3 | obs-d-18 | cobs/terra | must_follow | 16 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 17 | OTLP dual-path qualification |
| 4 | obs-d-11 | lobs/luna | must_follow | 18 | Python distribution guard |

## Boundary map and execution graph

Contract boundaries: sc-observability-types owns canonical errors/neutral signals; sc-observability owns settings and open sink contracts; sc-observability-log owns its attachment companion contract; sc-observability-otlp owns private transport/lifecycle contracts. Existing boundary manifests and ADR-002/011/017/018 govern allowed edges: core -> types; observe -> core/types (never OTLP); OTLP -> core/types with observe dev-only; bridge -> core/types/macros; binding-runtime -> core/types/DTO/bridge; DTO -> types only; language wrappers -> runtime/DTO/types. D.12 records manifest/registry changes. Implementation beads each close their own side using contract test doubles; D.18 composes them; D.9/D.11 qualify real collectors and released-format Python artifacts.

## Rulings

1. All Cargo manifests/lock/workspace registration, optional dependencies, boundary records and allowlists belong to D.12, including D.4.4/D.4.7's 2.0 version bump. D.18 owns release-document version agreement.
2. D.12 creates module-root scaffolding, shared lifecycle/exporter interfaces, each crate's one error registry and separate constants module. D.7/D.8 are independent implementations of those contracts; neither waits for its sibling.
3. D.12 first records the user's acceptance of ADR-017/018 and boundary/status updates; the user is the technical lead. **ADR-017/018 acceptance: accepted 2026-09-26 via user merge of PR #225**; ADR-012 is superseded only in part. D.12 verifies and records that accepted boundary contract; this planning fix does not expand its scope.
4. D.18 alone owns release/**, docs/api-approvals/**, RELEASE-NOTES, CHANGELOG and migration guides. D.12 alone owns architecture/requirements/API-design normative documents. Each sprint owns its generated sprint document.
5. Every signature error has one contract owner and one cause mapping in D.12/D.13. OTLP ConfigFailure is retained as the typed source inside OTLP-005 InitError, not a competing public constructor return. No public Exporter/Signal/ExporterSet is introduced.
6. Initial roots are D.12/D.13/D.10. Same-wave implementation fences are disjoint. Sibling contract consumers without file handoffs are parallel_safe; serial artifact/file-handoff consumers are must_follow under the subsequent lead ruling below. Layer/pr_target chain is merge order, not a dependency edge.
7. Lead ruling 2026-09-26 05:34 UTC: ownership overlap is checked per wave. Every cross-wave overlap is an explicit handoff recorded on both beads. D.12 stages 2.0 canonical modules alongside functioning 1.x contracts; implementations migrate under temporary compatibility; D.18 activates canonical exports and removes compatibility. Every bead closes with cargo check --workspace --all-features and workspace tests green. Contract/implementation scaffolding never counts as final production behavior.
8. **wave 4: pending user ruling (lead recommends approval)**. D.9 needs D.18's composed production exporter factory; D.11 needs its activated six-platform release policy. Keep the fourth wave pending that explicit user decision; do not fold D.9 into D.18 or claim the baseline three-wave approval covers it.

## Parallelism after correction

18 dev sprints; 4 waves; critical path 4 dev sprints (for example D.12 -> D.7 -> D.18 -> D.9), excluding sanity/plan-review gate nodes. Largest numbered wave has 12 beads. Maximum dependency-independent width is 13 because root D.10 may run alongside the 12 released implementation beads. No extra prerequisite edge is introduced: all cross-wave handoffs follow existing direct/transitive sanity gates. The previous claimed nine-sprint path confused the merge-order chain with dependencies. Relation counts and concrete edges are generated below from live metadata; user approval of the fourth wave remains pending.

## Wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | lobs/luna | root | 1 | sc-observability-types and OTLP contract |
| 1 | obs-d-13 | cobs/terra | root | 2 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 3 | Windows ARM64 Python distribution |
| 2 | obs-d-1 | cobs/terra | must_follow | 4 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 5 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 6 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 7 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | parallel_safe | 8 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 9 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 10 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 11 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 12 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 13 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 14 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 15 | log consumer migration |
| 3 | obs-d-18 | cobs/terra | must_follow | 16 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 17 | OTLP dual-path qualification |
| 4 | obs-d-11 | lobs/luna | must_follow | 18 | Python distribution guard |

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
| D.4.1 | d-12#2 |
| D.4.2 | d-12#2 |
| D.4.3 | d-4#1, d-14#1, d-14#2, d-15#1, d-15#2, d-16#1, d-17#1, d-18#1, d-18#5 |
| D.4.4 | d-12#6, d-18#3 |
| D.4.5 | d-12#1, d-18#4 |
| D.4.6 | d-13#4, d-18#1 |
| D.4.7 | d-12#6, d-18#3 |
| D.4.8 | d-18#4 |
| D.5.1 | d-12#3 |
| D.5.2 | d-5#1 |
| D.5.3 | d-5#2 |
| D.5.4 | d-18#4 |
| D.5.5 | d-12#3, d-14#1, d-15#1, d-18#5 |
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
| D.10.3 | d-10#3, d-11#3 |
| D.10.4 | d-10#4, d-18#3 |
| D.11.1 | d-11#1 |
| D.11.2 | d-11#2 |
| D.11.3 | d-11#3 |
| D.11.4 | d-11#4 |


## Current deliverable additions

Items not in the original 56 (measured from the numbered deliverables, 2026-09-26): D.4#2, D.14#3, D.15#3, D.16#2 and D.17#2 retype the local tests of each migration boundary; D.16#3 and D.17#3 update that sprint's own documentation. D.12#4–6 (contracts, registrations, version ownership) and D.13#4 (staged compatibility retirement) are hoisted 1.x items and appear in the mapping above.; D.18#1–5 close canonical activation, composition, release gates and language integration. Per-sprint documentation is supporting evidence, not a separate dev gate.

Relation counts: root=3, parallel_safe=6, must_follow=9.

Actual dev prerequisite edges: must_follow=21, parallel_safe=7; plan-review and dev-to-sanity bookkeeping edges are excluded. Critical path verified from blocker edges: 4. No merge-order edge is counted.

## Acceptance criteria

- [ ] every sprint of the phase closed
- [ ] every finding closed with a close reason
- [ ] phase PR integrate/phase-d -> develop merged
