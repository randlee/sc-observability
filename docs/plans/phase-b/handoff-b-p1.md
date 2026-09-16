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
