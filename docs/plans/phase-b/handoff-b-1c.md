---
id: B.1c-observation-prep-handoff
status: preparation-complete
branch: feature/phase-b-1c-observation-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1c-observation-prep
parent: feature/phase-b-1b-logger-prep
parent_checkpoint: 56a0908b6264624c43545ae40ff7e127bb2b98a9
implementation_commits: 7d49ed2, 37bba66
---

# B.1c typed observation preparation handoff

## Implementation inventory

The observation facade in `crates/sc-observe/src/lib.rs` now has one common
typed implementation with legacy result adapters:

| Contract method | Implementation and evidence |
| --- | --- |
| `ObservabilityConfig::default_for_typed` | Derives the existing env prefix and preserves `ObservationInitialization` diagnostics; `default_for` adapts its result. |
| `ObservabilityConfig::service_name_typed` | Validates the configured service name with the same diagnostic/source behavior; `service_name` adapts its result. |
| `Observability::new_typed` | Uses the existing builder/runtime and typed logger construction; `new` adapts its result. |
| `Observability::flush_typed` | Calls the existing logger `flush_typed` operation once; stopped runtimes remain successful. |
| `Observability::shutdown_typed` | Uses the existing shared shutdown flag and logger typestate transition; repeated and concurrent calls remain idempotent. |
| `ObservabilityBuilder::build_typed` | Retains the existing empty-route validation and constructs the runtime through `Logger::new_typed`; `build` adapts its result. |

The existing `register_subscriber`, `register_projection`, `emit`, health,
filter, ordering, aggregation, and `ObservationError` contracts are unchanged.
Typed subscriber and log/span/metric projector fixtures enter those unchanged
registrations through `legacy_subscriber`, `legacy_log_projector`,
`legacy_span_projector`, and `legacy_metric_projector`. No duplicate runtime or
placeholder logger API is present.

## Required parity inventory

The unit and external fixtures cover:

- valid and invalid construction paths, empty routes, and downstream logger
  initialization failure;
- typed/legacy construction classification, diagnostics, and source retention;
- eligible and ineligible filtering, deterministic registration ordering, no
  matching route, one successful route plus one failing route, and all routes
  failing;
- subscriber callbacks and log, span, and metric projector output families;
- top-level health aggregation, diagnostics, and telemetry health exposure;
- explicit flush, post-shutdown emit, flush after stop, repeated shutdown, and
  concurrent shutdown;
- real external typed-adapter registration in
  `tests/typed_observation.rs`, with exactly one invocation per route and
  concrete JSONL output assertions.

## Two-pass validation evidence

Contract/API completeness pass:

```text
required methods: all six present with typed Result failures
registration surface: unchanged existing register_subscriber/register_projection only
task doc and project plan: present with branch/worktree metadata and sprint entry
public API semver: passed; 223 checks passed and 30 skipped for each crate
public API docs: passed
```

Behavior/source-integrity pass:

```text
cargo fmt --all -- --check: passed
cargo test --locked --workspace: 168 unit/integration tests passed; 0 failed; 6 normal doctests passed; 2 compile-fail doctests passed
cargo clippy --locked --workspace --all-targets -- -D warnings: passed
python3 -m unittest discover -s scripts/ci/tests -p 'test_validate_log_import.py': 31 tests, OK
bash scripts/ci/validate_docs_consistency.sh: passed
bash scripts/ci/validate_dependency_bans.sh: passed
bash scripts/ci/validate_repo_boundaries.sh: passed
bash scripts/ci/validate_public_api_docs.sh: passed
bash scripts/ci/validate_public_api_diff.sh: exit 1 with additive API report, as expected for Phase B typed operations
```

The final parent merge-forward was `56a0908b6264624c43545ae40ff7e127bb2b98a9`;
the parent owns its logger/provenance changes and no lower layer was edited.
This is preparation evidence only: copied bridge source import, independent QA,
and full B.1c integration acceptance remain pending.
