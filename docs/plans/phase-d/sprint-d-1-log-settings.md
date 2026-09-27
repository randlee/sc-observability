# d-1: Shared startup `LogSettings` (#96)

Generated projection of `obs-d-1`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 5
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability logging settings implementation
- Branch: `sprint/d-1-log-settings`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-1-log-settings`
- PR target (merge order only): `sprint/d-10-windows-arm64-wheel`
- Blocked by: `obs-d-13-sanity`
- Requirements: LOG-001, LOG-002, LOG-003, LOG-004, LOG-005, LOG-006, LOG-007, LOG-008, LOG-009, LOG-010, LOG-039, LOG-042, PHB-014, NFR-012, PHD-001, PHD-002
- ADRs: ADR-002, ADR-003, ADR-005, ADR-006, ADR-009, ADR-010, ADR-013, ADR-014, ADR-017, ADR-019
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

1. [REQ: LOG-001, LOG-002, LOG-003, LOG-004, LOG-005, LOG-009, LOG-039, LOG-042] Implement deterministic environment parsing for the complete inventory and
   field-wise resolution in the documented order including the LOG-009 root
   exception. Parsing uses a named `EnvSnapshot` so one resolution cannot mix
   process states.

2. [REQ: LOG-006, LOG-007, LOG-008, LOG-010, NFR-012] Convert the resolved value to `LoggerConfig` and its strong policy types,
   preserving all non-inventory defaults and introducing no post-construction
   mutation.

3. [REQ: PHB-014] Document the table, precedence, null/unset behavior, prefix rules, failure
   codes, and startup-only lifecycle. Add a public example embedding settings
   under an application's `logging` JSON key. Document the compact stable-error
   table: `PrefixCollision`/`SC_LOG_SETTINGS_PREFIX_COLLISION`,
   `InvalidEnvironment`/`SC_LOG_SETTINGS_INVALID_ENVIRONMENT`,
   `UnknownKey`/`SC_LOG_SETTINGS_UNKNOWN_KEY`,
   `InvalidValue`/`SC_LOG_SETTINGS_INVALID_VALUE`, and
   `Resolution`/`SC_LOG_SETTINGS_RESOLUTION`.


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

## Handoff from obs-d-12 and obs-d-13 (wave 1)

Consume obs-d-13's frozen concrete settings signature specification and obs-d-12's canonical error/registry artifact. In wave 2, bind the resulting errors and codes only in owned `runtime.rs`; do not alter either producer contract.

## Diagnostic-code naming

The five `LogSettingsError` diagnostics are registry-style names, not requirement ids: `SC_LOG_SETTINGS_PREFIX_COLLISION`, `SC_LOG_SETTINGS_INVALID_ENVIRONMENT`, `SC_LOG_SETTINGS_UNKNOWN_KEY`, `SC_LOG_SETTINGS_INVALID_VALUE`, and `SC_LOG_SETTINGS_RESOLUTION`.
## Acceptance criteria

- [ ] boundary:sc-observability — `cargo test -p sc-observability --test log_settings --locked` runs defaults/JSON/shared-env/app-env/precedence for every D.13 inventory row, invalid/empty/unknown/case/non-UTF8 prefix cases, and atomic retained-policy replacement (D1).
- [ ] boundary:sc-observability — config conversion parity preserves unrelated LoggerConfig defaults, validates LogRoot and performs no post-construction mutation; runtime.rs uses canonical 2.0 errors and preserves source (D2).
- [ ] `cargo check --manifest-path examples/log-settings/Cargo.toml --locked` passes the 2.0 public settings example and docs/logging/d-1-log-settings.md matches the contract table and the named SC_LOG_SETTINGS_* diagnostics (D3).
- [ ] This sprint does not close combined logging feature/release/API behavior; obs-d-18 does.
