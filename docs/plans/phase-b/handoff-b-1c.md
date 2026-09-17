---
id: B.1c-observation-prep-handoff
status: preparation-complete
branch: feature/phase-b-1c-observation-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1c-observation-prep
parent: feature/phase-b-1b-logger-prep
parent_checkpoint: 77d28c77f4d48b59e40f52b9ef735f68ebbb890e
implementation_commits: 7d49ed2, 37bba66, 392c1dd, d90fba2, 4327fd3, 5b25b20, 468ec1a, ba677c9
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

## C02 scenario-to-assertion matrix

| Scenario | Legacy assertion | Typed assertion | Evidence |
| --- | --- | --- | --- |
| Invalid names/config | Direct constructors reject empty names; deserialized invalid `ToolName` values make legacy and typed `default_for` fail with `SC_OBSERVE_INIT_FAILED` and the native env-prefix source, while invalid tool config makes both service-name paths fail with the native identifier source. | Same paired code, kind, and source checks. | `tests/typed_observation.rs:501` |
| Empty routes | `Observability::new` returns `InitFailureKind::ObservationInitialization` through the legacy result adapter. | `new_typed` returns the same kind and diagnostic code. | `tests/typed_observation.rs:571`; `src/lib.rs:1142` |
| Logger startup failure | Legacy builder path reports `LoggerInitialization` with the queue-capacity diagnostic. | Typed builder path reports the same kind, code, and diagnostic message. | `tests/typed_observation.rs:571`; `src/lib.rs:1142` |
| Eligible/ineligible filtering | Existing registration filter skips the ineligible observation, increments dropped count, and invokes no subscriber. | Paired typed fixture through `legacy_subscriber` has the same result. | `src/lib.rs:906`; `tests/typed_observation.rs:222` |
| Registration ordering | Two existing registrations invoke in insertion order. | The same typed adapters preserve `first, second` order. | `src/lib.rs:889`; `tests/typed_observation.rs:222` |
| No matching route | A different payload type returns `ObservationError::RoutingFailure`, dropped count 1, callback count 0. | Paired typed facade has the same error and counts. | `tests/typed_observation.rs:261` |
| Mixed failure | Failing plus successful registrations return `Ok`, count one subscriber failure, and deliver once to the successful route. | Same exact callback and health counts. | `src/lib.rs:936`; `tests/typed_observation.rs:261` |
| All failure | A failing route returns `RoutingFailure`, dropped count 1, failure count 1, and the routing diagnostic code. | Same exact result through typed construction and legacy registration. | `src/lib.rs:1017`; `tests/typed_observation.rs:261` |
| Diagnostic fidelity | Legacy adapter exposes custom and logger-family codes as unclassified without changing the context. | Round-trip typed adapter retains code and attached source. | `tests/typed_observation.rs:333`; neutral type-domain checks at `crates/sc-observability-types/src/typed.rs` |
| Output families | Existing registration invokes subscriber, log, span, and metric routes exactly once and emits concrete JSONL. | Typed implementations enter those registrations via the four B.1a adapters; paired fixture invokes each exactly once per facade. | `tests/typed_observation.rs:410`; `tests/typed_observation.rs:467` |
| Flush | Legacy `flush` returns the logger-flush classification and records one health failure for the operation. | `flush_typed` returns the same kind and health result; both paired fixtures wait boundedly for the writer’s second pass and assert exactly two sink flush calls. | `src/lib.rs:1198` |
| Shutdown/lifecycle | Legacy shutdown blocks later emit and flush-after-stop remains successful. | Typed shutdown is idempotent, including eight concurrent calls, and flush-after-stop remains successful. | `src/lib.rs:1034`; `src/lib.rs:1110`; `src/lib.rs:1170` |

The unchanged producer API is `emit` in both paths; there is intentionally no
invented `emit_typed` operation. Health and telemetry aggregation remain the
existing runtime contracts and are covered by `src/lib.rs:1053` and
`src/lib.rs:1075`.

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
cargo test --locked --workspace: historical preparation command only; this handoff does not retain a raw aggregate count or doctest total
cargo clippy --locked --workspace --all-targets -- -D warnings: passed
python3 -m unittest discover -s scripts/ci/tests -p 'test_validate_log_import.py': 41 tests, OK
bash scripts/ci/validate_docs_consistency.sh: passed
bash scripts/ci/validate_dependency_bans.sh: passed
bash scripts/ci/validate_repo_boundaries.sh: passed
bash scripts/ci/validate_public_api_docs.sh: passed
bash scripts/ci/validate_public_api_diff.sh: exit 1 with additive API report, as expected for Phase B typed operations
```

The final parent merge-forward is
`77d28c77f4d48b59e40f52b9ef735f68ebbb890e`; the parent owns its
logger/provenance changes and no lower layer was edited.
This is preparation evidence only: copied bridge source import, independent QA,
and full B.1c integration acceptance remain pending.

## QA1 observation fix-layer evidence

The five reconciled QA1 findings are implemented and verified on
`fix/phase-b-1c-qa1` at merged source
`fba711497b26bc9a0c7816d5352fdc8c0c4c64d3`. See
[`task-b-1c-qa1-fixes.md`](task-b-1c-qa1-fixes.md) for the two-pass
finding matrix, exact validation commands, raw log index and current counts.
The retained workspace run has 269 non-doc tests and 9 doctests passing,
with 7 ignored doctests; the explicit doctest rerun has 9 passing and 7 ignored.
This supersedes no historical source provenance and does not close independent
QA or full B.1c integration acceptance.
