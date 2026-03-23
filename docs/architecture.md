# SC-Observability Architecture

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Source documents**:
- `api-design.md` on the design review branch
- `requirements.md` on the requirements review branch

## 1. Purpose

This document defines the runtime and crate architecture for the standalone
`sc-observability` workspace.

It explains:

- how the 4 crates fit together
- how producers emit observations
- how observations become logs, spans, metrics, and health signals
- where boundaries are enforced
- what the system explicitly does not do

## 2. Architectural Principles

The architecture is built on these principles:

- Observation-first, not sink-first
- Logging-only usage must remain lightweight
- OTLP transport must remain isolated
- Core shared types must remain generic and ATM-free
- Configuration is explicit-first
- Construction-time registration only
- Runtime backend failures are fail-open
- Invariants should be enforced by types where practical

## 3. Topology

### 3.1 Workspace Shape

The architecture targets a 4-crate workspace:

- `sc-observability-types`
- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

Dependency direction:

```text
sc-observability-types
    ↑
    ├── sc-observability
    ├── sc-observability-otlp
    └── sc-observe

sc-observe -> sc-observability
sc-observe -> sc-observability-otlp
```

Implications:

- basic CLIs can depend only on `sc-observability`
- richer applications can depend on `sc-observe`
- OTLP support remains opt-in through `sc-observability-otlp`

### 3.2 Runtime Shape

The runtime architecture is:

```text
producer
  -> Observability
     -> typed subscribers
     -> log projectors
        -> Logger
           -> log sinks
     -> span/metric projectors
        -> Telemetry
           -> SpanAssembler
           -> OTLP exporters
```

The producer emits one canonical observation. The architecture does not require
the producer to emit separate log, span, metric, and domain-event signals.

## 4. Crate Responsibilities

### 4.1 `sc-observability-types`

This crate is the shared contract layer.

Owns:

- shared observability types
- diagnostics and remediation types
- error-code abstractions
- trace and span identifiers
- observation routing traits
- shared health-report types

Must not own:

- local sinks
- routing runtime
- OTLP transport
- ATM adapters
- application-specific payload types

### 4.2 `sc-observability`

This crate is the lightweight logging layer.

Owns:

- `Logger`
- `LoggerConfig`
- `LogSink`
- built-in file sink
- built-in human-readable console sink
- sink registration and sink-local filtering
- validation and redaction of log events
- logging health and dropped-event accounting

Must not own:

- observation routing
- OTLP exporters
- daemon or socket handoff
- ATM-specific metadata behavior

### 4.3 `sc-observe`

This crate is the observation orchestration runtime.

Owns:

- `Observability`
- `ObservabilityBuilder`
- `ObservabilityConfig`
- subscriber registration
- projector registration
- routing and fan-out
- routing failure isolation
- top-level health aggregation
- derivation of internal logger/telemetry configs from `ObservabilityConfig`

Must not own:

- application-specific payload types
- local file sink implementation
- OTLP transport logic
- ATM adapters

### 4.4 `sc-observability-otlp`

This crate is the OTLP transport layer.

Owns:

- `Telemetry`
- `TelemetryConfig`
- `OtelConfig`
- OTLP exporters
- transport protocol selection
- batching, retry, timeout, flush, shutdown
- `SpanAssembler`
- exporter health and dropped-export accounting

Must not own:

- local file logging
- application-specific payload types
- ATM-specific metadata behavior

## 5. Core Data Model

### 5.1 Observation Envelope

The producer-facing unit is `Observation<T>`.

Shared envelope fields:

- `version`
- `timestamp`
- `service`
- `identity`
- `trace`
- `payload`

The envelope owns shared process and trace metadata. Consumer payloads own
domain-specific fields.

### 5.2 Diagnostics

Diagnostics are shared across:

- CLI rendering
- log events
- span events
- health summaries
- API errors

Every diagnostic contains:

- stable `ErrorCode`
- `message`
- optional `cause`
- mandatory `Remediation`
- optional docs reference
- structured details

### 5.3 Trace Correlation

Generic trace correlation is limited to:

- `TraceId`
- `SpanId`
- `parent_span_id`

`TraceContext` is intentionally W3C-style only. Request, runtime, session, and
application metadata do not belong inside it.

### 5.4 Span Lifecycle

Producer-facing span lifecycle uses typestate:

- `SpanRecord<SpanStarted>`
- `SpanRecord<SpanEnded>`

Started spans are opened through `SpanRecord<SpanStarted>::new(...)`.
Ended spans are only reachable through `.end(...)`.

Trace output uses:

- `SpanSignal::Started`
- `SpanSignal::Event`
- `SpanSignal::Ended`

OTLP export consumes:

- `CompleteSpan { record: SpanRecord<SpanEnded>, events: Vec<SpanEvent> }`

### 5.5 Metrics

Metrics remain aggregate signals and are not the canonical record of state
transitions.

Metric data includes:

- timestamp
- service
- name
- kind
- value
- unit
- attributes

## 6. Producer-Facing Architecture

### 6.1 `Observability`

`Observability` is the top-level producer-facing routing service in
`sc-observe`.

Responsibilities:

- accept observations
- route to subscribers
- route to projectors
- isolate runtime failures
- surface aggregate health

Lifecycle:

- built from `ObservabilityConfig`
- optionally constructed through `ObservabilityBuilder`
- registrations are closed at build time
- `emit()` after `shutdown()` returns `ObservationError::Shutdown`

### 6.2 `ObservabilityBuilder`

`ObservabilityBuilder` exists to make registration construction-time only.

Responsibilities:

- accept subscriber registrations
- accept projector registrations
- produce a closed `Observability` runtime

No runtime registration after construction is part of v1.

### 6.3 Emitter Traits

Producer injection traits:

- `ObservationEmitter<T>`
- `LogEmitter`
- `SpanEmitter`
- `MetricEmitter`

These traits are sealed because they wrap the internal runtime’s queueing and
routing semantics. External crates consume them but do not implement them.

## 7. Routing Architecture

### 7.1 Subscriber/Projector Split

The routing layer separates:

- typed subscribers that receive the original `Observation<T>`
- projectors that derive `LogEvent`, `SpanSignal`, and `MetricRecord`

This supports one observation fan-out to multiple outputs without asking the
producer to emit each output independently.

### 7.2 Registration Model

Registrations are construction inputs.

The architecture allows:

- `SubscriberRegistration<T>`
- `ProjectionRegistration<T>`

Registrations are:

- type-specific
- `Send + Sync`
- fixed at build time
- invoked in deterministic registration order

### 7.3 Filtering

Filtering is part of registration and runtime routing, not producer burden.

The routing architecture allows:

- subscriber/projector eligibility filtering
- deterministic ordered invocation
- failure isolation across matched registrations

If no active or eligible subscriber/projector path remains for an observation,
the runtime returns `ObservationError::RoutingFailure`.

### 7.4 Object Safety

All routing traits are designed for `Arc<dyn Trait<T>>` usage.

Important invariant:

- `T` is fixed at the dynamic dispatch site
- erasure is over the concrete implementation
- there is no type-erased observation payload parameter at runtime

This is why object-safety is documented per routing trait.

## 8. Logging Architecture

### 8.1 Logging Flow

Logging flow is:

```text
LogEvent
  -> redaction/validation
  -> Logger
  -> sink fan-out
  -> file / console / custom sinks
```

### 8.2 Sink Model

The logger owns:

- sink registration
- sink-local filtering
- fan-out
- sink invocation order
- sink failure handling

Built-in v1 scope:

- JSONL file sink
- human-readable console sink
- multi-sink fan-out

### 8.3 Redaction

Redaction occurs before sink fan-out.

Order:

1. built-in denylist redaction
2. built-in bearer-token redaction
3. custom `Redactor` chain

All sinks receive already-redacted events.

### 8.4 Logging Failure Semantics

The logger splits failures into two classes:

- invalid event/input failures: return `EventError`
- sink/backend failures: fail-open and update health/drop counters

Sink failures must not block the caller’s core command flow.

## 9. Telemetry Architecture

### 9.1 Telemetry Flow

Telemetry flow is:

```text
LogEvent / SpanSignal / MetricRecord
  -> Telemetry
  -> SpanAssembler (for spans)
  -> exporters
  -> collector
```

### 9.2 Telemetry Configuration

`TelemetryConfig` is the internal configuration used by `Telemetry`.

Within `sc-observe`, it is derived from `ObservabilityConfig.otel`.
In standalone OTLP usage, it remains directly constructible in
`sc-observability-otlp`.

`OtelConfig` owns transport-specific configuration, including:

- endpoint
- `OtlpProtocol`
- auth header
- CA file
- TLS relaxation flag
- timeout
- retry/backoff settings

Invalid transport configuration fails at `Telemetry::new(...)` with
`InitError`.

### 9.3 Span Assembly

`SpanAssembler` bridges span signals and complete trace export payloads.

Responsibilities:

- buffer `SpanSignal::Started`
- attach `SpanSignal::Event` to the active span by `span_id`
- emit `CompleteSpan` only on `SpanSignal::Ended`
- drop unmatched started spans at flush/shutdown

This bridge is required because OTLP export requires a completed span plus its
events, not a stream of partial lifecycle signals.

### 9.4 Telemetry Failure Semantics

The telemetry layer splits failures into:

- invalid telemetry emission inputs: `TelemetryError::ExportFailure(...)`
- exporter/backend failures after validation: fail-open with health/drop updates
- lifecycle guard failure: `TelemetryError::Shutdown`

Telemetry emit methods after shutdown are invalid and return
`TelemetryError::Shutdown`.

## 10. Health Architecture

### 10.1 Layered Health

Health exists at three layers:

- routing/runtime health in `sc-observe`
- sink health in `sc-observability`
- exporter health in `sc-observability-otlp`

### 10.2 Routing Health

`ObservabilityHealthReport` aggregates:

- dropped observations
- subscriber failures
- projection failures
- logging health snapshot
- telemetry health snapshot
- last diagnostic summary

### 10.3 Logging Health

`LoggingHealthReport` exposes:

- top-level logging state
- dropped events total
- active log path
- per-sink status
- last diagnostic summary

`SinkHealthState` is typed and does not use stringly states.

### 10.4 Telemetry Health

`TelemetryHealthReport` exposes:

- top-level telemetry state
- dropped exports total
- per-exporter status
- last diagnostic summary

`ExporterHealthState` is typed and does not use stringly states.

## 11. Configuration Architecture

### 11.1 Explicit-First Model

The architecture is explicit-first:

- explicit config is primary
- environment loading is convenience
- explicit config overrides env
- env overrides defaults

### 11.2 `ObservabilityConfig` Derivations

Within `sc-observe`:

- `ObservabilityConfig` derives internal `LoggerConfig`
- `ObservabilityConfig.otel` derives internal `TelemetryConfig`

This composition rule exists only inside `sc-observe`. It does not remove
standalone lower-level crate construction paths.

### 11.3 Environment Modes

Telemetry environment loading supports:

- standard OTel names
- custom prefixes

The architecture explicitly forbids ATM-specific env names in the generic
public API.

## 12. Extension Model

### 12.1 Open Extension Points

Open extension points:

- `Observable`
- `ProcessIdentityResolver`
- `ObservationSubscriber<T>`
- `ObservationFilter<T>`
- `LogProjector<T>`
- `SpanProjector<T>`
- `MetricProjector<T>`
- `LogSink`
- `LogExporter`
- `TraceExporter`
- `MetricExporter`
- `Redactor`

These are the places where consumer-owned behavior is expected.

### 12.2 Sealed Interfaces

Sealed interfaces:

- `ObservationEmitter<T>`
- `LogEmitter`
- `SpanEmitter`
- `MetricEmitter`
- `DiagnosticInfo`

These are sealed to protect runtime invariants and error-contract invariants.

## 13. Consumer-Owned Event Pattern

The architecture must support consumer-owned typed observations such as
`AgentInfo`.

Canonical proving case:

- consumer defines payload type and event enum
- consumer emits one `Observation<AgentInfo>`
- subscribers receive the typed observation
- log projectors emit `LogEvent`
- span projectors emit `SpanSignal`
- telemetry assembles spans and exports them
- metrics derive from the same observation family

This pattern proves the system supports:

- one observation to many outputs
- child spans
- in-span events
- typed subscribers
- typed metrics/logs/spans from the same payload family

## 14. Excluded Architecture

This architecture explicitly excludes:

- daemon fan-in
- socket handoff
- spool/merge semantics
- runtime-home discovery
- ATM-specific core fields
- ATM session/runtime normalization
- process supervision

## 15. Implementation Sequence

Recommended implementation order:

1. finalize and merge API design
2. finalize and merge requirements
3. finalize and merge architecture
4. add `sc-observe` to the workspace
5. align `sc-observability-types` with the full shared type model
6. remove OTLP-facing leakage from the lightweight logging crate
7. implement logging surface against the new contracts
8. implement routing runtime
9. implement OTLP surface and span assembly
10. build the required consumer-owned proving example and integration tests
