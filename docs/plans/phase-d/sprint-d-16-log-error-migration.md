# d-16: Log error migration

Generated projection of `obs-d-16`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 15
- Assignee / model: cobs / terra
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: sc-observability-log error migration
- Branch: `sprint/d-16-log-error-migration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-16-log-error-migration`
- PR target (merge order only): `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-003, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-046, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, PHD-001, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-014, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observability-log/src/control.rs`
  - `crates/sc-observability-log/src/error.rs`
  - `crates/sc-observability-log/src/handle.rs`
  - `crates/sc-observability-log/src/mapping.rs`
  - `crates/sc-observability-log/tests/api_freeze.rs`
  - `crates/sc-observability-log/tests/flush_single_flight.rs`
  - `crates/sc-observability-log/tests/init_runtime_start.rs`
  - `crates/sc-observability-log/tests/shutdown_timeout.rs`
  - `crates/sc-observability-log/tests/static_level_cap.rs`
  - `docs/plans/phase-d/sprint-d-16-log-error-migration.md`

## Deliverables

1. Migrate `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` uses in the named `sc-observability-log` modules to `obs-d-12` (`ADR-017`, `PHD-001`) named variants.
2. Retype the five named log-crate regression tests to assert the canonical variant, stable code, and `ErrorContext` source identity.

## This Sprint Does Not Close

The bridge API is `obs-d-2`, the typed sink signature is `obs-d-13`, consumer/examples are `obs-d-17`, and workspace integration is `obs-d-18`.
The sprint document is supporting context and is not a separate closure gate.


## Design

## Log migration recipe

Consume `obs-d-12`'s canonical cause mapping and `ErrorContext` contract (ADR-017, PHD-001). Use the canonical types definitions for `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` in the owned modules. `PHB-002` still permits companion-only `DetachError` from `obs-d-13`; preserve that distinct boundary. `control.rs`/`handle.rs`/`mapping.rs` convert per cause: validation -> `InitError::Configuration`, startup -> `Runtime`, flush -> `Drain`, shutdown deadline -> `Timeout` and other shutdown failure -> `Drain`. Preserve stable codes and source identity through mapping; registry constants are D.12-owned. Retype exactly `api_freeze`, `flush_single_flight`, `init_runtime_start`, `shutdown_timeout`, and `static_level_cap` tests. Bridge fixtures belong to `obs-d-2`, not this bead.

## Canonical boundary and handoff

This bead retargets owned call sites and tests to the accepted ADR-017 surface. Any transitional compatibility needed by unfinished sibling consumers is limited to the existing boundary and is consumed by `obs-d-18`, which owns canonical activation and final compatibility retirement. No new legacy feature or duplicate classifier is introduced. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by `obs-d-16`, owned by `obs-d-18` from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion and final compatibility retirement.

- `crates/sc-observability-log/src/control.rs`
- `crates/sc-observability-log/src/error.rs`
- `crates/sc-observability-log/src/handle.rs`
- `crates/sc-observability-log/src/mapping.rs`
- `crates/sc-observability-log/tests/api_freeze.rs`
- `crates/sc-observability-log/tests/flush_single_flight.rs`
- `crates/sc-observability-log/tests/init_runtime_start.rs`
- `crates/sc-observability-log/tests/shutdown_timeout.rs`
- `crates/sc-observability-log/tests/static_level_cap.rs`

## Contract ties

- `LOG-046` governs the retained shutdown drain/timeout mapping.
- `ADR-014` governs preservation of typed operational results at downstream language boundaries.


## Acceptance criteria

- [ ] `cargo test -p sc-observability-log --test api_freeze --test flush_single_flight --test init_runtime_start --test shutdown_timeout --test static_level_cap --locked` asserts the canonical variants and source identity (`obs-d-16#1/#2`).
- [ ] `error.rs` imports canonical v2 definitions and the four owned implementation modules construct/import canonical errors rather than tuple wrappers (`obs-d-16#1`).
- [ ] This sprint does not close bridge behavior, consumers, canonical activation, or release; `obs-d-2`/`obs-d-17`/`obs-d-18` do. The sprint document is supporting context, not an independent gate.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; `obs-d-18` additionally runs all-features release tests and semver/removal gates.
