# B.1b typed logger preparation handoff

## Implementation

The logger preparation layer adds neutral `LogFailure` and `TryLogFailure`,
typed startup, admission, and flush methods, plus `typed::TypedLogSink` and
explicit legacy/typed adapters. JSONL and console sinks run their typed write
implementation once; their retained `LogSink` implementations only convert the
returned error. Validation, unavailable-level admission, and writer flush now
produce neutral failures at their production sites, while legacy facade methods
perform compatibility conversion.

The retained `emit` path, owner constructors, query/follow/health/shutdown
surfaces, admission/filtering semantics, and bridge API are unchanged.

## Parent and revisions

- Required provenance parent merged: `8aeabb584b3b2d004606591ff80741867ebb9ca7`
- Merge-forward revision: `44b3f3c5d06e3ab4195984f8141777be5726ae6d`
- Typed production-site correction: `2d1f207c24268d618f1fda135484cde88f079706`
- Final parent-merged validation base: `453f9e8` (includes provenance
  completeness PASS at `d6571c1`)
- Final C03 scenario-evidence revision: `211899ec5c61ff80983c0b730605bbb2a318e9fa`

## Validation

- PASS: `cargo test --locked -p sc-observability --all-targets`
- PASS: `cargo test --locked -p sc-observability --all-targets --all-features`
  (79 unit tests and 2 logging-only consumer tests at the final parent-merged
  C03 tip)
- PASS: `cargo fmt --all -- --check`
- PASS: `cargo clippy --locked -p sc-observability --all-targets --all-features -- -D warnings`
- PASS: `cargo test --locked -p sc-observability --doc`
- PASS: `cargo test --locked --workspace --doc`
- PASS: `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- PASS: `python3 scripts/ci/validate_public_api_semver.py` (223 checks passed
  for each of the four workspace crates)
- PASS: `bash scripts/ci/validate_public_api_docs.sh`
- REVIEWED: `bash scripts/ci/validate_public_api_diff.sh` generated the expected
  nonzero additive-diff report against `1.2.0`; it includes the stacked B.P1/
  B.1a additions and this layer's typed logger API. It is evidence for public
  API review, not a claim that an additive diff is empty.

The task-plan fixture matrix records the paired legacy/typed and retained
behavior coverage. The copied bridge regression is pending B.1 source
integration and is intentionally not represented as a completed local run.

The final C03 fixtures cover the failure-injectable `ConsoleSink::from_writer`
seam in both legacy and typed forms, preserving the stable write code, native
source rendering, degraded health summary, and exactly one physical write per
call. The existing held-level-control synchronization fixture now exercises
both legacy and typed admission. Built-in JSONL and console flush uses the
inherited `TypedLogSink` default no-op; custom adapter fixtures remain the
explicit-flush failure evidence.

## Startup ordering and rollback boundary

`LoggerBuilder::build_inner` calls `LoggerRuntime::try_new` before creating the
`LevelControl` or returning a `Logger`/`LevelOwner`. `WriterRuntime::try_new`
allocates the channel and starts its only worker as its final fallible operation;
if that spawn fails, no worker exists to roll back and the partially allocated
channel/tracker values are dropped. The builder returns `InitFailure` before it
can construct a logger or owner. The paired injected-start tests verify the
legacy and typed paths retain the native source and return no owner. There is
no coordinator or secondary worker stage in this runtime, so no invented
partial-start rollback path is claimed.

## Remaining integration boundary

This is scoped B.1b preparation only. B.1 accepted bridge integration,
observation/telemetry adoption, warning activation, publication, and phase
closure remain owned by their respective later layers.

## Integration-layer status addendum (feature/phase-b-1-integration)

Implementation-complete, confirmed against merged source: `InitError` and
`EventError` are the two B.1b-owned families (`LogFailure`/`TryLogFailure`
compose `EventFailure`); both have registry-parity tests passing
(`error_registry_parity.rs::init_failure_matches_owning_registry`,
`::event_failure_matches_owning_registry`). B.1e's warning activation (noted
as pending in this handoff's "remaining integration boundary") landed on
`feature/phase-b-1e-migration-validation` and is merged into the integration
branch; `cargo clippy --workspace --all-targets --all-features -- -D warnings`
passes across all active crates as a result.

The copied bridge regression called out as "pending B.1 source integration"
above is now identified precisely: the frozen bridge uses `InitError` via
`sc_observability::Logger::new` (`handle.rs:903`) and `EventError` via
`Logger::try_log_with_outcome`'s `TryLogError` wrapper (`control.rs:111`,
`handle.rs:504`). Both are recorded as explicit disposition rows in
`error-api-inventory.md`. lobs has landed narrow per-call-site
`#[allow(deprecated, reason = ...)]` annotations covering these sites, but the
`import-provenance.json` adaptation-kind extension needed to keep the
immutable import manifest documented (rather than silently drifting) has not
yet landed — this is reported and tracked by the integration layer, not
resolved as of this addendum. Independent QA/coordinator completeness PASS
remains pending for both this preparation layer and the integration layer.
