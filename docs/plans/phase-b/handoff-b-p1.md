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
