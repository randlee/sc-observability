---
id: A.1
title: Architecture And API Lock
status: planned
branch: feature/thread-optimization-a1-architecture-lock
worktree: ../sc-observability-worktrees/feature/thread-optimization
target: develop
---

# Sprint A.1 — Architecture And API Lock

```yaml
plan_type: sprint_plan
phase: A
sprint: A.1
worktree: ../sc-observability-worktrees/feature/thread-optimization
branch: feature/thread-optimization-a1-architecture-lock
status: planned
estimated_scope: medium
```

## Goal

Lock the normative writer-thread architecture and the public logging contract
before implementation begins.

## Hard Dependencies

- `docs/project-plan.md`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/public-api-checklist.md`
- `docs/performance-pass.md`

## Prerequisites

- retained-log maintenance work is treated as complete historical input rather
  than the current controlling sprint
- no runtime implementation work for thread optimization lands before this
  sprint is accepted

## Exact Targets

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/project-plan.md`
- `docs/public-api-checklist.md`
- `docs/performance-pass.md`
- `docs/phase-A/readiness.md`

## Deliverables

- normative docs updated to define the queue-backed writer-thread architecture
  as the approved runtime model
- explicit statement that retained-log maintenance runs on the same writer
  thread during idle or post-batch windows rather than on a dedicated
  maintenance-only thread
- explicit statement that producers validate, redact, and queue log events,
  while one writer thread owns batching, sink writes, rotation, pruning, and
  flush
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
- locked queue and writer health fields for `LoggingHealthReport`
- locked public config surface for queue capacity and batching controls
- explicit public-API governance requirement stating that API changes must be
  documented and machine-checked
- update `docs/performance-pass.md` so it no longer conflicts with the approved
  redesign

## Locked Contract Samples

The sprint must freeze the logger-facing method shape and health additions with
explicit signatures or equivalent prose-tight code samples:

```rust
impl Logger<Running> {
    // The concrete error types for log() and try_log() are finalized in A.1
    // and must be named in api-design.md before A.1 closes.
    pub fn log(&self, event: LogEvent) -> Result<(), /* locked in A.1 */>;
    pub fn try_log(&self, event: LogEvent) -> Result<(), /* locked in A.1 */>;

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
- the docs explicitly record that future public API changes require both docs
  updates and automated API-gate approval

## Non-Closure

- `A.1` does not implement runtime code
- `A.1` does not add CI scripts; it only locks the requirement for them

## Required Validation

- `bash scripts/ci/validate_docs_consistency.sh`
- reviewer-confirmed cross-doc signature and semantics check against this
  sprint doc
