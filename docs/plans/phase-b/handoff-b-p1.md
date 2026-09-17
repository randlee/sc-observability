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

## Independent QA and execution closeout

This handoff does not self-certify QA. QA-4 independently verified the B.P1
code/evidence findings at `a8951321e6b1df6044c2ee1f6f41c07a9dad7d99`; its
[report](https://github.com/randlee/sc-observability/pull/103#issuecomment-5706903378)
records 17/18 tracked findings resolved, all ten PR checks successful, and
QA-B010 as the sole governance-only hold. The retained QA-4 evidence directory
is `/Users/randlee/.config/atm/share/sc-obs/qa-evidence/phase-b-bp1-qa-4/a8951321e6b1df6044c2ee1f6f41c07a9dad7d99/rust-qa-agent/`;
its 17 raw files and `SHA256SUMS.txt` are the checksum index.

On 2026-09-16 (America/Los_Angeles), aobs relayed the owner's instruction:
"you task was to complete phase-b w/ publish delayed until the end."
Coordinating lead aobs withdraws its additional manual QA-B010 execution stop
and records scoped implementation approval in
[`phase-b-runtime-level.md`](../../api-approvals/phase-b-runtime-level.md).
This is neither a personal owner signature nor independent QA PASS; historical
verdicts remain unchanged. QA-5's scoped checks are satisfied while its overall
review verdict at that head remains FAIL. Live publication remains reserved for B.7.

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

QA-B010 was explicitly open at this point in the historical record because
`runtime-level-contract.md` was `proposed_for_public_api_review`. The coordinator
withdrawal above supersedes that additional execution stop. No prior critical review
failure is represented here as a PASS.

## QA-2 and QA-3 evidence record

QA-2 reviewed `08e33dd5f2597885fe9f33b0d9b8b4bb98d3e604` for PR #103 and
returned **FAIL**. Its report permalink is
<https://github.com/randlee/sc-observability/pull/103#issuecomment-5706554643>.
The report's original seven blocking IDs were QA-B003, QA-B004, QA-B005,
QA-B006, QA-B007, QA-B010, and QA-B011; its machine count incorrectly said six
and its timestamp was a placeholder. The subsequent correction is recorded at
<https://github.com/randlee/sc-observability/pull/103#issuecomment-5706566302>:
the authoritative count is seven, 11/17 findings were independently resolved,
and the verdict remains FAIL.

The deleted QA-2 logs are historical context only; they are not evidence for
this revision. QA-3 reviewed
`41dd3201a2005c3ecac1f107f6892055f403518f` and returned **FAIL**; its report
is [the QA-3 PR record](https://github.com/randlee/sc-observability/pull/103#issuecomment-5706715286).
The retained raw evidence is the exact directory
`/Users/randlee/.config/atm/share/sc-obs/qa-evidence/phase-b-bp1-qa-3/41dd3201a2005c3ecac1f107f6892055f403518f/rust-qa-agent/`:
`tool-versions.txt`, `fmt.log`, `clippy.log`, `tests-debug.log`,
`tests-release.log`, `doctests.log`, `version-literals.log`,
`consumer-fixture.log`, `gh-pr-checks-103.log`, `gh-pr-view-103.log`, and
`SHA256SUMS.txt`. The recorded checksums are the integrity reference for those
logs; their presence is evidence retention, not a self-certified QA PASS.

QA-3's tracker denominator is 18 (the original 17 plus QA-B011): 13 findings
were independently fixed, and QA-B003, QA-B004, QA-B005, QA-B007, and QA-B010
remained open at that review. QA-4 subsequently independently verified the
first four at `a8951321`; aobs withdrew the remaining
coordinator-imposed execution hold. These later dispositions do not rewrite QA-3's
FAIL verdict.

## Compatibility fixture provenance and commands

The immutable legacy consumer and `LevelFilter` fixture are traced to the
verified `v1.2.0` release commit
`dcc52685fd845c8d1bddde29199e799ae921cf5c` (2026-05-26). The native
`LevelFilter` Serde fixture is frozen from that baseline and must never be
regenerated from B.P1 code.

The two legs deliberately exercise the same frozen legacy source in isolated
dependency environments: `bp1-published-v1.2.0-baseline` locks exact registry
`=1.2.0` crates, while `bp1-published-v1.2.0-consumer` resolves the B.P1
candidate through local paths. The committed lockfiles make the distinction
inspectable: only the baseline lock has registry source/checksum entries for
the two `sc-observability` crates.

The fixture-input SHA-256 values are:

| Input | SHA-256 |
| --- | --- |
| `level_filter.json` | `93d3f2db5b07e292122ac983b6f82e7d957a948cd9ddf3b98d2113223fa1afb5` |
| released-baseline `Cargo.toml` | `c0d8651563c2c40d13d420df08a1bc9ec76294527c9c237107661a8810a6a465` |
| released-baseline `Cargo.lock` | `5aff91a6449ec0be1b588ff19bce1cb093e23e78ae8351cd0048ea8ce374c65f` |
| released-baseline `src/main.rs` | `68ef6d6426edc348ec2b46b879525f190d306a3af5ac2fdb94ece000c3597557` |
| candidate `Cargo.toml` | `68f4c0d5d03ef8361319e8d0a54a324b8f8caef9edc6fef549b6c3e5fba8b7d6` |
| candidate `Cargo.lock` | `e084747a104c243ec9bcc6f51777a1fb7720c8f92902a26cf936552fb4899fa2` |
| candidate `src/main.rs` | `68ef6d6426edc348ec2b46b879525f190d306a3af5ac2fdb94ece000c3597557` |

Quality-mgr can independently reproduce both legs with:

```sh
git show dcc52685fd845c8d1bddde29199e799ae921cf5c:crates/sc-observability-types/src/level.rs
cargo test -p sc-observability-types published_level_filter_fixture_retains_its_native_serde_shape
cargo run --locked --manifest-path crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-baseline/Cargo.toml
cargo run --locked --manifest-path crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.toml
python3 scripts/ci/validate_version_literals.py
```

## AC2 test mapping

| AC2 clause | Artifact |
| --- | --- |
| Admission versus mutation contention; coherent snapshot | `admission_and_level_mutation_contend_on_one_control_state` uses a `Barrier`, held control lock, and channels. |
| Post-stop retained snapshot; poisoned state | `level_state_recovers_the_last_committed_snapshot_after_poisoning`. |
| Stale owner after shutdown | `level_owner_changes_only_its_logger_and_filters_with_shared_admission`. |
| Owner cannot retain writer shutdown | The same test shuts down while the weak owner remains available and then observes `Stopped`. |
| Independent logger isolation | `separate_level_owners_do_not_cross_logger_boundaries`. |

## AC3 test mapping

`each_changed_level_queues_one_fixed_info_diagnostic` verifies fixed Info
diagnostics for Warn/Error/Off transitions. `saturated_diagnostic_queue_keeps_the_level_change_committed`
uses the existing writer maintenance gate to deterministically fill the bounded
queue, then verifies `Changed` with revision 2 and the original queue-full
diagnostic rather than rollback.

`level_owner_rejects_changes_during_an_actual_shutdown_stopping_window` holds
the writer in its controlled maintenance pass, starts shutdown, observes the
published `Stopping` lifecycle, and verifies a rejected owner change does not
mutate the retained state. `level_change_diagnostic_uses_configured_context_and_redacts_outside_state_lock`
verifies configured service/identity, custom-redactor application, and that the
redactor can acquire the level-state mutex, proving callback work is outside it.
