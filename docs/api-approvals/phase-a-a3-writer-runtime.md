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

## Affected Artifacts

- `crates/sc-observability-types/src/health.rs`
- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability/src/lib.rs`
- `crates/sc-observability/src/runtime.rs`
- `crates/sc-observability/src/maintenance.rs`
- `docs/public-api-checklist.md`
- `docs/api-design.md`
