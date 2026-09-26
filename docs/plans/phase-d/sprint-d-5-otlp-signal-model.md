# d-5: OTLP 2.0 signal model

Generated projection of `obs-d-5`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 8
- Assignee / model: cobs / terra
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: OTLP signal projectors
- Branch: `sprint/d-5-otlp-signal-model`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-5-otlp-signal-model`
- PR target (merge order only): `sprint/d-4-error-enums-2-0`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-004, LAY-005, LAY-006, NFR-001, NFR-004, NFR-005, NFR-006, NFR-007, NFR-009, OTLP-001, OTLP-002, OTLP-003, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-012, OTLP-013, OTLP-014, OTLP-015, OTLP-016, OTLP-017, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-021, TYP-023, TYP-024, TYP-030, TYP-031
- ADRs: ADR-001, ADR-002, ADR-004, ADR-005, ADR-009, ADR-012, ADR-017, ADR-018
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp/src/assembly.rs`
  - `crates/sc-observability-otlp/src/projectors.rs`
  - `crates/sc-observability-otlp/tests/error_registry_parity.rs`
  - `docs/plans/phase-d/sprint-d-5-otlp-signal-model.md`

## Goal

Implement OTLP signal projection and assembly against obs-d-12 neutral signal contracts; no lower-crate or binding migration is owned here.

## Deliverables

1. Migrate projectors.rs and assembly.rs to preserve span kind/flags/links/events/status/timing/resource/scope and the completed-span invariant.

2. Project MetricValue gauge/sum/histogram without scalar placeholders; preserve temporality/start time and consume the D.12 validated HistogramPoint/MetricModelError contract.

3. Add focused assembly/projector cases and error_registry_parity.rs assertions for invalid histogram, temporality and interval failures using D.12 stable codes.

## This Sprint Does Not Close

Neutral type definitions/serde are D.12; sc-observe consumers D.14; binding-runtime D.15; DTO/language conversion and migration docs D.18; actual collector equivalence D.9.

## Design

## Implementation contract

Use obs-d-12 Public contract and MetricModelError mapping (ADR-017/018). This boundary is OTLP projectors/SpanAssembler only. Histogram invariants are validated by types constructors; projection preserves every bucket and finite sum instead of duplicating validation or synthesizing a one-bucket value. Assembly drops unfinished spans only at the contracted final flush/shutdown. No shared docs, manifests, SDK or binding edits.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-5, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/assembly.rs`
- `crates/sc-observability-otlp/src/projectors.rs`

## Acceptance criteria

- [ ] `cargo test -p sc-observability-otlp --lib assembly --locked` and `cargo test -p sc-observability-otlp --lib projectors --locked` run nonzero tests for the D1/D2 cases.
- [ ] `cargo test -p sc-observability-otlp --test error_registry_parity --locked` checks model failure code/source mapping and malformed inputs (D3).
- [ ] This sprint does not close neutral serde, other-crate consumers, release evidence or collector wire equivalence; the named owners above do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
