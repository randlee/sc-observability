# B.P1 Runtime-Level Core Handoff

## Implementation inventory

- `sc-observability-types` adds the neutral runtime-level values:
  `OperationDiagnostic`, `LevelState`, `LevelChangeSource`,
  `ChangeDiagnostic`, `LevelChange`, `LevelChangeError`, and
  `AdmissionOutcome`, including the contract's native Serde tagging and stable
  code/remediation accessors.
- `sc-observability` adds the non-cloneable weak `LevelOwner`, opt-in
  `Logger::new_with_level_owner` and `LoggerBuilder::build_with_level_owner`,
  coherent `Logger::level_state`, and `try_log_with_outcome`.
- Admission now reads one per-logger effective level under the same short state
  lock used by runtime changes. Validation, redaction, queue waits, sink work,
  and writer work remain outside that lock.
- New construction uses a fallible writer-start path. Existing infallible
  construction remains source-compatible and is not used by the new APIs.
- Each committed change makes one nonblocking, bounded diagnostic admission
  attempt using the configured service and fixed core target/action fields.

## Developer test artifacts

- Native Serde coverage for the new value/error shapes is in
  `sc-observability-types/src/level.rs`.
- Core owner, revision, filtering, baseline, and stopped-owner behavior is in
  `sc-observability/src/lib.rs`.

## Required independent QA evidence

This handoff intentionally records no self-certified approval. `quality-mgr`
must run the sprint's workspace debug/release tests, doctests, formatting,
clippy, dependency/API/semver/docs checks, published-consumer fixture evidence,
and any required concurrency/fault follow-up before the sprint can close.

## QA-1 provenance and pending gates

QA-1 reviewed `feature/B-P1-runtime-core` at
`0a7600584e606b399ad28f5869f0a969a0bf5f61` for PR #103 and returned **FAIL**
on 2026-09-16. The source report is
`/tmp/phase-b-bp1-fix-1/qa-1-report.md`; its routing ledger is
`/tmp/phase-b-bp1-fix-1/findings-routing.md`. The report records that targeted
format/clippy/tests were green at review time, but that is not an approval and
does not establish the missing debug/release/doctest/API-diff/semver/consumer
gates. QA-B001 through QA-B010, QA-I001 through QA-I003, and QA-M001 through
QA-M004 remain subject to quality-mgr's evidence-based disposition in QA-2.

QA-B010 remains explicitly open: `runtime-level-contract.md` stays
`proposed_for_public_api_review` pending an actual ruling. No prior critical
review failure is represented here as a PASS, and this handoff does not claim
sprint closure.

## QA-2 durable evidence correction

QA-2 reviewed `08e33dd5f2597885fe9f33b0d9b8b4bb98d3e604` for PR #103 and
returned **FAIL**. Its report permalink is
<https://github.com/randlee/sc-observability/pull/103#issuecomment-5706554643>.
The report's original seven blocking IDs were QA-B003, QA-B004, QA-B005,
QA-B006, QA-B007, QA-B010, and QA-B011; its machine count incorrectly said six
and its timestamp was a placeholder. The subsequent correction is recorded at
<https://github.com/randlee/sc-observability/pull/103#issuecomment-5706566302>:
the authoritative count is seven, 11/17 findings were independently resolved,
and the verdict remains FAIL.

The raw QA-2 command logs were deleted after the QA task closed. They cannot be
reconstructed, so this document does not report their tool output as durable
evidence or call it PASS. Fresh QA-3 must retain raw fmt/clippy/test/API/semver
outputs, tool versions, and the exact reviewed SHA before QA-B005 can close.
On the reviewed SHA, CI format, clippy, docs-consistency, and dependency-bans
were successful; version-literals failed, and test, manifest-validation, and
public-api-governance were skipped because they depend on that failing gate.

## Compatibility fixture provenance and commands

The immutable legacy consumer and `LevelFilter` fixture record the reported
published `v1.2.0` baseline provenance (`dcc5263`). That object is not present
in this checkout, so this claim must be independently checked against the
published release before fixture regeneration; the fixture must never be
regenerated from B.P1 code. The consumer manifest and lock use package version
`1.2.0` to satisfy the repository's literal-consistency rule while preserving
the legacy source API shape.

Quality-mgr can independently capture the required fresh transcript with:

```sh
git show dcc5263:crates/sc-observability-types/src/level.rs
cargo test -p sc-observability-types published_level_filter_fixture_retains_its_native_serde_shape
cargo run --manifest-path crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.toml
python3 scripts/ci/validate_version_literals.py
```

## AC2 test mapping

| AC2 clause | Artifact |
| --- | --- |
| Admission versus mutation contention; coherent snapshot | `admission_and_level_mutation_contend_on_one_control_state` uses a `Barrier`, held control lock, and channels. |
| Post-stop retained snapshot; poisoned state | `level_state_recovers_the_last_committed_snapshot_after_poisoning`. |
| Stale owner after shutdown | `level_owner_changes_only_its_logger_and_filters_with_shared_admission`. |
| Owner cannot retain writer shutdown | The same test shuts down while the weak owner remains available and then observes `Stopped`. |
| Independent logger isolation | Pending fresh QA verification; no claim of completion is made here. |

## AC3 test mapping

`each_changed_level_queues_one_fixed_info_diagnostic` verifies fixed Info
diagnostics for Warn/Error/Off transitions. `saturated_diagnostic_queue_keeps_the_level_change_committed`
uses the existing writer maintenance gate to deterministically fill the bounded
queue, then verifies `Changed` with revision 2 and the original queue-full
diagnostic rather than rollback.
