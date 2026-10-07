# d-16: Historical absorbed log-error migration

## Status

This is a historical disposition record, not an active sprint projection. The pending `obs-decision-d16-d18-ownership` option A absorbs D16's unfinished canonical logging-error work into `obs-d-18`, which is the maintenance owner of this historical plan file. It does **not** assert an `obs-d-16` pass, close its bead or sanity bead, waive any requirement, or change the live Beads dependency graph.

## Original D16 scope, now proposed for D18 ownership

1. Migrate `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` uses in `control.rs`, `error.rs`, `handle.rs`, and `mapping.rs` to D12 canonical named variants.
2. Retype `api_freeze`, `flush_single_flight`, `init_runtime_start`, `shutdown_timeout`, and `static_level_cap` to assert the canonical variant, stable code, and `ErrorContext` source identity.

The former requirements are retained by D18: LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-003, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-046, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, PHB-002, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, PHD-001, PHD-002, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, and TYP-039. The retained ADR ties are ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-014, ADR-017, and ADR-019.

## Preserved boundary and validation

D18 consumes D12's canonical v2 error enums and `ErrorContext`; `error.rs` and the four implementation modules import and construct those canonical definitions directly, with no temporary tuple wrappers or parallel classifier. It preserves the distinct companion-only `DetachError` boundary, stable error codes, source identity, and the cause mapping `Configuration`/`Runtime`/`Drain`/`Timeout`. D2 remains the bridge artifact and D12 remains the canonical types source; their actual D18 prerequisites are preserved. D18 must run the five named log tests, which assert the detailed typed cause mapping, stable codes, and source identity, plus its workspace/all-features, public API, semver, and release gates. No temporary duplicate adapter is permitted.

See [the proposed ownership decision](decision-d16-d18-ownership.md) for the before/after mapping, proposed bead updates, and review status.
