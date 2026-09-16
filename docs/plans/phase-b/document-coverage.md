---
status: proposed
---

# Phase B required-document coverage

This is a navigation map, not a replacement checklist. Each sprint's authoritative
lists define its work and closure. Future handoffs and generated schemas are
execution outputs, not existing implementation evidence.

## Required documents

| Category | Existing planning authority | Execution responsibility |
| --- | --- | --- |
| Project and phase | [Project plan](../../project-plan.md), [phase index](plan-phase-b.md) | All sprint owners maintain their own evidence |
| Sprint plans | The 18 sprint records linked from the phase index | One authoritative deliverable/acceptance/validation list per record |
| Requirements | [Requirements](../../requirements.md), especially PHB-001–014 | Owning sprint updates implementation status only after evidence |
| Architecture and ADRs | [Architecture](../../architecture.md), ADR index in §7 | ADR-011–015 stay proposed until accepted |
| Crate requirements/architecture | Per-crate table below | Each listed owner verifies additions against published compatibility |
| Interfaces/protocols | [Runtime](runtime-level-contract.md), [bridge](target-bridge-api.md), [errors](error-api-contract.md), [bindings](binding-contract.md) | Signatures and all corner-case fixtures belong to their assigned sprint |
| Machine-readable boundaries | [Boundary manifest](boundaries.json) | Planned contracts and owners now; generated schema-v1 artifacts in B.3, runtime consumers in B.3a/B.4 |
| Known issue disposition | [Issues inventory](issues-inventory.md) | Scope disposition only; execution evidence closes work, not a planning edit |
| Testing/platforms | [Test strategy](../../test-strategy.md), [platform guidelines](../../cross-platform-guidelines.md), sprint validation lists | Core/bridge desktop matrices and B.4a Python matrix are explicit gates |
| Process/QA/triage | [Team protocol](../../team-protocol.md), [planning guidelines](../../../.claude/skills/plan-hardening/sprint-planning-guidelines.md) | No Phase B change to ATM workflow or QA routing is planned |

## Crate and package coverage

Central requirements and architecture contain the four published crates' per-crate
sections; separate duplicate files are not needed. PHB requirements and the linked
contracts define the new companions and adapters at the same level of authority.

| Crate/package | Requirements | Architecture/interface | Implementation owner |
| --- | --- | --- | --- |
| sc-observability-types | Requirements §3, PHB-003/004/007/008 | Architecture §3.1, ADR-012/013; error/runtime contracts | B.P1 runtime values; B.1a classified failures/neutral traits |
| sc-observability | Requirements §4, PHB-003/007–009 | Architecture §3.2, ADR-010/012/013; runtime/error contracts | B.P1 level state; B.1b typed logger/sink operations |
| sc-observe | Requirements §5, PHB-003/004 | Architecture §3.3, ADR-012; error contract | B.1c observation integration |
| sc-observability-otlp | Requirements §6, PHB-003/004 | Architecture §3.4, ADR-012; error contract | B.1d telemetry integration |
| sc-observability-log | PHB-001/002/007–011 | ADR-011/013; target bridge and runtime contracts | B.P3 BTIT implementation, B.1 mechanical copy; no duplicate redesign |
| sc-observability-log-macros | PHB-001/002/011 | ADR-011; target bridge macro inventory | B.P3 BTIT implementation, B.1 mechanical copy |
| sc-observability-log-consumer-check | PHB-001/014 | ADR-011; B.1 consumer manifest/signatures | B.1 unpublished consumer fixture |
| sc-observability-dto | PHB-002/010/012/013 | ADR-014; binding contract and B.3 shared signatures | B.3 neutral wire types/conversions |
| sc-observability-binding-runtime | PHB-002/010–013 | ADR-011/014/015; [native contract](native-binding-runtime.md) | B.3b shared core/bridge backends and coordinator |
| TypeScript client/generator and Tauri adapter | PHB-010–012 | ADR-014; B.3a client and command signatures | B.3a client, host adapter and real IPC example |
| Python extension and Rust embedding rlib | PHB-010/011/013 | ADR-014/015; B.4 host/lifecycle signatures | B.4 runtime; B.4a platform distributions; B.5 integration; B.6 optional async |

B.1e alone owns warning activation and adoption guidance; B.P2, B.2 and B.7 own
separate immutable release baselines. Reusing earlier fixtures at a release gate
does not assign their implementation to the release sprint again.
