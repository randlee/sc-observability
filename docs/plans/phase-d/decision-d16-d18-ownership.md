# Proposed D16 to D18 ownership amendment

## Status

**Proposed — pending review of `obs-decision-d16-d18-ownership`.** This document is a planning decision record only. It makes no live Beads change, does not close `obs-d-16` or `obs-d-16-sanity`, and does not claim D16 passed.

## Decision

Adopt option A: retire D16 as an active planned sprint and make D18 the sole owner of D16's unfinished canonical logging-error migration. D18 retains its existing real prerequisites; only the obsolete D16 sanity dependency is removed from the proposed plan. D12 supplies the canonical types contract and D2 supplies the bridge artifact; neither is waived or duplicated.

## Before/after mapping

| Before (unfinished D16 scope) | After (proposed D18 owner) |
| --- | --- |
| D16 #1: migrate `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` in `control.rs`, `error.rs`, `handle.rs`, and `mapping.rs` to D12 named variants | D18 #1, absorbed canonical log-error migration sub-deliverable |
| D16 #2: retype the five log regression tests | D18 #1, absorbed regression-test sub-deliverable |
| `api_freeze`, `flush_single_flight`, `init_runtime_start`, `shutdown_timeout`, `static_level_cap` | D18 #1 validation: canonical variant, stable code, and `ErrorContext` source identity |
| LAY-001, LAY-002, LAY-006, LAY-007; LOG-001, LOG-003, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-046, LOG-047, LOG-048 | D18 #1; listed in D18 requirements metadata |
| NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009; PHB-002, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011; PHD-001, PHD-002 | D18 #1; listed in D18 requirements metadata |
| SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006; TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039 | D18 #1; listed in D18 requirements metadata |
| ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-014, ADR-017, ADR-019 | D18 design/validation; listed in D18 ADR metadata |

The exact five-test command is retained under D18 acceptance criteria. D18 additionally retains the workspace/all-features, public API/semver, release, and integration validation gates already assigned to it.

## Proposed plan and Beads updates

1. Remove the `d-16` row from the proposed `sprints.jsonl` and remove `d-16` from D18's proposed prerequisite tuple. All other D18 prerequisites remain unchanged.
2. Convert the D16 plan page to a historical absorption disposition and add the D18 deliverable, requirements, ADR ties, tests, and acceptance criterion above.
3. If approved, update the live `obs-d-16`, `obs-d-16-sanity`, and `obs-d-18` Beads records atomically to match this decision. This amendment does **not** perform those live updates, close either D16 bead, or mutate a live dependency.

## Constraints preserved

The public compatibility surface remains until D18's planned activation. D18 must not add a temporary duplicate adapter, must preserve stable codes and source identity, and must keep the `DetachError` companion exception scoped to its existing boundary. The proposed change is documentation/planning only; it authorizes no runtime or policy work.
