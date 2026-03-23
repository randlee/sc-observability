# SC-Observability Architecture

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Source documents**:
- `api-design.md` on the design review branch
- `requirements.md` on the requirements review branch

## 1. System Overview

The standalone `sc-observability` workspace is a 4-crate system:

```text
                          +---------------------------+
                          |  sc-observability-types   |
                          | shared contracts/types    |
                          +------------+--------------+
                                       |
                  +--------------------+--------------------+
                  |                                         |
      +-----------v-----------+                 +-----------v-------------+
      |   sc-observability    |                 | sc-observability-otlp   |
      | lightweight logging   |                 | OTLP transport/export   |
      +-----------+-----------+                 +-----------+-------------+
                  ^                                         ^
                  |                                         |
                  +--------------------+--------------------+
                                       |
                          +------------v--------------+
                          |        sc-observe         |
                          | observation runtime       |
                          | routing/subscribers       |
                          | projectors/fan-out        |
                          +---------------------------+
```

At a glance:

| Crate | Owns |
| --- | --- |
| `sc-observability-types` | Shared types, identifiers, diagnostics, health contracts, routing and emitter traits |
| `sc-observability` | Lightweight structured logging, sinks, redaction, rotation, logging health |
| `sc-observe` | `Observability`, builder/config, routing, subscribers, projectors, top-level health |
| `sc-observability-otlp` | `Telemetry`, `SpanAssembler`, OTLP config, exporters, retry/flush/shutdown |

The system is observation-first:

- producers emit one `Observation<T>`
- `sc-observe` fans that observation out to typed subscribers and projectors
- projectors derive logs, spans, and metrics
- logging and OTLP are downstream consumers, not producer-facing primary APIs

This architecture does not require:

- an ATM daemon
- socket handoff
- spool/merge semantics
- ATM-specific event types in core crates

## 2. Component Architecture

### 2.1 Producer Layer

The producer-facing runtime is `Observability` in `sc-observe`.

Core types:

- `Observability`
- `ObservabilityBuilder`
- `ObservabilityConfig`
- `Observation<T>`
- sealed emitter traits:
  - `ObservationEmitter<T>`
  - `LogEmitter`
  - `SpanEmitter`
  - `MetricEmitter`

Responsibilities:

- accept canonical observations from producers
- own construction-time registration of subscribers and projectors
- derive internal `LoggerConfig` and `TelemetryConfig`
- coordinate routing, failure isolation, and health aggregation

### 2.2 Routing Layer

The routing layer lives in `sc-observe`.

Core types:

- `ObservationSubscriber<T>`
- `ObservationFilter<T>`
- `LogProjector<T>`
- `SpanProjector<T>`
- `MetricProjector<T>`
- `SubscriberRegistration<T>`
- `ProjectionRegistration<T>`

Responsibilities:

- receive `Observation<T>` for a fixed `T`
- evaluate optional registration filters
- invoke matching registrations in deterministic registration order
- isolate failures so one subscriber/projector failure does not block later matches
- return `ObservationError::RoutingFailure` when no eligible path remains

Routing model:

```text
Observation<T>
  -> matching subscribers<T>
  -> matching projectors<T>
     -> LogEvent -> Logger
     -> SpanSignal -> Telemetry
     -> MetricRecord -> Telemetry
```

### 2.3 Logging Layer

The logging layer lives in `sc-observability`.

Core types:

- `Logger`
- `LoggerConfig`
- `LogSink`
- `LogFilter`
- `JsonlFileSink`
- `ConsoleSink`
- `RedactionPolicy`
- `Redactor`

Responsibilities:

- validate and redact `LogEvent`
- append to the built-in JSONL file sink
- render human-readable console output
- fan out to multiple sinks
- apply sink-local filtering
- track sink health and dropped-event counts

Logging flow:

```text
LogEvent
  -> validation
  -> redaction
  -> Logger
  -> sink fan-out
  -> file / console / custom sink
```

### 2.4 Telemetry Layer

The telemetry layer lives in `sc-observability-otlp`.

Core types:

- `Telemetry`
- `TelemetryConfig`
- `OtelConfig`
- `OtlpProtocol`
- `SpanAssembler`
- `CompleteSpan`
- `LogExporter`
- `TraceExporter`
- `MetricExporter`

Responsibilities:

- accept projected log, span, and metric data
- bridge `SpanSignal` into exportable completed spans
- batch/export OTLP payloads
- own retry, timeout, flush, and shutdown behavior
- track exporter health and dropped exports

Telemetry flow:

```text
LogEvent / SpanSignal / MetricRecord
  -> Telemetry
  -> SpanAssembler
  -> exporters
  -> collector
```

`SpanAssembler` exists because:

- producers and projectors naturally emit started spans, in-span events, and ended spans
- OTLP trace export requires a completed span and its events
- `CompleteSpan { record: SpanRecord<SpanEnded>, events: Vec<SpanEvent> }` is the export boundary

### 2.5 Type System Layer

The shared type layer lives in `sc-observability-types`.

Core types:

- `SpanRecord<S>`
- `SpanStarted`
- `SpanEnded`
- `SpanSignal`
- `TraceContext`
- `TraceId`
- `SpanId`
- `ErrorContext`
- `Diagnostic`
- `Remediation`

Responsibilities:

- encode compile-time span lifecycle safety
- keep trace correlation generic and W3C-style
- require remediation-aware diagnostics
- provide stable shared contracts without ATM coupling

Important invariants:

- only `SpanRecord<SpanStarted>` has a public constructor
- `SpanRecord<SpanEnded>` is only created via `.end(...)`
- producer-facing `SpanRecord<S>` fields are private
- `TraceContext` contains only `trace_id`, `span_id`, and `parent_span_id`
- `ErrorContext` is not directly constructible without remediation

## 3. Trait Injection Strategy

This section defines how observability is wired into producer binaries.

### 3.1 Main-Level Wire-Up

Producer binaries construct observability once near `main()` and then inject
handles into the rest of the program.

The standard pattern is:

1. build `ObservabilityConfig`
2. register subscribers/projectors with `ObservabilityBuilder`
3. build a closed `Observability` runtime
4. pass narrow emitter traits into subsystems

`sc-observe` owns internally:

- registration tables
- routing and filter execution
- logger and telemetry composition
- failure isolation
- health aggregation

The producer sees:

- `ObservationEmitter<T>` for its typed observations
- optionally `LogEmitter`, `SpanEmitter`, or `MetricEmitter` where lower-level code needs them directly

### 3.2 Explicit Injection vs Shared `Arc`

The preferred application pattern is explicit dependency injection:

- constructor args for long-lived services
- struct fields for loops/controllers
- function args for one-off helpers

`Arc` is used to share the runtime safely across async tasks and threads.

Recommended pattern:

- construct `Arc<Observability>` once
- coerce or expose narrow trait handles from that shared runtime
- clone the `Arc` into async tasks, thread workers, or agent loops

The design does not assume global mutable singletons for normal operation.

### 3.3 Async Tasks, Threads, and Agent Loops

When work is spawned:

- clone the shared handle
- move the handle into the task/thread
- emit observations from inside the worker using the same shared runtime

This keeps:

- registration fixed
- health centralized
- queueing/routing semantics owned by `sc-observe`

It avoids:

- ad hoc runtime construction inside event paths
- per-task logger/exporter setup
- duplicated routing state

### 3.4 Minimal Producer Wire-Up

```rust
use std::sync::Arc;

use sc_observability::Logger;
use sc_observability_otlp::Telemetry;
use sc_observe::Observability;
use sc_observability_types::{
    Observation,
    ObservationEmitter,
    ObservationSubscriber,
    LogProjector,
    SpanProjector,
    MetricProjector,
};

#[derive(Clone)]
struct AgentLoop {
    agent_events: Arc<dyn ObservationEmitter<AgentInfo>>,
}

impl AgentLoop {
    fn new(agent_events: Arc<dyn ObservationEmitter<AgentInfo>>) -> Self {
        Self { agent_events }
    }

    async fn run(&self, event: Observation<AgentInfo>) -> anyhow::Result<()> {
        self.agent_events.emit(event)?;
        Ok(())
    }
}

fn build_observability(config: ObservabilityConfig) -> anyhow::Result<Arc<Observability>> {
    let logger = Logger::new(LoggerConfig::from_observability(&config)?)?;
    let telemetry = config.otel.as_ref()
        .map(|otel| Telemetry::new(TelemetryConfig::from_observability(&config, otel)))
        .transpose()?;

    let observability = Observability::builder(config)
        .with_logger(logger)
        .with_telemetry(telemetry)
        .register_subscriber::<AgentInfo>(Arc::new(AgentDashboardSubscriber::new()))
        .register_projection::<AgentInfo>(ProjectionRegistration {
            log_projector: Some(Arc::new(AgentInfoLogProjector::new())),
            span_projector: Some(Arc::new(AgentInfoSpanProjector::new())),
            metric_projector: Some(Arc::new(AgentInfoMetricProjector::new())),
            filter: None,
        })
        .build()?;

    Ok(Arc::new(observability))
}

async fn main_loop(config: ObservabilityConfig) -> anyhow::Result<()> {
    let observability = build_observability(config)?;
    let agents = AgentLoop::new(observability.clone());

    // spawned tasks and loops receive cloned handles
    let background_agents = agents.clone();
    tokio::spawn(async move {
        let _ = background_agents.run(next_agent_observation().await);
    });

    Ok(())
}
```

The sketch above is illustrative:

- construction happens once
- registration happens before `build()`
- producer code emits one typed `Observation<AgentInfo>`
- routing, projection, logging, and OTLP export happen inside the runtime
- `.with_logger(...)` and `.with_telemetry(...)` are illustrative builder-time
  composition hooks, not finalized method names; implementation must align the
  concrete builder API with `api-design.md` rather than inventing incompatible
  signatures

## 4. Sealed vs Open Trait Inventory

| Trait | Crate | Sealed/Open | Rationale |
| --- | --- | --- | --- |
| `ObservationEmitter<T>` | `sc-observability-types` | Sealed | Producer-facing emit semantics are owned by the runtime; external crates consume but do not implement them |
| `LogEmitter` | `sc-observability-types` | Sealed | Same reason; prevents alternate runtime semantics leaking into the API |
| `SpanEmitter` | `sc-observability-types` | Sealed | Span emission lifecycle is owned by the runtime and telemetry wiring |
| `MetricEmitter` | `sc-observability-types` | Sealed | Metric routing semantics remain internal to the runtime |
| `Observable` | `sc-observability-types` | Open | Consumer crates must define their own typed observation payloads |
| `DiagnosticInfo` | `sc-observability-types` | Sealed | Only crate-owned error types should expose canonical diagnostic access |
| `ObservationSubscriber<T>` | `sc-observability-types` | Open | Consumers need custom typed subscribers |
| `ObservationFilter<T>` | `sc-observability-types` | Open | Consumers need custom registration filtering policies |
| `LogProjector<T>` | `sc-observability-types` | Open | Consumers must map their own observation families into logs |
| `SpanProjector<T>` | `sc-observability-types` | Open | Consumers must map their own observation families into span signals |
| `MetricProjector<T>` | `sc-observability-types` | Open | Consumers must map their own observation families into metrics |
| `ProcessIdentityResolver` | `sc-observability-types` | Open | Consumers may need custom host/pid resolution behavior |
| `Redactor` | `sc-observability-types` | Open | Logging callers may need custom redaction logic |
| `LogSink` | `sc-observability` | Open | Logging is explicitly sink-extensible |
| `LogFilter` | `sc-observability` | Open | Sink-local filtering is a supported extension point |
| `LogExporter` | `sc-observability-otlp` | Open | OTLP logging exporters remain replaceable/testable |
| `TraceExporter` | `sc-observability-otlp` | Open | Trace exporters remain replaceable/testable |
| `MetricExporter` | `sc-observability-otlp` | Open | Metric exporters remain replaceable/testable |

## 5. ADRs

### ADR-001: Observation-First Architecture

- **Status**: Accepted
- **Context**: Producers should not emit separate log, span, metric, and domain-event payloads for one fact. That duplicates work, couples producers to sinks, and makes extension awkward.
- **Decision**: Producers emit `Observation<T>` as the canonical signal. Subscribers and projectors fan that observation out to logs, spans, metrics, and custom consumers.
- **Consequences**:
  - producer code stays simple
  - logging and OTLP become downstream projections
  - one observation can drive multiple outputs consistently

### ADR-002: Typestate for `SpanRecord<S>`

- **Status**: Accepted
- **Context**: Span lifecycle errors are easy to introduce when started/ended state is represented only by runtime flags or public mutable fields.
- **Decision**: Use typestate with `SpanRecord<SpanStarted>` and `SpanRecord<SpanEnded>`, private fields, and `.end(...)` as the only transition.
- **Consequences**:
  - invalid producer-side span transitions become unrepresentable
  - compile-time guarantees reduce lifecycle bugs
  - export code still uses runtime `SpanSignal`/`CompleteSpan` where streaming assembly is needed

### ADR-003: Observability Lifecycle Typestate Deferred

- **Status**: Accepted
- **Context**: Applying typestate to the whole `Observability` runtime would complicate injection and shared-handle usage across async tasks and threads.
- **Decision**: Keep lifecycle enforcement as a semantic runtime rule for v1: `emit()` after `shutdown()` returns a named shutdown error instead of using runtime typestate wrappers.
- **Consequences**:
  - producer injection remains simple
  - lifecycle misuse is still explicit and testable
  - a future version may revisit stronger lifecycle typing if runtime ergonomics improve

### ADR-004: Config-Time Registration Only

- **Status**: Accepted
- **Context**: Runtime registration changes complicate routing determinism, health visibility, and concurrency semantics.
- **Decision**: Subscribers and projectors are registered through `ObservabilityBuilder` before the runtime is built. No runtime registration API is part of v1.
- **Consequences**:
  - routing tables are closed and deterministic
  - health and failure semantics are easier to reason about
  - dynamic plugin loading is intentionally deferred

### ADR-005: Sealed Emitter Traits

- **Status**: Accepted
- **Context**: The emitter traits represent runtime-owned queueing, routing, lifecycle, and failure semantics. External implementations would fragment those guarantees.
- **Decision**: `ObservationEmitter<T>`, `LogEmitter`, `SpanEmitter`, and `MetricEmitter` are sealed.
- **Consequences**:
  - producer injection surface stays stable
  - the runtime keeps ownership of emit semantics
  - extension happens through open subscriber/projector/sink/exporter traits instead

### ADR-006: 4-Crate Split With `sc-observe`

- **Status**: Accepted
- **Context**: A single crate cannot stay both extremely lightweight for CLI logging and rich enough for routing, projections, and OTLP transport without accumulating unwanted cost and coupling.
- **Decision**: Split the workspace into four crates: shared contracts, lightweight logging, observation runtime, and OTLP transport.
- **Consequences**:
  - basic CLIs can depend only on `sc-observability`
  - richer applications can opt into `sc-observe`
  - OTLP complexity stays isolated in `sc-observability-otlp`

### ADR-007: Zero ATM Coupling

- **Status**: Accepted
- **Context**: The extracted workspace must serve multiple Rust tools and must not preserve ATM-specific path, daemon, or payload assumptions in the shared core.
- **Decision**: The workspace has no `agent-team-mail-*` dependencies. `sc-observability-types` owns shared contracts, and ATM-specific observation types remain in ATM-owned crates.
- **Consequences**:
  - core crates stay reusable
  - ATM becomes a consumer, not a hidden dependency
  - daemon/socket/spool semantics remain out of scope for the shared workspace

## 6. Crate Boundaries

| Crate | Depends On | Must Not Depend On | Public Surface Summary |
| --- | --- | --- | --- |
| `sc-observability-types` | Rust standard ecosystem support crates only | `sc-observability`, `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | Shared value types, diagnostics, identifiers, health contracts, emitter traits, routing traits |
| `sc-observability` | `sc-observability-types` | `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | `Logger`, `LoggerConfig`, sinks, redaction, rotation, logging health |
| `sc-observe` | `sc-observability-types`, `sc-observability`, `sc-observability-otlp` | `agent-team-mail-*` | `Observability`, builder/config, routing, subscribers, projectors, top-level health |
| `sc-observability-otlp` | `sc-observability-types` | `sc-observability`, `sc-observe`, `agent-team-mail-*` | `Telemetry`, config, `SpanAssembler`, OTLP exporters, exporter health |

Boundary summary:

- `sc-observability-types` is the shared base
- `sc-observability` remains lightweight and logging-only
- `sc-observe` composes logging and telemetry but does not own their lower-level implementations
- `sc-observability-otlp` owns OTLP-specific transport/export only

## 7. Pre-Implementation Cleanup Required

- `ARCH-QA-010`: remove the `OtelConfig` re-export from the lightweight `sc-observability` surface
- `ARCH-QA-011`: replace the current `TraceRecord` / `MetricRecord` stubs in `sc-observability-types`

These cleanup items must happen before implementation begins so the codebase matches the approved crate boundaries and type ownership model.
