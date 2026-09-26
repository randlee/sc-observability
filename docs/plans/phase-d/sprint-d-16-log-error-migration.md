# d-16: Log error migration

## Plan metadata

- Wave: 14
- Branch: `sprint/d-16-log-error-migration`
- PR target: `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
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

1. Migrate `IdentityError`, `InitError`, `FlushError`, and `ShutdownError` uses in the named sc-observability-log modules to obs-d-12 (ADR-017) named variants.
2. Retype the five named log-crate regression tests to assert the obs-d-12 (ADR-017) variant, stable code, and `ErrorContext` source identity.
3. Update the log error-migration sprint documentation.

## This Sprint Does Not Close

The bridge API is D2, the typed sink signature is obs-d-13 (ADR-017), consumer/examples are D17, and workspace integration is D18.
Update the sprint doc as explanatory evidence alongside code; documentation is not a separate closure gate.


## Design

## Log migration recipe

Consume obs-d-12's canonical cause mapping (ADR-017), removing same-name IdentityError/InitError/FlushError/ShutdownError copies from error.rs and re-exporting the surviving types definitions. PHB-002 still permits companion-only DetachError from obs-d-13; never remove that distinct boundary. control.rs/handle.rs/mapping.rs convert per cause: validation -> InitError::Configuration, startup -> Runtime, flush -> Drain, shutdown deadline -> Timeout and other shutdown failure -> Drain. Preserve stable codes and source identity through mapping; registry constants are D.12-owned. Retype exactly api_freeze, flush_single_flight, init_runtime_start, shutdown_timeout and static_level_cap tests. Bridge fixtures belong to D.2, not this bead.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-16, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log/src/control.rs`
- `crates/sc-observability-log/src/error.rs`
- `crates/sc-observability-log/src/handle.rs`
- `crates/sc-observability-log/src/mapping.rs`
- `crates/sc-observability-log/tests/api_freeze.rs`
- `crates/sc-observability-log/tests/flush_single_flight.rs`
- `crates/sc-observability-log/tests/init_runtime_start.rs`
- `crates/sc-observability-log/tests/shutdown_timeout.rs`
- `crates/sc-observability-log/tests/static_level_cap.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observability-log --test api_freeze --test flush_single_flight --test init_runtime_start --test shutdown_timeout --test static_level_cap --locked` asserts the canonical variants and source identity (D1/D2).
- [ ] error.rs imports canonical v2 definitions without duplicate v2 enums; the explicitly staged 1.x compatibility is retired by D.18, and the four owned implementation modules construct/import canonical errors rather than tuple wrappers (D1).
- [ ] Sprint doc explains the completed cause mapping; it is supporting evidence, not an independent gate (D3). This sprint does not close bridge behavior, consumers or release; D.2/D.17/D.18 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

