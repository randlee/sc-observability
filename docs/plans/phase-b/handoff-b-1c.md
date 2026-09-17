---
id: B.1c-observation-prep-handoff
status: preparation-complete
branch: feature/phase-b-1c-observation-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1c-observation-prep
parent: feature/phase-b-1b-logger-prep
parent_checkpoint: 6d01518f2c72b16f97a8c2576596c828f17584e9
implementation_commits: 7d49ed2, 37bba66, 392c1dd, d90fba2, 4327fd3, 5b25b20
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
| Invalid names | Empty `ToolName` and `ServiceName` are rejected before facade construction. | Same validated type domain; no invalid config can reach either facade. | `tests/typed_observation.rs:501` |
| Empty routes | `Observability::new` returns `InitFailureKind::ObservationInitialization` through the legacy result adapter. | `new_typed` returns the same kind and diagnostic code. | `tests/typed_observation.rs:507`; `src/lib.rs:1142` |
| Logger startup failure | Legacy builder path reports the logger initialization classification. | Typed builder path reports `InitFailureKind::LoggerInitialization`. | `tests/typed_observation.rs:529`; `src/lib.rs:1142` |
| Eligible/ineligible filtering | Existing registration filter skips the ineligible observation, increments dropped count, and invokes no subscriber. | Paired typed fixture through `legacy_subscriber` has the same result. | `src/lib.rs:906`; `tests/typed_observation.rs:222` |
| Registration ordering | Two existing registrations invoke in insertion order. | The same typed adapters preserve `first, second` order. | `src/lib.rs:889`; `tests/typed_observation.rs:222` |
| No matching route | A different payload type returns `ObservationError::RoutingFailure`, dropped count 1, callback count 0. | Paired typed facade has the same error and counts. | `tests/typed_observation.rs:261` |
| Mixed failure | Failing plus successful registrations return `Ok`, count one subscriber failure, and deliver once to the successful route. | Same exact callback and health counts. | `src/lib.rs:936`; `tests/typed_observation.rs:261` |
| All failure | A failing route returns `RoutingFailure`, dropped count 1, failure count 1, and the routing diagnostic code. | Same exact result through typed construction and legacy registration. | `src/lib.rs:1017`; `tests/typed_observation.rs:261` |
| Diagnostic fidelity | Legacy adapter exposes custom and logger-family codes as unclassified without changing the context. | Round-trip typed adapter retains code and attached source. | `tests/typed_observation.rs:333`; neutral type-domain checks at `crates/sc-observability-types/src/typed.rs` |
| Output families | Existing registration invokes subscriber, log, span, and metric routes exactly once and emits concrete JSONL. | Typed implementations enter those registrations via the four B.1a adapters; paired fixture invokes each exactly once per facade. | `tests/typed_observation.rs:410`; `tests/typed_observation.rs:467` |
| Flush | Legacy `flush` returns the logger-flush classification and records one health failure for the operation. | `flush_typed` returns the same kind, health result, and concrete sink invocation count. | `src/lib.rs:1198` |
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
cargo test --locked --workspace: 177 unit/integration tests passed; 0 failed; 6 normal doctests passed; 2 compile-fail doctests passed
cargo clippy --locked --workspace --all-targets -- -D warnings: passed
python3 -m unittest discover -s scripts/ci/tests -p 'test_validate_log_import.py': 41 tests, OK
bash scripts/ci/validate_docs_consistency.sh: passed
bash scripts/ci/validate_dependency_bans.sh: passed
bash scripts/ci/validate_repo_boundaries.sh: passed
bash scripts/ci/validate_public_api_docs.sh: passed
bash scripts/ci/validate_public_api_diff.sh: exit 1 with additive API report, as expected for Phase B typed operations
```

The final parent merge-forward is
`6d01518f2c72b16f97a8c2576596c828f17584e9`; the parent owns its
logger/provenance changes and no lower layer was edited.
This is preparation evidence only: copied bridge source import, independent QA,
and full B.1c integration acceptance remain pending.
