# d-3: Typed sink registration ergonomics (#203)

Generated projection of `obs-d-3`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 6
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability typed sink module
- Branch: `sprint/d-3-typed-sink-registration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-3-typed-sink-registration`
- PR target (merge order only): `sprint/d-2-host-logger-bridge`
- Blocked by: `obs-d-13-sanity`
- Requirements: DOC-003, LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-003, LOG-004, LOG-007, LOG-010, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-011, ADR-012, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observability/src/builder.rs`
  - `crates/sc-observability/tests/typed_registration.rs`
  - `docs/logging/d-3-typed-sink-registration.md`
  - `docs/plans/phase-d/sprint-d-3-typed-sink-registration.md`

## Goal

Implement canonical 2.0 typed sink registration using obs-d-13 contracts.

## Deliverables

1. Implement SinkRegistration::typed in builder.rs against the canonical open sink contract, preserving registration metadata and avoiding legacy_sink/typed_sink compatibility adapters.

2. Implement LoggerBuilder::register_typed_sink with chaining, typed registration errors, one write/flush per operation and preserved sink health/source diagnostics.

3. Update docs/logging/d-3-typed-sink-registration.md and typed_registration.rs consumer fixtures for canonical 2.0 registration; migrate builder.rs construction sites using obs-d-12's cause mapping.

## This Sprint Does Not Close

D.13 owns staged typed.rs definitions; D.18 removes transitional adapters; D.4 owns core facade/sink migration; D.18 owns public approvals, migration guides and full logging integration.

## Design

## Implementation contract

Consume obs-d-13 Typed sink signatures and ownership decisions. Both inherent registration implementations live in builder.rs; do not edit typed.rs or invent an adapter back to opaque 1.x errors. Existing custom LogSink implementations migrate to the same canonical 2.0 error signature. No duplicate headings or alternate file fence.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-13 (wave 1)

Created by obs-d-13, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability/src/builder.rs`

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-3, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability/src/builder.rs`

## Acceptance criteria

- [ ] `cargo test -p sc-observability --test typed_registration --locked` runs both entry points with Arc<dyn TypedLogSink> and no legacy adapter/import/deprecated usage (D1/D3).
- [ ] boundary:sc-observability — registration metadata, chaining, duplicate/invalid/closed failure, one write/flush, health and source identity are asserted against the D.13 contract (D2).
- [ ] This sprint does not close 1.4.1-to-2.0 release migration; obs-d-18 owns that gate.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
