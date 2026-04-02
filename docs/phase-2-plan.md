# Phase 2 Plan

**Status**: Draft for implementation review
**Branch**: `integrate/phase-2`
**Base**: `develop @ a666649`
**Reference repo**: `agent-team-mail`

## 1. Purpose

Phase 2 is the recovery implementation phase after the failed final review on
`develop`.

This phase has two goals:

1. fix correctness, best-practice, and documentation gaps before adding new
   public API surface
2. ship the missing query/follow and OTLP attachment work without further
   public API drift

The rule for this phase is strict: Sprint `2.0` closes before new API work
starts. No higher-layer sprint closes with unresolved ambiguity in its own
scope.

## 2. Reference Audit

Before designing anything, use `agent-team-mail` as the first source of
concrete implementation patterns.

Reference audit summary:

- direct reusable logging and rotation patterns exist in
  `agent-team-mail/crates/sc-observability/src/lib.rs`
- direct reusable OTLP transport, retry, timeout, and exporter separation
  patterns exist in
  `agent-team-mail/crates/sc-observability-otlp/src/lib.rs`
- direct reusable OTel bridge patterns exist in
  `agent-team-mail/crates/sc-observability/src/otlp_adapter.rs`
- direct reusable OTel health aggregation patterns exist in
  `agent-team-mail/crates/sc-observability/src/health.rs`
- `agent-team-mail` does not currently expose a direct `LogQuery`,
  `LogSnapshot`, `QueryError`, `JsonlLogReader`, or `LogFollowSession` module
  by those names, so the query/follow work is planned as new implementation
  using the existing ATM logging and rotation code as the behavioral reference

Planning consequence:

- query/follow contract and runtime work should port naming/layout/rotation
  behavior from ATM logging code where applicable, but should not pretend a
  one-file drop-in port exists
- OTLP transport, retry, timeout, and exporter wiring should be port-first
  wherever the standalone crate boundaries allow it

## 3. Phase Structure

Phase 2 is split into six sprints:

1. `Sprint 2.0` hardening and doc-truth reset
2. `Sprint 2.1` shared query/follow contract freeze
3. `Sprint 2.2` historical query runtime
4. `Sprint 2.3` follow runtime and standalone reader
5. `Sprint 2.4` OTLP public attachment surface
6. `Sprint 2.5` final hardening, docs sync, and release gate

This structure keeps bugs and design debt ahead of feature expansion while
still allowing the main missing API work to proceed in parallel once the shared
contract sprint is complete.

## 4. Dependency Graph

```text
Sprint 2.0
  -> Sprint 2.1
       -> Sprint 2.2
            -> Sprint 2.3
       -> Sprint 2.4
Sprint 2.2 + Sprint 2.3 + Sprint 2.4
  -> Sprint 2.5
```

Dependency rules:

- `Sprint 2.0` must finish first. It fixes correctness and best-practice debt
  that would otherwise leak into the new API work.
- `Sprint 2.1` freezes the shared types and error surface before runtime work.
- `Sprint 2.2` and `Sprint 2.4` can run in parallel after `Sprint 2.1`.
- `Sprint 2.3` depends on `Sprint 2.2` because follow and standalone reader
  should build on the finalized historical query machinery rather than fork it.
- `Sprint 2.5` is the final convergence sprint and must wait for all feature
  sprints.

## 5. Parallelization Plan

### 5.1 Safe Parallel Work

- `Sprint 2.0` Track A and Track B can run in parallel:
  - Track A: `sc-observability-types` hardening and doc status updates
  - Track B: runtime/error-plumbing fixes in `sc-observability` and
    `sc-observability-otlp`
- `Sprint 2.2` and `Sprint 2.4` can run in parallel after `Sprint 2.1`
  because one is logging-query runtime work and the other is OTLP attachment
  work

### 5.2 Work That Must Stay Serial

- `Sprint 2.1` must complete before any query/follow runtime code lands
- `Sprint 2.3` must not start before `Sprint 2.2` stabilizes the shared reader
  and filtering machinery
- `Sprint 2.5` must remain serial and be owned by one integrator pass

## 6. Sprint 2.0: Hardening And Truth Reset

### Scope

This sprint fixes all non-new-feature findings first:

- `BP-NT-001`: `SpanRecord::end(duration_ms: u64)` to typed duration guard
- `BP-NT-002`: raw OTLP millisecond config fields to typed duration guard
- `COBS-3/timestamp`: enforce UTC-only `Timestamp`
- `BP-ECR-001`: `ErrorContext::source()` must expose the real source error
- `BP-ECR-002`: preserve `std::io::Error` instead of stringifying too early
- `BP-IMC-001`: Telemetry shutdown TOCTOU
- `QA-001`: unsafe env mutation in tests
- `REQ-QA-010`: update governing docs from draft to approved where the design
  is already accepted
- `QA-002` through `QA-007`, `BP-IMC-003`, `BP-IMC-004`, `BP-NT-003`,
  `BP-NT-004`, `BP-NT-005`

Sprint `2.0` scopes `BP-IMC-001` to the telemetry shutdown race only. The
separate JSONL rotation size-check/rename race is tracked independently as
`BP-IMC-001-rotation` in Sprint `2.2`.

### Key files to modify

- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/error_codes.rs`
- `crates/sc-observability/src/lib.rs`
- `crates/sc-observability-otlp/src/lib.rs`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md`
- targeted tests under `crates/*/tests/` as needed

### Reference locations in `agent-team-mail`

- `crates/sc-observability-types/src/lib.rs`
  Use as the baseline for current OTLP config/env parsing and the raw
  millisecond fields that need to be replaced with typed duration values.
- `crates/sc-observability/src/lib.rs`
  Use for current retry/backoff call sites and file/rotation behavior that may
  need typed duration plumbing.
- `crates/sc-observability-otlp/src/lib.rs`
  Use for the transport timeout/retry/backoff wiring when replacing raw
  millisecond fields with typed values.
- `crates/sc-observability/src/health.rs`
  Use for health-state update patterns while reviewing shutdown and failure
  accounting.

### Deliverables

- `DurationMs` or equivalent shared duration type is introduced and used
  wherever phase findings require typed milliseconds
- UTC-only timestamp contract is enforced in code
- source-preserving error plumbing is restored
- shutdown race is removed from telemetry shutdown path
- unsafe env mutation tests are replaced with safe test setup
- governing docs are corrected so they no longer understate or overstate the
  current approval state

### Estimated waves

- Wave `2.0A`: shared types hardening (`DurationMs`, UTC timestamp)
- Wave `2.0B`: runtime error-plumbing and shutdown fixes
- Wave `2.0C`: test hardening and doc truth reset

### Exit criteria

- all listed non-feature findings are closed
- `cargo fmt --check --all`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- docs and checklist state are truthful before new API work starts

## 7. Sprint 2.1: Shared Query/Follow Contract Freeze

### Scope

Ship the entire shared query/follow contract in `sc-observability-types`:

- `REQ-QA-001`: `LogQuery`, `LogOrder`, and `LogFieldMatch`
- `REQ-QA-002`: `LogSnapshot`
- `REQ-QA-003`: `QueryError` and `SC_LOG_QUERY_*`
- `REQ-QA-004`: `QueryHealthReport`, `QueryHealthState`,
  `LoggingHealthReport.query`
- sealed `TelemetryHealthProvider` trait shipped here for later workspace-owned
  Sprint `2.4` plumbing

### Key files to modify

- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/error_codes.rs`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md`

### Reference locations in `agent-team-mail`

- `crates/sc-observability/src/lib.rs`
  Use for existing log-path layout, rotation naming, and event-file semantics
  that the query contracts must target.
- `crates/sc-observability-types/src/lib.rs`
  Use as the baseline for shared value-type style, validation style, and config
  layout.

No direct `LogQuery` or `JsonlLogReader` implementation exists in ATM by those
names today; this sprint is a genuine new shared-contract addition.

### Deliverables

- full shared query/follow contract exists in one crate
- validation rules are frozen:
  - `levels.is_empty()` means all levels
  - `limit = Some(0)` is invalid
  - `since > until` is invalid
  - `field_matches` use exact field-name and exact JSON value equality via
    `LogFieldMatch { field, value }`
- `QueryError` codes are stable and documented
- logging health is extended with query availability

### Estimated waves

- Wave `2.1A`: type definitions and validation
- Wave `2.1B`: error vocabulary and shared health additions
- Wave `2.1C`: docs/checklist freeze

### Exit criteria

- query/follow contracts are compileable and reviewable in isolation
- requirements, architecture, and checklist agree exactly on names and fields
- serde and validation coverage exists for the new shared contracts

## 8. Sprint 2.2: Historical Query Runtime

### Scope

Implement the historical query path in `sc-observability`:

- `REQ-QA-005` query half: `Logger::query`
- historical scan across active file plus rotated siblings
- filter application, ordering, truncation, and decode failure behavior
- query health population on `LoggingHealthReport`
- `BP-IMC-001-rotation`: resolve the JSONL rotation size-check/rename TOCTOU
  in the file-sink/query file model

### Key files to create or modify

- `crates/sc-observability/src/lib.rs`
- likely new internal modules:
  - `crates/sc-observability/src/query.rs`
  - `crates/sc-observability/src/jsonl_reader.rs`
  - `crates/sc-observability/src/query_health.rs`
- query-focused tests under:
  - `crates/sc-observability/tests/`

### Reference locations in `agent-team-mail`

- `crates/sc-observability/src/lib.rs`
  Use for the existing JSONL file layout, rotation naming, and write path.
  Historical query must target this exact on-disk model rather than invent a
  different layout.

### Deliverables

- `Logger::query(&self, &LogQuery) -> Result<LogSnapshot, QueryError>`
- deterministic `LogOrder::OldestFirst` and `LogOrder::NewestFirst`
- historical scan over the active file and resolved rotation set
- malformed JSONL records surface as `QueryError::Decode`
- query health reflects availability and last error

### Estimated waves

- Wave `2.2A`: reusable JSONL scan core
- Wave `2.2B`: `Logger::query` integration
- Wave `2.2C`: health accounting, truncation behavior, and tests

### Exit criteria

- historical query works over real rotated files
- no duplicate or silently skipped committed records in query tests
- logger query behavior remains synchronous and ATM-free

## 9. Sprint 2.3: Follow Runtime And Standalone Reader

### Scope

Implement the remaining logging API:

- `REQ-QA-005` follow half: `Logger::follow`, `LogFollowSession`
- `REQ-QA-006`: `JsonlLogReader`
- tail semantics, rotation reopen logic, and offline-reader parity

### Key files to create or modify

- `crates/sc-observability/src/lib.rs`
- likely new internal modules:
  - `crates/sc-observability/src/follow.rs`
  - `crates/sc-observability/src/jsonl_reader.rs`
- integration tests under:
  - `crates/sc-observability/tests/`

### Reference locations in `agent-team-mail`

- `crates/sc-observability/src/lib.rs`
  Use for the active log path and `.N` rotation behavior.

No direct ATM `LogFollowSession` or `JsonlLogReader` module is currently
present by those names, so this sprint should adapt ATM file semantics and the
current standalone sink implementation rather than attempt a literal port.

### Deliverables

- `Logger::follow(&self, LogQuery) -> Result<LogFollowSession, QueryError>`
- `LogFollowSession::poll()` and `health()`
- `JsonlLogReader::query()` and `follow()`
- tail start-position behavior is frozen:
  - follow begins at the tail of the currently visible log set
  - backlog plus tail is achieved by `query()` first, then `follow()`

### Estimated waves

- Wave `2.3A`: session state and offset tracking
- Wave `2.3B`: rotation/truncation reopen logic
- Wave `2.3C`: standalone reader parity and lifecycle tests

### Exit criteria

- follow remains caller-driven and synchronous
- active-file rename/recreate during rotation is handled without duplicate or
  silently skipped committed records
- offline reader and live logger share the same query semantics

## 10. Sprint 2.4: OTLP Public Attachment Surface

### Scope

Close `COBS-2` by shipping the actual public OTLP attachment surface:

- public projector-registration helpers
- telemetry health provider bridge into `ObservabilityBuilder`
- end-to-end full-stack wiring through shipped public APIs only

### Key files to create or modify

- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observe/src/lib.rs`
- `crates/sc-observability-otlp/src/lib.rs`
- full-stack integration tests:
  - `crates/sc-observability-otlp/tests/`
  - `crates/sc-observe/tests/` if additional routing coverage is needed

### Reference locations in `agent-team-mail`

- `crates/sc-observability-otlp/src/lib.rs`
  Use for exporter separation, timeout/backoff wiring, and transport-boundary
  patterns.
- `crates/sc-observability/src/otlp_adapter.rs`
  Use for bridge/adaptation structure between higher-level observability code
  and lower-level OTLP transport code.
- `crates/sc-observability/src/health.rs`
  Use for health-provider shape and runtime-to-health projection patterns.

### Deliverables

- `TelemetryProjectors<T>`
- `TelemetryProjectors<T>::into_registration()`
- workspace-owned `TelemetryHealthProvider` plumbing for `Telemetry`
- `ObservabilityBuilder::with_telemetry_health_provider(...)`
- `ObservabilityHealthReport.telemetry` populated when configured

### Estimated waves

- Wave `2.4A`: health-provider trait plumbing
- Wave `2.4B`: projector helper implementation
- Wave `2.4C`: public integration tests and docs

### Exit criteria

- a downstream user can wire `sc-observe` and `sc-observability-otlp` together
  without test-only wrappers
- `sc-observe` remains OTLP-free at the crate dependency level
- full-stack integration tests use only shipped public APIs

## 11. Sprint 2.5: Final Hardening And Release Gate

### Scope

Converge the entire phase:

- close any remaining doc/coverage drift from earlier sprints
- rerun all validation and final review gates
- ensure release-readiness docs are truthful
- request publish-gate review only after zero blocking findings remain

### Key files to create or modify

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/public-api-checklist.md`
- `docs/release-readiness-checklist.md`
- `docs/test-strategy.md`
- any touched crate test files needed to close coverage gaps

### Reference locations in `agent-team-mail`

- use the corresponding ATM `sc-observability*` docs and tests only as
  comparison points for behavior and coverage shape
- do not import ATM-specific compatibility behavior into shared crates during
  this sprint

### Deliverables

- all phase findings closed or explicitly rejected with rationale
- docs/checklists aligned with shipped code
- final review packet ready for QA and publish decision

### Estimated waves

- Wave `2.5A`: docs and checklist convergence
- Wave `2.5B`: full validation pass
- Wave `2.5C`: final QA handoff and publish recommendation

### Exit criteria

- zero blocking findings in the final review
- all required CI and validation gates pass
- the phase ends with a factual publish/no-publish recommendation

## 12. Development Order Summary

The execution order for engineering should be:

1. close Sprint `2.0` hardening debt first
2. freeze the shared contract in Sprint `2.1`
3. split implementation into:
   - Sprint `2.2` historical query runtime
   - Sprint `2.4` OTLP attachment surface
4. finish Sprint `2.3` follow/runtime reader work on top of `2.2`
5. converge everything in Sprint `2.5`

This order fixes the best-practice and correctness debt first, then implements
the incomplete design in controlled layers, then forces one final convergence
pass before publish.
