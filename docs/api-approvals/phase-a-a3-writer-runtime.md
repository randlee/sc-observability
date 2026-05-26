## Scope

Approve the Phase-A A.3 public API expansion for the queue-backed writer
runtime in `sc-observability` and the shared logging health contract updates in
`sc-observability-types`.

The approved surface change includes:

- `Logger::log(...)`
- `Logger::try_log(...)`
- deprecated compatibility `Logger::emit(...)`
- `LogError`
- `TryLogError`
- `WriterState`
- queue and writer fields added to `LoggingHealthReport`
- `Logger::shutdown()` returning `Logger<Stopped>` directly

## Approval

Approved for Phase A implementation as the intentional writer-thread logging
API change set. Breaking and additive diffs in this batch are expected and are
covered by the paired public API checklist and normative doc updates.

Approved release exception rationale:

- this change normalizes the public logging API around `log(...)` and
  `try_log(...)` while preserving `emit(...)` as a deprecated compatibility
  path for the `1.2.x` line
- the queue-backed writer model and the updated shutdown/health contracts are
  intentional public-surface corrections, not accidental regressions
- consumers must be explicitly notified through release documentation before a
  publish based on this approval is considered complete

Required consumer notice for any release that carries this approval:

- `CHANGELOG.md` must describe the deprecation of `emit(...)`, the addition of
  `log(...)` / `try_log(...)`, and the queue-admission vs durability contract
- release notes must describe the same API normalization and migration path
- versioned public API documentation links for all published crates must be
  included in the release record

## Affected Artifacts

- `crates/sc-observability-types/src/health.rs`
- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability/src/lib.rs`
- `crates/sc-observability/src/runtime.rs`
- `crates/sc-observability/src/maintenance.rs`
- `docs/public-api-checklist.md`
- `docs/api-design.md`
- `CHANGELOG.md`
- `release/RELEASE-NOTES-TEMPLATE.md`
- `docs/publishing.md`
