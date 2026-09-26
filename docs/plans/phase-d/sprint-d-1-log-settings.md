# d-1: Shared startup `LogSettings` (#96)

Generated projection of `obs-d-1`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 4
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability logging settings implementation
- Branch: `sprint/d-1-log-settings`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-1-log-settings`
- PR target (merge order only): `sprint/d-10-windows-arm64-wheel`
- Blocked by: `obs-d-13-sanity`
- Requirements: DOC-003, DOC-004, LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-002, LOG-003, LOG-004, LOG-005, LOG-006, LOG-007, LOG-008, LOG-009, LOG-010, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-020, LOG-021, LOG-023, LOG-037, LOG-038, LOG-040, LOG-043, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-025, TYP-026, TYP-027, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-011, ADR-012, ADR-013, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observability/src/runtime.rs`
  - `crates/sc-observability/tests/log_settings.rs`
  - `docs/logging/d-1-log-settings.md`
  - `docs/plans/phase-d/sprint-d-1-log-settings.md`
  - `examples/log-settings/src/**`

## Goal and dependency

Create the single serde-stable, binding-friendly configuration value in
`sc-observability` that applications resolve before constructing a `Logger`.
It is parallel-safe with D.2 and D.3 because neither consumes this type. This is a 2.0 implementation against the obs-d-13 contract; D.18 owns the published 1.4.1-to-2.0 comparison.


## Deliverables

1. Implement deterministic environment parsing for the complete inventory and
   field-wise resolution in the documented order including the LOG-009 root
   exception. Parsing uses a named `EnvSnapshot` so one resolution cannot mix
   process states.

2. Convert the resolved value to `LoggerConfig` and its strong policy types,
   preserving all non-inventory defaults and introducing no post-construction
   mutation.

3. Document the table, precedence, null/unset behavior, prefix rules, failure
   codes, and startup-only lifecycle. Add a public example embedding settings
   under an application's `logging` JSON key. Document the compact stable-error
   table: `PrefixCollision`/`LOG-001`, `InvalidEnvironment`/`LOG-002`,
   `UnknownKey`/`LOG-003`, `InvalidValue`/`LOG-004`, and
   `Resolution`/`LOG-005`.


## This Sprint Does Not Close

Do not migrate consumer applications, add fields outside the inventory,
implement dynamic reload, or plan #88 bindings/OTEL work.

## Design

## Implementation contract

Consume obs-d-13 Settings contract and Authoritative field inventory (ADR-017/PHB-002). runtime.rs owns environment parsing/resolution and LoggerConfig conversion; settings.rs is D.13's declaration/serde contract. Implement defaults, field overlays, atomic retained policy, selected-prefix validation, one EnvSnapshot, and the explicit JSON logRoot exception exactly as D.13 specifies. Migrate existing runtime.rs error constructors to obs-d-12's cause mapping while editing those paths. Document/example source belongs here; example Cargo registration is D.12. No normative/API/release files are edited here.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-13 (wave 1)

Created by obs-d-13, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability/src/runtime.rs`

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-1, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability/src/runtime.rs`

## Acceptance criteria

- [ ] boundary:sc-observability — `cargo test -p sc-observability --test log_settings --locked` runs defaults/JSON/shared-env/app-env/precedence for every D.13 inventory row, invalid/empty/unknown/case/non-UTF8 prefix cases, and atomic retained-policy replacement (D1).
- [ ] boundary:sc-observability — config conversion parity preserves unrelated LoggerConfig defaults, validates LogRoot and performs no post-construction mutation; runtime.rs uses canonical 2.0 errors and preserves source (D2).
- [ ] `cargo check --manifest-path examples/log-settings/Cargo.toml --locked` passes the 2.0 public settings example and docs/logging/d-1-log-settings.md matches the contract table and LOG-001..005 diagnostics (D3).
- [ ] This sprint does not close combined logging feature/release/API behavior; obs-d-18 does.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
