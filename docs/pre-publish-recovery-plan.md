# SC-Observability Pre-Publish Recovery Plan

**Status**: Active planning baseline
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`architecture.md`](./architecture.md)
- [`implementation-plan.md`](./implementation-plan.md)
- [`sprint-plan.md`](./sprint-plan.md)
- [`public-api-checklist.md`](./public-api-checklist.md)
- [`release-readiness-checklist.md`](./release-readiness-checklist.md)

## 1. Purpose

This document is the controlling execution plan for the current pre-publish
recovery phase.

It replaces any optimistic assumption that the repo is already ready to publish.
The goal of this phase is to:

1. fix release-blocking correctness and best-practice gaps first
2. finish the incomplete design elements before implementation drifts further
3. close all public API ambiguity before publish
4. require one final zero-blocker review before release work resumes

## 2. Current Blocking Findings

The current `develop` branch must not be published until these are resolved:

1. the query/follow contracts and logging API approved in
   [`requirements.md`](./requirements.md) and
   [`architecture.md`](./architecture.md) do not exist in shipped code
2. the documented OTLP attachment model does not exist as a real public crate
   surface; it currently exists only as test scaffolding
3. the UTC-only timestamp contract is documented but not enforced in the shared
   type system

These are publish blockers, not cleanup items.

## 3. Recovery Rules

- Fix truthfulness first. Docs, checklists, and release gates must reflect the
  actual state of the codebase before new feature work starts.
- Fix lower-layer contract gaps before higher-layer integration work.
- No sprint closes with unresolved ambiguity in the public surface shipped by
  that sprint.
- No sprint closes with `[~]` or `[ ]` checklist items in its own scope unless
  they are explicitly deferred in this document.
- BP-NT-001 and BP-NT-002 are pre-publish scope, not silently deferred:
  Sprint 1 introduces shared `DurationMs` and updates `SpanRecord::end(...)`;
  Sprint 3 replaces raw OTLP millisecond fields with `DurationMs`.
- No publish gate may be marked complete until the final review reports zero
  blocking findings.

## 4. Recovery Order

| Sprint | Priority | Goal | Crates | Publish impact |
| --- | --- | --- | --- | --- |
| S0 | highest | truth reset and design freeze | docs only | unblocks safe implementation |
| S1 | highest | shared contract hardening | `sc-observability-types` | removes core API drift |
| S2 | high | logging query/follow runtime | `sc-observability` | closes largest missing API surface |
| S3 | high | routing and OTLP attachment closure | `sc-observability-types`, `sc-observe`, `sc-observability-otlp` | makes full-stack integration real |
| S4 | high | hardening, final review loop, release gate | all crates + docs | decides publish/no-publish |

The rule is strict: `S(n+1)` does not start until `S(n)` is code-complete,
doc-complete, test-complete, and review-ready.

## 5. Sprint 0: Truth Reset And Design Freeze

### 5.1 Objective

Make the docs and execution plan truthful, then freeze the exact missing public
design so development can proceed without invention.

### 5.2 Mandatory outputs

- update the planning docs so they no longer imply that release readiness is
  already achieved
- make the release checklist reflect the current blocking gaps
- freeze the exact public shape for the missing query/follow contracts
- freeze the exact public shape for OTLP attachment
- freeze the exact UTC timestamp enforcement strategy

### 5.3 Design decisions to freeze in Sprint 0

#### Query/Follow semantics

These decisions must be written down before implementation starts:

- `LogQuery.levels.is_empty()` means "all levels"
- `limit = None` means "no explicit limit"
- `limit = Some(0)` is invalid and returns `QueryError::InvalidQuery`
- `since > until` is invalid and returns `QueryError::InvalidQuery`
- `field_matches` use exact field-name lookup and exact JSON value equality
- `order` is applied after filtering and before limiting
- `Logger::follow(...)` and `JsonlLogReader::follow(...)` are tail-style
  sessions; they begin at the end of the currently visible log set and do not
  replay historical backlog
- callers that need backlog plus tail must call `query()` first, then `follow()`
- malformed JSONL records return `QueryError::Decode`; they are never silently
  skipped
- `QueryHealthReport` represents query/follow availability only and does not
  replace normal logging sink health

#### OTLP attachment shape

The public OTLP integration surface is frozen as:

- `sc-observability-otlp` ships a public `TelemetryProjectors<T>` helper
- `TelemetryProjectors<T>` wraps caller-provided `LogProjector<T>`,
  `SpanProjector<T>`, and `MetricProjector<T>` implementations and forwards the
  projected records into `Telemetry`
- `TelemetryProjectors<T>::into_registration()` returns a
  `ProjectionRegistration<T>` suitable for direct registration with
  `ObservabilityBuilder`
- `Telemetry` participates through workspace-owned plumbing around the public
  `TelemetryHealthProvider` trait owned by `sc-observability-types`
- `TelemetryHealthProvider` is frozen as:
  `pub trait TelemetryHealthProvider: telemetry_health_provider_sealed::Sealed + Send + Sync { fn telemetry_health(&self) -> TelemetryHealthReport; }`
- `ObservabilityBuilder` exposes
  `with_telemetry_health_provider(Arc<dyn TelemetryHealthProvider>)` so
  `ObservabilityHealthReport.telemetry` can be populated without adding an OTLP
  dependency to `sc-observe`

#### UTC timestamp enforcement

The shared timestamp strategy is frozen as:

- `Timestamp` stops being a plain type alias and becomes a real public newtype
- all public constructors normalize to UTC
- serde output is stable and UTC-only
- raw non-UTC `OffsetDateTime` values do not cross the public API boundary

### 5.4 Exit criteria

- every controlling planning document points at this recovery phase
- the release readiness checklist is truthful
- the API checklist contains all missing planned public items
- no open naming or behavior questions remain for S1 through S3

## 6. Sprint 1: Shared Contract Hardening

### 6.1 Objective

Close the missing shared-contract and best-practice gaps in
`sc-observability-types`.

### 6.2 Required code outputs

- UTC-enforced `Timestamp`
- `DurationMs`
- `LogOrder`
- `LogFieldMatch`
- `LogQuery`
- `LogSnapshot`
- `QueryError`
- `QueryHealthState`
- `QueryHealthReport`
- `TelemetryHealthProvider`
- `SpanRecord::end(DurationMs)` update
- `SC_LOG_QUERY_INVALID_QUERY`
- `SC_LOG_QUERY_IO`
- `SC_LOG_QUERY_DECODE`
- `SC_LOG_QUERY_UNAVAILABLE`
- `SC_LOG_QUERY_SHUTDOWN`
- `LoggingHealthReport.query`

The `SC_LOG_QUERY_*` stable error codes land in Sprint 2.1, where the shared
query/follow contract ships as one coherent surface.

`LoggingHealthReport.query` is also deferred to Sprint 2.1 so the health field
lands together with `QueryHealthReport` and the shared query vocabulary.

### 6.3 File targets

- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/error_codes.rs`
- `docs/public-api-checklist.md`
- `docs/requirements.md` only if wording must be tightened for the frozen
  design decisions
- `docs/architecture.md` only if wording must be tightened for the frozen
  design decisions

### 6.4 Required tests

- UTC normalization and serde tests for `Timestamp`
- validation and serde tests for `DurationMs`
- validation tests for `LogQuery`
- serde round-trip tests for all new query types
- `QueryError` to stable error-code mapping tests
- `LoggingHealthReport` serde test proving `query` survives round-trip

### 6.5 Exit criteria

- `sc-observability-types` contains the full shared query/follow contract
- UTC-only timestamps are enforced, not just documented
- all new public items are marked `[x]` in the checklist for Sprint 1 scope
- no remaining query/follow type or error-surface ambiguity exists

## 7. Sprint 2: Logging Query/Follow Runtime

### 7.1 Objective

Implement the missing historical query and synchronous tail APIs in
`sc-observability`.

### 7.2 Required code outputs

- `Logger::query(&self, &LogQuery) -> Result<LogSnapshot, QueryError>`
- `Logger::follow(&self, LogQuery) -> Result<LogFollowSession, QueryError>`
- `LogFollowSession`
- `JsonlLogReader`
- rotation-aware discovery of active and rotated log files
- `LoggingHealthReport.query` population

### 7.3 Required behavior

- historical reads cover the active log plus the rotation set that matches the
  documented naming layout
- `LogOrder::OldestFirst` and `LogOrder::NewestFirst` are deterministic
- follow sessions survive active-file rename/recreate during rotation on
  Unix-family platforms
- committed records are neither duplicated nor silently skipped across rotation
  on Unix-family platforms; Windows remains best-effort because stable Rust
  does not expose a reliable file identity equivalent to `(dev, ino)`
- `follow().poll()` is synchronous and caller-driven
- `query()` and `follow()` remain available without `sc-observe` or OTLP

### 7.4 File targets

- `crates/sc-observability/src/lib.rs`
- additional internal modules if the implementation becomes too large:
  `query.rs`, `follow.rs`, `jsonl_reader.rs`, `health.rs`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md` if new focused query/follow test gates need to be
  called out explicitly

### 7.5 Required tests

- historical query over active file only
- historical query over active + rotated files
- `LogOrder::NewestFirst` ordering with limit
- invalid query validation paths
- malformed record decode failure path
- follow session starts at tail, not backlog
- follow session after rotation/recreate on Unix-family platforms
- explicit Windows limitation coverage for best-effort truncate/recreate
  detection
- logger shutdown makes query/follow unavailable where appropriate
- offline `JsonlLogReader` parity tests against `Logger`

### 7.6 Exit criteria

- `sc-observability` ships the full approved query/follow API
- tests prove rotation correctness
- no async runtime or file watcher dependency is introduced
- query/follow documentation and implementation match exactly

## 8. Sprint 3: Routing And OTLP Attachment Closure

### 8.1 Objective

Turn the OTLP attachment story from documentation-only intent into a shipped,
reviewable public integration surface.

### 8.2 Required code outputs

- `TelemetryProjectors<T>`
- `TelemetryProjectors<T>::into_registration() -> ProjectionRegistration<T>`
- `TelemetryHealthProvider` implementation for `Telemetry`
- `ObservabilityBuilder::with_telemetry_health_provider(...)`
- `ObservabilityHealthReport.telemetry` populated when a provider is attached
- `OtelConfig.timeout_ms`, `initial_backoff_ms`, `max_backoff_ms`, and
  `MetricsConfig.export_interval_ms` converted from raw `u64` to `DurationMs`

`sc-observe` remains a dev-only dependency of `sc-observability-otlp` in this
phase so the public attachment path can be exercised in integration tests
without making routing a runtime dependency of the OTLP crate.

### 8.3 Required behavior

- attachment uses only public APIs from `sc-observability-types`,
  `sc-observe`, and `sc-observability-otlp`
- `sc-observe` does not gain a dependency on `sc-observability-otlp`
- applications can register wrapped projectors with `ObservabilityBuilder`
  without test-only scaffolding
- `ObservabilityHealthReport` exposes attached telemetry health when configured

That test coverage requirement does not change the shipped crate layering:
`sc-observe` is still dev-only for `sc-observability-otlp`, not a Sprint 3
runtime dependency.

### 8.4 File targets

- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observe/src/lib.rs`
- `crates/sc-observability-otlp/src/lib.rs`
- `docs/public-api-checklist.md`
- `docs/architecture.md` if the new public helper names must be made explicit

### 8.5 Required tests

- public attachment integration test with no test-only wrappers
- health propagation test proving `ObservabilityHealthReport.telemetry`
  populates when a provider is attached
- post-shutdown behavior for attached telemetry
- exporter-failure path still remains fail-open

### 8.6 Exit criteria

- the documented OTLP attachment model exists in shipped code
- no test-only helper is required to wire the full stack together
- `sc-observe` remains generic and OTLP-free
- downstream users can follow one clear public integration path

## 9. Sprint 4: Hardening, Final Review Loop, And Publish Gate

### 9.1 Objective

Run the final production-readiness pass only after S0 through S3 are merged.

### 9.2 Required outputs

- all docs and checklists aligned with shipped behavior
- all new public API items marked `[x]`
- release readiness checklist updated from factual verification, not optimism
- final critical review with severity-tagged findings
- explicit publish/no-publish decision captured in ATM
- `LogFollowSession` lifecycle typing reviewed and either finalized or
  explicitly deferred with rationale

### 9.3 Required validation gates

- `cargo fmt --check --all`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `bash scripts/ci/validate_repo_boundaries.sh`
- docs consistency validation
- dependency-ban validation
- one final full-stack integration path proving logging + routing + OTLP
  attachment through shipped public APIs

### 9.4 Exit criteria

- final review reports zero blocking findings
- release readiness checklist is actually complete
- team-lead receives a pass/fail publish recommendation backed by evidence

### 9.5 Important Findings Carried Into Sprint 4

Sprint 4 never executed, so this section must not claim blanket closure. Each
carry-over ID below is reconciled against the current branch using either
branch-local source evidence or the open production-readiness review findings.

| Carry-over ID | Current status | Evidence / open mapping | Notes |
| --- | --- | --- | --- |
| `QA-001` | fixed with evidence | [`crates/sc-observability/Cargo.toml:15-16`](../crates/sc-observability/Cargo.toml), [`crates/sc-observability/src/lib.rs:885-888`](../crates/sc-observability/src/lib.rs) | test env mutation now uses `temp-env` helpers instead of raw process-env mutation |
| `BP-ST-001` | still open | `PRR-B-008`, `PRR-I-007` | the original finding text is not preserved anywhere on this branch, so closure cannot be proven from source evidence |
| `BP-ST-002` | still open | `PRR-B-008`, `PRR-I-007` | the original finding text is not preserved anywhere on this branch, so closure cannot be proven from source evidence |
| `BP-IMC-001` | still open | `PRR-B-008`, `PRR-I-007` | the tag meaning drifted between planning and later QA; do not claim closure until it is reconciled to one stable issue statement |
| `BP-IMC-002` | still open | `PRR-B-008`, `PRR-I-007` | the original finding text is not preserved in the branch-local docs, so the current code cannot be truthfully mapped back to a closed issue |
| `BP-NT-003` | still open | `PRR-B-008`, `PRR-I-007` | the original newtype-related finding text is not preserved in the branch-local docs |
| `BP-NT-004` | still open | `PRR-B-008`, `PRR-I-007` | the original newtype-related finding text is not preserved in the branch-local docs |
| `BP-NT-005` | still open | `PRR-B-008`, `PRR-I-007` | the original newtype-related finding text is not preserved in the branch-local docs |
| `BP-ECR-001` | fixed with evidence | [`crates/sc-observability-types/src/errors.rs:7-9`](../crates/sc-observability-types/src/errors.rs), [`crates/sc-observability-types/src/errors.rs:24`](../crates/sc-observability-types/src/errors.rs), [`crates/sc-observability-types/src/errors.rs:75-86`](../crates/sc-observability-types/src/errors.rs) | public error wrappers and enums now expose the underlying `ErrorContext` via `#[source]` instead of hiding the real source chain |
| `BP-ECR-002` | still open | `PRR-B-001`, [`crates/sc-observability-otlp/src/lib.rs:716-723`](../crates/sc-observability-otlp/src/lib.rs) | `Telemetry::shutdown()` still discards `flush()` with `let _ = self.flush()` |
| `BP-ECR-003` | still open | `PRR-B-008`, `PRR-I-007` | the original error-contract finding text is not preserved in the branch-local docs |
| `REQ-QA-008-phase` | still open | `PRR-B-008`, `PRR-I-007` | this phase-level QA tag survives only as an ID; the branch-local plan does not preserve enough detail to prove closure |
| `REQ-QA-009-phase` | still open | `PRR-B-008`, `PRR-I-007` | this phase-level QA tag survives only as an ID; the branch-local plan does not preserve enough detail to prove closure |

LogFollowSession lifecycle typing: accepted at current design
(synchronous poll-only, no typestate on session lifetime). The only
post-construction transition is shutdown, enforced at runtime via the Logger
shutdown flag; typestate would require shared interior-state machinery with no
ergonomic benefit for a synchronous polling API. Explicitly deferred to
post-publish.

Sprint 4 also reviews `BP-TS-001` on Logger and Telemetry shutdown-state
hardening and `BP-TS-002` on `SpanRecord<SpanEnded>` optional duration before
publish. The closure rule for this branch is that none of these items remain
blocking after the Sprint 4 validation suite passes and the
release-readiness checklist is marked from evidence rather than optimism.

Windows follow limitation: accepted platform limitation for v1. On Windows,
the non-Unix file identity fallback uses `(len, modified_nanos)` because
stable Rust does not expose a reliable replacement for Unix `(dev, ino)` file
identity. That fallback cannot always distinguish ordinary append activity from
truncate-and-recreate of the active file, so the recreate follow test is
skipped on Windows with an explicit rationale in the test source.

ARCH-001 verification outcome: `crates/sc-observability-otlp/src/` contains no
`sc_observe::` imports or runtime calls. The only `sc-observe` usage for the
public attachment path is in integration tests and manifest wiring, so the
runtime dependency can move to `dev-dependencies` in the follow-on code pass.

BP-PANIC-001 implementation note: replace the exporter-status `&str` dispatch
in `Telemetry::record_export_success(...)` and
`Telemetry::record_export_failure(...)` with a crate-local `ExporterKind` enum.
The enum should cover the existing `logs`, `traces`, and `metrics` call sites
and own the mapping to the corresponding exporter health slot.

### 9.6 Explicitly Deferred To Post-Publish

- `BP-TS-001`: deeper typestate hardening for runtime shutdown would require
  invasive API and ownership changes that are too disruptive for a
  stability-first publish gate.
- `BP-TS-002`: replacing the runtime shutdown checks with richer compile-time
  lifecycle encoding would add shared-state complexity without changing the
  synchronous query/follow surface.

No other Sprint 4 carry-over item is allowed to remain deferred.

## 10. Design Closure Loop

This loop is mandatory at the end of every sprint.

1. Compare implementation against:
   - `requirements.md`
   - `architecture.md`
   - `public-api-checklist.md`
   - sprint-specific exit criteria in this plan
2. List every mismatch, ambiguity, and unproven behavior.
3. Fix the mismatches in the same sprint unless explicitly deferred in this
   plan.
4. Re-run validation.
5. Repeat until no unresolved issue remains in the sprint scope.

No sprint closes on "close enough". If the design is still ambiguous, the loop
is not done.

## 11. Publish Gate Rule

Publishing stays blocked until all of the following are true:

- query/follow contracts and runtime are shipped
- OTLP attachment is shipped as a public API
- UTC-only timestamps are enforced in code
- the final review reports no blocking findings
- `docs/release-readiness-checklist.md` is fully and truthfully complete
