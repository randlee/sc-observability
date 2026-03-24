# SC-Observability Implementation Plan

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`architecture.md`](./architecture.md)
- [`api-design.md`](./api-design.md)
- [`project-plan.md`](./project-plan.md)
- [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)
- [`test-strategy.md`](./test-strategy.md)
- [`sprint-plan.md`](./sprint-plan.md)

## 1. Purpose

This document turns the approved design into an implementation-ready build
plan. It answers four questions:

1. what gets built first
2. what each crate must ship before the next crate starts
3. what tests are required at each milestone
4. what ATM-specific work stays outside the shared workspace

## 2. Implementation Rules

- Implement in dependency order:
  `sc-observability-types -> sc-observability -> sc-observe -> sc-observability-otlp`
- Do not start a higher-layer public API before the lower-layer public API is
  compileable and reviewed.
- Do not introduce ATM-specific types or `agent-team-mail-*` dependencies into
  the shared workspace.
- Keep all public error codes in one `error_codes.rs` per crate.
- Keep all non-trivial shared constants in one `constants.rs` per crate.
- No magic-number policy/config literals outside constants modules.
- Keep the API checklist current as implementation lands.
- All sprint plans must preserve the standalone boundary defined by
  `docs/requirements.md`, `docs/architecture.md`, `docs/git-workflows.md`, and
  `docs/publishing.md` as required by `docs/project-plan.md`.

## 3. Milestone Order

### M0. Workspace Baseline

Goal:
- ensure the 4-crate workspace builds cleanly with the current skeletons

Required outputs:
- workspace members and dependency order correct
- per-crate `constants.rs` and `error_codes.rs` present
- boundary CI green

Exit criteria:
- `cargo check --workspace --all-targets` passes
- `bash scripts/ci/validate_repo_boundaries.sh` passes

### M1. `sc-observability-types`

Goal:
- finalize the shared contracts and validation logic

Required outputs:
- validated name newtypes
- `ErrorCode`, `Diagnostic`, `Remediation`, `ErrorContext`
- `TraceId`, `SpanId`, `TraceContext`
- `Observation<T>`, `LogEvent`, `SpanRecord<S>`, `SpanSignal`, `MetricRecord`
- shared health models
- shared open traits
- serde coverage for all public data contracts

Implementation notes:
- finish constructors, accessors, and invariants
- keep typestate-only span lifecycle
- preserve object safety on open traits used behind `Arc<dyn ...>`

Exit criteria:
- all public shared types compile without placeholder fields
- unit tests cover validation, serialization, and error rendering
- API checklist marks `sc-observability-types` as finalized

### M2. `sc-observability`

Goal:
- ship lightweight structured logging with no routing or OTLP dependency

Required outputs:
- `LoggerConfig::default_for(...)`
- `Logger`
- `LogSink`, `LogFilter`, `SinkRegistration`
- JSONL file sink
- human-readable console sink
- redaction pipeline
- rotation/retention behavior
- logging health reporting

Implementation notes:
- default path layout must match docs
- validation failures return `EventError`
- sink failures stay fail-open and update health/drop counters

Exit criteria:
- logger works with file sink only
- logger works with file + console fan-out
- redaction runs before sink fan-out
- logging-only integration test passes without `sc-observe`

### M3. `sc-observe`

Goal:
- ship typed observation routing layered on logging

Required outputs:
- `ObservabilityConfig`
- `ObservabilityBuilder`
- `Observability`
- registration of subscribers and projectors
- per-type routing and filtering
- top-level health aggregation
- sealed `ObservationEmitter<T>`

Implementation notes:
- registration remains construction-time only
- per-type routing order must be deterministic
- no direct dependency on `sc-observability-otlp`

Exit criteria:
- one typed observation can fan out to:
  - one or more subscribers
  - one or more log projectors
  - one or more metric/span projectors
- post-shutdown emission returns `ObservationError::Shutdown`
- no runtime registration after `build()`

### M4. `sc-observability-otlp`

Goal:
- ship OTLP-backed telemetry layered on `sc-observe`

Required outputs:
- `TelemetryConfigBuilder`
- `Telemetry`
- `OtelConfig`, `OtlpProtocol`
- `SpanAssembler`
- `CompleteSpan`
- `LogExporter`, `TraceExporter`, `MetricExporter`
- telemetry health

Implementation notes:
- `TelemetryConfig` stays independent of `ObservabilityConfig`
- attachment happens through projector registration
- incomplete spans are only exported after assembly completes

Exit criteria:
- logs, spans, and metrics can be attached through builder registration
- `Telemetry::new(...)` rejects invalid OTLP config eagerly
- emit methods return `TelemetryError::Shutdown` after `shutdown()` is called
- all in-flight spans are flushed or explicitly dropped and counted before
  shutdown completes
- flush and shutdown behavior matches the documented telemetry lifecycle

### M5. ATM Adapter Integration

Goal:
- prove that ATM can adopt the shared workspace without shared-repo design
  churn

Required outputs:
- ATM-owned adapter work in the ATM repo
- mapping from ATM payloads to `Observation<T>` and projections
- env/config translation
- health JSON projection
- durability behavior outside shared crates

Shared-repo proving outputs:
- ATM boundary example remains green
- shared docs stay aligned with ATM adapter docs

Exit criteria:
- ATM adapter mapping spec accepted by the ATM team with no open blocking items
- ATM proving path exercises logging, routing, and OTLP attachment
- no shared-repo boundary regressions

## 4. Required Deliverables Per Crate

| Crate | Required code | Required tests | Required docs check |
| --- | --- | --- | --- |
| `sc-observability-types` | real constructors, validation, serde, errors | unit tests | public API checklist updated |
| `sc-observability` | logger + sinks + redaction + health | unit + integration tests | path/default behavior verified |
| `sc-observe` | builder + routing + filtering + health | unit + integration tests | routing rules verified |
| `sc-observability-otlp` | config + exporters + span assembly + health | unit + integration tests | attachment model verified |

## 5. Cross-Crate Acceptance Gates

The next milestone cannot start until the previous one has:

- compileable public API
- tests at the level required by [`test-strategy.md`](./test-strategy.md)
- no unresolved public API checklist items
- docs aligned with implemented behavior

See also: [`test-strategy.md`](./test-strategy.md) §6 Exit Criteria.

## 6. Out Of Scope For Shared Implementation

The following are not part of the shared implementation sprint:

- ATM daemon fan-in and direct-spool behavior
- ATM-prefixed env parsing
- ATM `LogEventV1` compatibility surfaces
- ATM health JSON projection
- any `agent-team-mail-*` runtime dependency

Those remain governed by [`atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)
and the ATM-owned adapter work.
