# SC-Observability Project Plan

## Status

This repo is in post-1.0 maintenance and downstream-integration mode.

The workspace crates are already published. The current planning focus is:

- keep code, docs, and release procedures aligned with the shipped surface
- preserve the standalone crate boundaries defined by the normative docs
- maintain consumer-facing usability and downstream integration contracts
- stage new cross-repo work behind explicit review and QA

Historical recovery and pre-publish planning documents remain valuable
reference material, but they are no longer the controlling phase for current
work.

## Near-Term Work

1. Keep repo workflow and review discipline aligned with ATM.
2. Preserve the standalone crate boundaries defined in:
   - [`requirements.md`](./requirements.md)
   - [`architecture.md`](./architecture.md)
   - [`git-workflows.md`](./git-workflows.md)
   - [`publishing.md`](./publishing.md)
3. Maintain the consumer-facing docs and examples that prove the shipped public
   API remains usable for downstream adopters.
4. Keep release and publishing docs aligned with the fact that the workspace
   crates are already published and semver-governed.
5. Maintain extraction inventory and boundary ADRs as modules move from ATM to
   the standalone repo.
6. Keep ATM-specific adapter work outside the shared crates.
7. Maintain explicit downstream integration contracts for shipped consumers so
   cross-repo reviews do not rely on inferred layering or stale assumptions.
8. Keep completed sprint records archived without leaving root `docs/`
   cluttered.
9. Preserve the completed `v1.2.0` Phase A record and route new migration,
   additive API and binding work through the proposed Phase B plan.

## Issue #70 / v1.1.0

Retained-log rotation, pruning, and maintenance was the additive `v1.1.0`
logging-layer work item for `sc-observability`.

Historical sprint plan:

- [`archive/sprints/sprint-plan-retained-log-maintenance.md`](./archive/sprints/sprint-plan-retained-log-maintenance.md)

Historical sequence:

1. S1 docs sprint:
   - update `requirements.md`, `architecture.md`, and `project-plan.md`
   - define the retained-log policy surface, maintenance ownership, health
     reporting, and definitive shutdown contract
   - complete quality review before any code lands
   Exit criteria:
   - field names and defaults match
     `archive/sprints/sprint-plan-retained-log-maintenance.md`
   - normative docs lock `RetainedLogPolicy` as a struct nested in
     `LoggerConfig`
   - `LoggingHealthReport` and retained-log worker-state ownership are defined
     consistently across docs
   - no implementation code lands before docs review passes
2. S2 implementation sprint:
   - add the retained-log policy/config surface to `sc-observability`
   - move generic rotation/pruning maintenance into the logging layer
   - add health, shutdown, and integration tests
3. S3 consumer-doc sprint:
   - update `README.md` and/or `CONSUMING.md` with a retained-log policy
     configuration example once the implementation ships

## Thread Optimization / v1.2.0 Phase A

The completed logging-runtime workstream after retained-log maintenance was
the Phase A thread optimization effort, released in `v1.2.0`.

Controlling phase folder:

- [`plans/phase-a/readiness.md`](./plans/phase-a/readiness.md)
- [`plans/phase-a/sprint-A1.md`](./plans/phase-a/sprint-A1.md)
- [`plans/phase-a/sprint-A2.md`](./plans/phase-a/sprint-A2.md)
- [`plans/phase-a/sprint-A3.md`](./plans/phase-a/sprint-A3.md)
- [`plans/phase-a/sprint-A4.md`](./plans/phase-a/sprint-A4.md)

Planned sequence:

1. `A.1` architecture-and-API lock sprint:
   - lock the writer-thread architecture, queue semantics, health surface,
     `log()` / `try_log()` / deprecated `emit()` contract, and public-API
     governance requirements in the normative docs
2. `A.2` public-API governance sprint:
   - add CI-visible semver/public-API gates so future API changes cannot land
     silently
3. `A.3` runtime-and-API implementation sprint:
   - replace the dedicated maintenance-only worker model with a queue-backed
     writer runtime and implement the approved public logging API additions
4. `A.4` consumer rollout sprint:
   - update consumer docs and examples so new adopters use `log()` /
     `try_log()` and can surface queue/writer degradation in app health or
     `doctor` output

Phase A exit criteria:

- `A.1` through `A.4` each record an accepted commit and verdict in
  `docs/plans/phase-a/readiness.md`
- the final accepted runtime uses one writer-owned queue-backed logging model
  rather than a maintenance-only background worker
- public API changes are documented and CI-visible
- consumer docs teach `log()` / `try_log()` as the preferred logging APIs

## Phase B — Log bridge publication and language bindings

The proposed next lettered phase is tracked in
[`plans/phase-b/plan-phase-b.md`](./plans/phase-b/plan-phase-b.md).
Its first migration sprint copies the review-corrected generic BTIT crates.
Explicit prerequisite sprints implement/qualify additive core runtime elevation
and obtain accepted BTIT integration before that copy. Bounded core error
migration sprints add improved methods and warning-only legacy adapters before
Rust publication, followed by separately gated shared schema (B.3),
shared native runtime (B.3b), TypeScript/Tauri (B.3a), Python runtime (B.4) and Python distribution/platform
qualification (B.4a), then Python integration/async support and binding release.
Go remains future scope. The proposal does not reopen the accepted
Phase A closure or claim that BTIT's currently open review findings are resolved.

### B.P1 — Per-logger runtime level core

B.P1 implements the additive neutral runtime-level state, weak ownership, and
admission-outcome core described by
[`plans/phase-b/sprint-b-p1-runtime-core.md`](./plans/phase-b/sprint-b-p1-runtime-core.md).
It follows the execution-authorized runtime-level contract; B.P2 develops
from B.P1's pushed, independently verified implementation, and B.P1's PR
merges before B.P2's PR, to qualify staged artifacts. QA-4 independently verified 17/18 tracked B.P1 findings at
`a8951321e6b1df6044c2ee1f6f41c07a9dad7d99`; the remaining manual governance
hold was withdrawn by coordinating lead aobs following the owner's direction
to complete Phase B with publication delayed until the end. Public-API review
status remains proposed for public API review and is owner-deferred to Phase B
completion; historical QA verdicts remain
unchanged (QA-5 scoped checks satisfied; overall historical verdict FAIL), and
B.P1 remains unmerged and not live-published. Sprint `status: complete` denotes
implementation completion, with QA and merge state tracked separately.

### B.P2 — Staged runtime-level package qualification

B.P2 selects the `1.3.0` four-crate candidate, retains deterministic `.crate`
archive checksums and normalized package inventories, and validates separate
published-baseline and extracted-candidate consumers. Retained macOS/Linux/Windows
qualification artifacts passed for the recorded staged candidate; independent QA,
merge, live publication, and B.7 registry-only re-proof remain pending. B.P2
does not publish to crates.io. B.7 owns the phase-end live release and
registry-only consumer proof.

## Rule

Any sprint plan added here must preserve the standalone boundary defined by:

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/git-workflows.md`
- `docs/publishing.md`

## Implementation Planning Set

Current maintenance and integration work should use these planning documents
together:

- `docs/pre-publish-recovery-plan.md`
- `docs/implementation-plan.md`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md`
- `docs/archive/sprints/sprint-plan.md`
- `docs/release-readiness-checklist.md`

## Historical Recovery Baseline

The earlier recovery program remains the historical baseline for these planning
principles:

1. fix correctness and best-practice gaps first
2. ship the missing approved API surface before higher-layer expansion
3. close incomplete design elements in dedicated sprints
4. repeat the design-closure loop until no unresolved issue remains

The detailed sprint-by-sprint execution record remains in
[`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) for reference.

## Consumer Usability Baseline

This follow-up work defines the minimum consumer-facing usability baseline for
the shipped public API. It remains relevant after the initial release because
downstream adopters still depend on these entrypoints and examples staying
accurate.

1. Consumer onboarding sprint (`#20`)
   Exit criteria:
   - `README.md` is a real consumer entrypoint with crate-selection guidance
     and a minimal logging-only snippet
   - root `CONSUMING.md` documents logging-only setup, default paths,
     `SC_LOG_ROOT`, sink toggles, custom sink registration, and `Logger::health()`
   - `examples/custom-sink-example/` exists and compiles against the public API only
   - consumer-facing default sink/path/environment behavior is documented
2. Default file sink path cleanup (`#21`)
   Exit criteria:
   - the default file sink layout is simplified to
     `<log_root>/logs/<service>.log.jsonl`
   - all user-facing docs, examples, and tests reflect the new layout
   - any migration note for the old nested path is documented before release
3. Console sink writer parity (`#55`)
   Exit criteria:
   - `ConsoleSink::stderr()` is added as a public companion to `stdout()`
   - the public writer-selection surface is explicitly limited to stdout/stderr
   - consumer docs include the stdout/stderr selection guidance
4. Retained-sink fault-injection sprint (`#57`)
   Exit criteria:
   - a public retained-sink fault-injection surface exists for live validation
     of `degraded` and `unavailable` sink health states
   - the hook is intentionally gated for validation use and lives in the
     retained-sink layer, not the query/follow layer
   - docs explain how downstream consumers exercise the same failure paths they
     rely on in production health checks

## Downstream Integration Documentation

The repo also needs stable downstream integration guidance for adjacent repos
that integrate against the shipped public API.

1. `sc-compose` logging-only integration contract
   Exit criteria:
   - `requirements.md` and `architecture.md` explicitly state the exact split
     between `sc-observability-types` and `sc-observability`
   - the docs explicitly scope this work to simple logging-only integration and
     explicitly defer OTel expansion
   - the docs state that `sc-composer` keeps a local observer layer and does
     not depend on `sc-observability-types`
   - the docs define the minimum local observer interface shape, event source,
     and `dyn`-compatible injection model required for the downstream adapter
   - the docs state that `sc-compose` constructs `Logger`, applies the
     file/console sink policy, uses `Logger::health()` for health reporting,
     and calls `Logger::shutdown()` on exit
   - the docs explicitly define the no-op fallback path when no observer or
     logger-backed adapter is installed
   - the docs define the adapter-owned mapping from `sc-compose` local observer
     events to `LogEvent` fields, including command lifecycle events and
     `message` guidance
   - the docs identify the planned downstream `sc-compose observability-health`
     CLI surface precisely enough for implementation and review
   - `qm-comp` cross-document consistency review passes; all three docs are
     confirmed mutually consistent before merge
