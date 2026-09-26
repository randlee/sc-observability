# d-4: 2.0 discriminated error enum migration (#92)

Generated projection of `obs-d-4`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 7
- Assignee / model: lobs / luna
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: sc-observability error migration
- Branch: `sprint/d-4-error-enums-2-0`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-4-error-enums-2-0`
- PR target (merge order only): `sprint/d-3-typed-sink-registration`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-003, LOG-004, LOG-007, LOG-010, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, NFR-012, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-011, ADR-012, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observability/src/health.rs`
  - `crates/sc-observability/src/lib.rs`
  - `crates/sc-observability/src/sinks.rs`
  - `crates/sc-observability/tests/error_migration.rs`
  - `crates/sc-observability/tests/logging_only.rs`
  - `docs/plans/phase-d/sprint-d-4-error-enums-2-0.md`

## Goal

Migrate the core facade and sink implementation to obs-d-12 canonical errors (ADR-017).

## Deliverables

1. Replace local wrapper definitions/construction in lib.rs, sinks.rs and health.rs with re-exports/named variants of the surviving sc-observability-types definitions. Use the one cause mapping; remove the duplicate core enums rather than retaining a second same-name type.

2. Retype logging_only.rs and focused error_migration.rs tests to assert variant, stable diagnostic, remediation and source identity for core sink/facade paths.

## This Sprint Does Not Close

runtime.rs constructions are migrated by D.1, builder.rs by D.3, typed.rs/settings.rs by D.13; D.12 owns registry constants and definitions. D.18 closes workspace removal, release/API evidence and migration docs. These are replacements for 2.0, not coexisting legacy adapters.

## Design

## Construction-site disposition

Core lib.rs/sinks.rs/health.rs are this bead's implementation. runtime.rs Logger creation/admission/flush construction is D.1; builder.rs registration/build errors are D.3; typed.rs adapter/classification and settings.rs resolution errors are D.13. All use the same obs-d-12 cause-to-variant table (ADR-017). The types-crate definition survives; core re-exports it. No D.4 edit to a manifest, registry, requirement or release document. metadata.owned_paths is the sole fence.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-4, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability/src/health.rs`
- `crates/sc-observability/src/lib.rs`
- `crates/sc-observability/src/sinks.rs`

## Acceptance criteria

- [ ] `cargo test -p sc-observability --test logging_only --test error_migration --locked` passes canonical variant/code/source tests for the two deliverables.
- [ ] boundary:sc-observability — lib.rs, sinks.rs and health.rs contain no opaque wrapper constructors or duplicate shared error definitions on the migrated v2 path; tests distinguish sink Write/Flush and shutdown Timeout/Drain.
- [ ] This sprint does not close release version/doc agreement, 1.x consumer qualification or changes in another sprint's construction files; those owners and obs-d-18 close them.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
