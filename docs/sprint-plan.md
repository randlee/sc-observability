# SC-Observability Sprint Plan

**Status**: Draft for review
**Purpose**: Sequence the current pre-publish recovery work into predictable,
reviewable sprints.

## 1. Expected Sprint Count

Plan for 5 recovery sprints:

1. truth reset and design freeze
2. shared contract hardening
3. logging query/follow runtime
4. routing and OTLP attachment closure
5. hardening and final publish gate

This is the required order for the current recovery phase. Combining any of
these should be treated as schedule compression and must be justified
explicitly.

## 2. Sprint Breakdown

### Sprint 0: Truth Reset And Design Freeze

Scope:
- make planning docs and release gates truthful
- freeze the exact missing public design before implementation resumes
- settle query/follow semantics, OTLP attachment shape, and UTC timestamp
  enforcement

Done means:
- [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) is the
  controlling plan
- release-readiness claims match reality
- no open naming or behavior ambiguity remains for the missing public APIs

### Sprint 1: Shared Contract Hardening

Scope:
- finish the missing `sc-observability-types` query/follow contract
- replace the timestamp alias with a UTC-enforced public type
- add the shared health/provider surfaces required for full-stack integration

Done means:
- `LogQuery`, `LogSnapshot`, `QueryError`, `QueryHealthReport`, and query error
  codes are shipped
- `LoggingHealthReport.query` exists
- UTC-only timestamp behavior is enforced in code
- the checklist is `[x]` for Sprint 1 scope

### Sprint 2: Logging Query/Follow Runtime

Scope:
- implement the missing query/follow runtime in `sc-observability`
- ship `Logger::query`, `Logger::follow`, `LogFollowSession`, and
  `JsonlLogReader`
- prove rotation-safe historical and follow behavior

Done means:
- historical query and follow are implemented and synchronous
- rotation behavior is proven with tests
- logging-only deployments can use the full query/follow surface

### Sprint 3: Routing And OTLP Attachment Closure

Scope:
- ship the real public OTLP attachment path
- add telemetry health bridging without creating an OTLP dependency in
  `sc-observe`
- remove test-only scaffolding from the full-stack integration story

Done means:
- `TelemetryProjectors<T>` is shipped
- `ObservabilityBuilder` can expose attached telemetry health through a generic
  provider
- a downstream user can wire the full stack through shipped public APIs only

### Sprint 4: Hardening And Final Publish Gate

Scope:
- rerun the design-closure loop on the finished implementation
- update all docs and checklists to the shipped truth
- execute the final pre-publish review and release gate

Done means:
- all public API checklist items are marked finalized
- release-readiness checklist is truthful and complete
- final review returns zero blocking findings
- publish work can begin only after Sprint 4 passes

## 3. Sprint Dependencies

- Sprint 1 depends on Sprint 0
- Sprint 2 depends on Sprint 1
- Sprint 3 depends on Sprint 2
- Sprint 4 depends on Sprint 3

Query/follow dependency placement:

- shared query/follow contracts close in Sprint 1
- logging query/follow runtime closes in Sprint 2
- Sprint 3 consumes the stabilized logging/query surface; it does not redefine
  it

## 4. Review Expectations

Each sprint should end with:

- code complete
- tests complete
- docs aligned
- branch pushed
- ATM review requested

No sprint should rely on later cleanup to close a public API decision.
