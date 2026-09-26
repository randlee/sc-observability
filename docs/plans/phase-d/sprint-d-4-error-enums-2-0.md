# d-4: 2.0 discriminated error enum migration (#92)

## Plan metadata

- Wave: 7
- Branch: `sprint/d-4-error-enums-2-0`
- PR target: `sprint/d-3-typed-sink-registration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability/src/health.rs`
  - `crates/sc-observability/src/lib.rs`
  - `crates/sc-observability/tests/error_migration.rs`
  - `crates/sc-observability/tests/logging_only.rs`
  - `docs/plans/phase-d/sprint-d-4-error-enums-2-0.md`

## Goal

Migrate core facade and health construction/handling sites to obs-d-12
canonical errors (ADR-017).

## Deliverables

1. [REQ: LOG-044, LOG-045, LOG-046, PHD-001] Migrate construction and handling
   call sites in `lib.rs` and `health.rs` to types-owned named variants using
   the single cause mapping. `sinks.rs`, including all eleven error-type call
   sites, belongs to D.3. Preserve transitional wrappers, classification, and
   adapters for D.18 to retire.

2. [REQ: LOG-044, LOG-045, LOG-046, NFR-012, PHD-001] Retype `logging_only.rs`
   and focused `error_migration.rs` tests to assert variant, stable diagnostic,
   remediation, and source identity for the migrated core facade/health paths.

## This Sprint Does Not Close

Runtime constructions are D.1; builder constructions and the entire
`sinks.rs` migration are D.3; `typed.rs` and `settings.rs` are D.13; D.12 owns
canonical definitions, registry constants, OTLP configuration, and the 2.0
version bump. D.18 alone removes 1.x wrappers, classification, and adapters,
and closes release/API evidence and migration docs.


## Design

## Construction-site disposition

Core `lib.rs` and `health.rs` construction/handling call sites are this bead's
implementation. D.3 exclusively owns the complete `sinks.rs` error migration,
including existing `LogSink` implementors and all eleven error-type call sites;
D.4 does not edit or hand off that file. `runtime.rs` Logger
creation/admission/flush construction is D.1; `builder.rs` registration/build
errors are D.3; `typed.rs` adapter/classification and `settings.rs` resolution
errors are D.13. All use the same obs-d-12 cause-to-variant table (ADR-017).
The types-crate definition survives; core re-exports it. D.4 neither removes
transitional wrappers/classification/adapters nor edits a manifest, registry,
requirement, or release document. metadata.owned_paths is the sole fence.

The only file fence is metadata.owned_paths; paths mentioned as dependencies
are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-4, owned by obs-d-18 from wave 3; after this bead closes
it makes no further edits. The receiver owns compatibility retirement and
production completion.

- `crates/sc-observability/src/health.rs`
- `crates/sc-observability/src/lib.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observability --test logging_only --test error_migration
  --locked` passes canonical variant/code/source tests for the two deliverables.
- [ ] boundary:sc-observability — migrated `lib.rs` and `health.rs`
  construction/handling sites use the canonical cause mapping; tests distinguish
  shutdown Timeout/Drain and health diagnostic/source preservation (D1/D2).
- [ ] This sprint does not close release version/doc agreement, 1.x consumer
  qualification, compatibility retirement, D.3's `sinks.rs` migration, or
  changes in another sprint's construction files; those owners and obs-d-18
  close them.
