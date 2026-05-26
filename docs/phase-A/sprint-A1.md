---
id: A.1
title: Architecture And API Lock
status: implemented
branch: feature/thread-optimization-a1-architecture-lock
worktree: ../sc-observability-worktrees/feature/thread-optimization-a1-architecture-lock
target: integrate/phase-a
---

# Sprint A.1 — Architecture And API Lock

```yaml
plan_type: sprint_plan
phase: A
sprint: A.1
worktree: ../sc-observability-worktrees/feature/thread-optimization-a1-architecture-lock
branch: feature/thread-optimization-a1-architecture-lock
status: implemented
estimated_scope: medium
```

## Goal

Lock the normative writer-thread architecture and the public logging contract
before implementation begins.

Execution note:

- the original planning draft referenced `develop` as the eventual downstream
  integration target
- the authoritative execution target for phase-A feature branches is
  `integrate/phase-a`, which is the correct target recorded in this sprint
  frontmatter

## Hard Dependencies

- `docs/project-plan.md`
- `docs/requirements.md` from the `plan/phase-a` branch state, not the
  `develop` baseline
- `docs/architecture.md` from the `plan/phase-a` branch state, not the
  `develop` baseline
- `docs/api-design.md` from the `plan/phase-a` branch state, not the
  `develop` baseline
- `docs/public-api-checklist.md`
- `docs/performance-pass.md` as ancillary historical context only; it is not a
  normative contract source for the phase-A API lock

## Prerequisites

- retained-log maintenance work is treated as complete historical input rather
  than the current controlling sprint
- no runtime implementation work for thread optimization lands before this
  sprint is accepted

## Exact Targets

- `docs/requirements.md`
- `docs/requirements.md` `LOG-014`
- `docs/architecture.md`
- `docs/architecture.md` §3.2 owns list
- `docs/architecture.md` §3.2.1 and §7 (`ADR-010`)
- `docs/architecture.md` §3.2.4
- `docs/api-design.md`
- `docs/project-plan.md`
- `docs/public-api-checklist.md`
- `docs/performance-pass.md`
- `docs/phase-A/readiness.md`
- `scripts/ci/validate_writer_thread_lock.sh`

## Deliverables

- normative docs updated to define the queue-backed writer-thread architecture
  as the approved runtime model
- explicit statement that retained-log maintenance runs on the same writer
  thread during idle or post-batch windows rather than on a dedicated
  maintenance-only thread
- explicit updates to:
  - `LOG-023`
  - `LOG-041`
  - `LOG-046`
  - `LOG-047`
  - `LOG-048`
  - `docs/architecture.md` §3.2.1
  so the normative docs no longer describe a separate maintenance worker model
- explicit lock text for the validator-sensitive requirements:
  - `LOG-041 Retained-log maintenance shall run on the writer thread during idle or post-batch windows, stay off the producer hot path, and shall not require an async runtime dependency.`
  - `LOG-046 \`Logger::shutdown()\` shall drain queued events, stop the writer thread within the configured bounded shutdown timeout, and record timeout/degraded state in health or error reporting before returning when the drain does not finish cleanly.`
- explicit statement that producers validate, redact, and queue log events,
  while one writer thread owns batching, sink writes, rotation, pruning, and
  flush
- ADR-010 in `docs/architecture.md` recording the writer-thread concurrency
  decision, rationale, rejected alternatives, and shutdown/drop consequences
- explicit statement that `WriterState` is defined in
  `sc-observability-types` per `TYP-030` / `TYP-040` and re-exported by
  `sc-observability`
- locked contract for:
  - `Logger::log(...)`
  - `Logger::try_log(...)`
  - deprecated `Logger::emit(...)`
  - `Logger::flush(...)`
  - `Logger::shutdown(...)`
- explicit semantics for:
  - `log()` blocks until queue admission, not durability
  - `try_log()` is non-blocking and returns explicit queue-full failure
  - `emit()` remains compatibility-only and is deprecated in favor of
    `log()` / `try_log()`
- explicit statement that `Logger<Running>` methods include `log()`,
  `try_log()`, `flush()`, and deprecated `emit()`
- explicit statement that `flush_errors_total` remains part of
  `LoggingHealthReport` and is not removed by the writer-thread redesign
- explicit shutdown drain contract stating:
  - `shutdown()` drains already-queued events before stopping the writer thread
  - the drain is bounded by the writer-thread shutdown timeout surface that
    supersedes the separate maintenance-worker join model
  - queued-but-unwritten events remaining after that bound are recorded through
    degraded health and dropped-event accounting
- locked queue and writer health fields for `LoggingHealthReport`
- locked logging error-code registry additions in `api-design.md` §11.9:
  - `LOGGER_QUEUE_FULL`
  - `LOGGER_WRITER_DEGRADED`
  - `LOGGER_SHUTDOWN_TIMED_OUT`
- locked public config surface for queue capacity and batching controls
- explicit public-API governance requirement stating that API changes must be
  documented and machine-checked
- explicit phase-A semver intent that the planned deprecation annotation uses
  `since = "1.2.0"` and is locked against the `project-plan.md` phase-A
  version designation, with final enforcement handled by the `A.2`
  governance gate
- explicit `project-plan.md` phase-A section for `v1.2.0` containing the
  sprint sequence `A.1` through `A.4` and exit criteria consistent with
  `docs/phase-A/readiness.md`
- update `docs/performance-pass.md` so it no longer conflicts with the approved
  redesign
- dedicated validation gate `scripts/ci/validate_writer_thread_lock.sh` that
  checks the writer-thread architecture markers in `requirements.md`,
  `architecture.md`, and `api-design.md`

## Locked Contract Samples

The sprint must freeze the logger-facing method shape and health additions with
explicit signatures or equivalent prose-tight code samples:

```rust
impl Logger<Running> {
    pub fn log(&self, event: LogEvent) -> Result<(), LogError>;
    pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError>;

    #[deprecated(
        since = "1.2.0",
        note = "Use log() for blocking queue admission or try_log() for non-blocking logging."
    )]
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError>;

    pub fn flush(&self) -> Result<(), FlushError>;
    pub fn shutdown(self) -> Logger<Stopped>;
}
```

```rust
pub enum LogError {
    InvalidEvent(EventError),
    WriterDegraded,
    ShutdownTimedOut,
}

pub enum TryLogError {
    InvalidEvent(EventError),
    QueueFull,
    WriterDegraded,
    ShutdownTimedOut,
}
```

```rust
pub struct LoggingHealthReport {
    pub state: LoggingHealthState,
    pub dropped_events_total: u64,
    pub flush_errors_total: u64,
    pub active_log_path: PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub queue_depth: u64,
    pub queue_capacity: u64,
    pub queue_high_water_mark: u64,
    pub queue_full_drops_total: u64,
    pub writer_state: WriterState,
    pub last_writer_error: Option<DiagnosticSummary>,
    pub query: Option<QueryHealthReport>,
    pub maintenance: Option<MaintenanceHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}

pub enum WriterState {
    Running,
    Degraded,
    Stopped,
}
```

## Paths To Delete

- none

## Acceptance Criteria

- `requirements.md`, `architecture.md`, `api-design.md`, and the phase-A sprint
  docs all describe the same writer-thread ownership model
- no normative doc still describes a maintenance-only background worker as the
  target runtime model
- the docs explicitly distinguish queue admission from write durability
- the docs explicitly define queue-full behavior and health reporting
- the docs freeze explicit method signatures or equivalent contract samples for
  `log()`, `try_log()`, deprecated `emit()`, and the queue/writer health
  additions
- `Logger::shutdown()` returns `Logger<Stopped>` with no `Result` wrapper and
  is identical in `requirements.md`, `api-design.md`, and this sprint doc
- `requirements.md` contains `LOG-047` and `LOG-048`, and this sprint cites
  them as the authoritative error-contract requirements for `LogError` and
  `TryLogError`
- the docs explicitly update `LOG-041`, `LOG-046`, and architecture §3.2.1 so
  the maintenance-worker model is replaced coherently by the writer-thread
  shutdown-drain model
- the docs explicitly retain `flush_errors_total` consistently, and that
  disposition is reflected in both `api-design.md` and `architecture.md`
  §3.2.4
- `docs/architecture.md` contains ADR-010 covering the writer-thread decision,
  rationale, rejected dedicated-worker alternative, and drop/shutdown
  consequences
- the docs explicitly record that future public API changes require both docs
  updates and automated API-gate approval
- `project-plan.md` contains the `v1.2.0` phase-A section, sprint sequence,
  and exit criteria consistent with `docs/phase-A/readiness.md`
- `NFR-010` and `NFR-011` compliance is explicitly verified for the normative
  doc updates, including the `1.2.0` version literal used by the planned
  deprecation annotation

## Non-Closure

- `A.1` does not implement runtime code

## Required Validation

- `bash scripts/ci/validate_docs_consistency.sh`
- `bash scripts/ci/validate_writer_thread_lock.sh`
- reviewer-confirmed cross-doc signature and semantics check against this
  sprint doc
