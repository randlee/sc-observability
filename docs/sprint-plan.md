# SC-Observability Sprint Plan

**Status**: Draft for review
**Purpose**: Sequence implementation into predictable, reviewable sprints.

## 1. Expected Sprint Count

Plan for 6 implementation sprints:

1. shared contracts
2. lightweight logging
3. observation routing
4. OTLP telemetry
5. ATM adapter integration
6. hardening and release readiness

This is the safer plan. Combining any of these should be treated as schedule
compression, not the default.

## 2. Sprint Breakdown

### Sprint 1: Shared Contracts

Scope:
- finish `sc-observability-types`
- finalize newtypes, diagnostics, trace/span/metric contracts, shared errors

Done means:
- public API checklist complete for `sc-observability-types`
- unit tests and serde tests green

### Sprint 2: Lightweight Logging

Scope:
- build `sc-observability`
- file sink, console sink, redaction, fan-out, health

Done means:
- logging-only example works
- fail-open sink behavior verified

### Sprint 3: Observation Routing

Scope:
- build `sc-observe`
- builder, subscriber/projector registration, filtering, routing, health

Done means:
- one typed observation fans out correctly
- shutdown/routing failure behavior verified

### Sprint 4: OTLP Telemetry

Scope:
- build `sc-observability-otlp`
- config builder, span assembler, exporters, telemetry health

Done means:
- OTLP attaches through projector registration
- telemetry lifecycle behavior verified

### Sprint 5: ATM Adapter Integration

Prerequisites:
- ATM adapter mapping spec accepted by ATM team with no open blocking items
- all Open ATM-Owned Decisions in [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)
  §9 resolved or formally deferred with documented rationale

Scope:
- ATM-owned adapter work, not shared-crate behavior
- implement mapping spec and proving path

Done means:
- ATM adapter mapping spec accepted with no open blocking items
- shared repo example and ATM repo proving path align

### Sprint 6: Hardening

Scope:
- CI strengthening
- performance pass
- migration and release readiness

Done means:
- all public API checklist items marked finalized
- no unresolved Important findings from QA-1 through QA-6
- migration and release readiness checklist drafted
- release and cutover tasks can start

## 3. Sprint Dependencies

- Sprint 2 depends on Sprint 1
- Sprint 3 depends on Sprint 2
- Sprint 4 depends on Sprint 3
- Sprint 5 depends on Sprint 4, ATM adapter mapping spec acceptance with no
  open blocking items, and resolution or formal documented deferral of the open
  ATM-owned decisions listed in `atm-adapter-mapping-spec.md` §9
- Sprint 6 depends on all previous sprints

## 4. Review Expectations

Each sprint should end with:

- code complete
- tests complete
- docs aligned
- branch pushed
- ATM review requested

No sprint should rely on “we will tighten the docs later” to close a public API
decision.
