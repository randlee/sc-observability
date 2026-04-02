# SC-Observability Implementation Plan

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`architecture.md`](./architecture.md)
- [`api-design.md`](./api-design.md)
- [`project-plan.md`](./project-plan.md)
- [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md)
- [`public-api-checklist.md`](./public-api-checklist.md)
- [`test-strategy.md`](./test-strategy.md)
- [`sprint-plan.md`](./sprint-plan.md)

## 1. Purpose

This document turns the approved design into an implementation-ready build
plan.

The current execution mode is the pre-publish recovery program in
[`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md). That recovery
plan is the controlling sequence for current work.

## 2. Implementation Rules

- Implement in dependency order unless the recovery plan explicitly calls out a
  docs-only sprint.
- Do not start higher-layer public API work before lower-layer public API gaps
  are closed.
- Do not introduce ATM-specific types or `agent-team-mail-*` dependencies into
  the shared workspace.
- Keep all public `ErrorCode` string constants in one `error_codes.rs` per
  crate.
- Keep shared cross-crate constants in
  `sc-observability-types/src/constants.rs` as the SSOT location.
- Higher-layer crates may keep a `constants.rs` file only for crate-local
  values that are not shared across crate boundaries.
- Public Rust error types remain centralized in `sc-observability-types` per
  `requirements.md` TYP-030 and are re-exported where needed.
- No magic-number policy/config literals outside constants modules.
- Keep the API checklist current as implementation lands.
- Keep the release-readiness checklist truthful at all times.
- No sprint closes on deferred documentation cleanup.

## 3. Active Recovery Milestones

### S0. Truth Reset And Design Freeze

Goal:
- make the planning docs and release gates truthful
- freeze the exact missing public design before coding resumes

Required outputs:
- controlling recovery plan published in docs
- release readiness checklist corrected
- public API checklist updated with all missing planned public items
- exact design choices frozen for query/follow semantics, OTLP attachment, and
  UTC timestamp enforcement

Exit criteria:
- no open naming or behavior ambiguity remains for S1 through S3

### S1. Shared Contract Hardening

Goal:
- close the missing shared query/follow and timestamp contract gaps in
  `sc-observability-types`

Required outputs:
- UTC-enforced `Timestamp`
- `LogOrder`, `LogFieldMatch`, `LogQuery`, `LogSnapshot`
- `QueryError`, `QueryHealthState`, `QueryHealthReport`
- query error-code constants
- `TelemetryHealthProvider`
- `LoggingHealthReport.query`

Exit criteria:
- the shared query/follow contract exists in shipped code
- UTC-only timestamps are enforced in code
- tests cover validation and serde behavior for the new shared contracts

### S2. Logging Query/Follow Runtime

Goal:
- ship the missing logging query/follow runtime in `sc-observability`

Required outputs:
- `Logger::query`
- `Logger::follow`
- `LogFollowSession`
- `JsonlLogReader`
- rotation-aware query/follow behavior
- query health population on logging health

Exit criteria:
- the logging crate ships the full approved query/follow API
- tests prove no duplicate or silently skipped committed records across
  rotation
- no async runtime or ATM-specific dependency is introduced

### S3. Routing And OTLP Attachment Closure

Goal:
- turn the OTLP attachment model into a real public integration surface

Required outputs:
- `TelemetryProjectors<T>`
- `TelemetryProjectors<T>::into_registration()`
- telemetry health provider integration for `ObservabilityBuilder`
- public full-stack wiring path with no test-only scaffolding

Exit criteria:
- downstream users can wire `sc-observe` and `sc-observability-otlp` together
  using shipped public APIs only
- `ObservabilityHealthReport.telemetry` is populated when configured

### S4. Hardening And Final Publish Gate

Goal:
- run the final production-readiness pass only after S0 through S3 are complete

Required outputs:
- all docs aligned with shipped behavior
- all checklist items finalized
- final severity-tagged review
- explicit publish/no-publish recommendation

Exit criteria:
- final review reports zero blocking findings
- release-readiness checklist is fully and truthfully complete

## 4. Required Deliverables Per Recovery Milestone

| Milestone | Required code | Required tests | Required docs check |
| --- | --- | --- | --- |
| `S0` | none required beyond docs and checklist updates | docs consistency | recovery plan becomes controlling |
| `S1` | shared query/follow contracts + UTC timestamps | unit + serde tests | API checklist updated |
| `S2` | logger query/follow runtime | unit + integration tests | runtime signatures and behavior verified |
| `S3` | OTLP attachment surface + telemetry health bridge | unit + integration tests | attachment model verified |
| `S4` | no new feature work by default; hardening only | full workspace validation | release checklist and final review aligned |

## 5. Cross-Crate Acceptance Gates

The next milestone cannot start until the previous one has:

- compileable public API for its scope
- tests at the level required by [`test-strategy.md`](./test-strategy.md)
- no unresolved checklist items in its own scope
- docs aligned with implemented behavior
- design-closure loop completed with no remaining ambiguity in scope

## 6. Out Of Scope For Shared Implementation

The following are not part of the shared implementation sprint:

- ATM daemon fan-in and direct-spool behavior
- ATM-prefixed env parsing
- ATM `LogEventV1` compatibility surfaces
- ATM health JSON projection
- any `agent-team-mail-*` runtime dependency

Those remain governed by [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)
and ATM-owned adapter work.
