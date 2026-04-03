# SC-Observability Architecture

**Status**: Approved
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`api-design.md`](./api-design.md)
- [`atm-quickstart.md`](./atm-quickstart.md)
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)

## 1. System Overview

The workspace is a layered stack, not a monolith:

```text
sc-observability-types
  shared neutral contracts only
          |
sc-observability
  lightweight logging only
          |
sc-observe
  observation routing / pub-sub / projection
          |
sc-observability-otlp
  OpenTelemetry / OTLP integration
```

The critical architectural rule is that each layer can be understood in
isolation:

- the types layer does not know about logging, routing, or OTLP behavior
- the logging layer does not know about routing or OTLP behavior
- the routing layer depends on logging but not on OTLP
- the OTLP layer builds on the lower-level infrastructure and owns all OTel
  transport concerns

## 1.1 Approval Scope

```text
APPROVED for shared-repo boundary direction / blocker closure.
NOT YET sufficient as the complete ATM migration specification.
```

The shared workspace architecture is approved when the crate boundaries,
dependency layering, and generic extension points are correct. Complete ATM
migration confidence additionally requires the ATM adapter requirements and ATM
adapter architecture documents that define the compatibility behavior outside
this repo.

## 2. Architectural Principles

- Preserve linear layering.
- Keep `sc-observability` lightweight and self-contained.
- Keep `sc-observe` generic over downstream integrations.
- Keep OpenTelemetry concerns only in `sc-observability-otlp`.
- Keep shared contracts in `sc-observability-types` neutral and ATM-free.
- Prevent higher-layer requirements from leaking downward.

## 3. Per-Crate Architecture

### 3.1 `sc-observability-types`

This crate is the shared contract layer.

Owns:

- `ErrorCode`, `Diagnostic`, `Remediation`, `ErrorContext`
- `Timestamp`, `DurationMs`
- `TraceContext`, `TraceId`, `SpanId`
- `SpanRecord<S>`, `SpanSignal`, `MetricRecord`, `LogEvent`
- `TelemetryHealthProvider`
- `LogQuery`, `LogOrder`, `LogFieldMatch`
- `LogSnapshot`, `QueryError`, `QueryHealthState`, `QueryHealthReport`
- health report contracts
- shared open traits such as `Observable`, `DiagnosticInfo`,
  subscribers, filters, and projectors

Must not own:

- sinks
- routing runtime behavior
- OTLP exporters or OpenTelemetry dependencies
- application-specific observation payloads

Important boundary:

- this crate is neutral shared vocabulary, not a behavior layer

### 3.2 `sc-observability`

This crate is the lightweight logging layer.

Owns:

- `Logger`
- `LoggerConfig`
- `LogSink`
- `JsonlFileSink`
- `ConsoleSink`
- redaction
- rotation
- `LoggingHealthReport`, `SinkHealth`, and `SinkHealthState` defined in
  `sc-observability-types`, re-exported by `sc-observability`

Runtime role:

- validate and redact `LogEvent`
- fan out to local sinks
- record sink-local health and drop behavior
- expose crate-local logging injection traits implemented by `Logger`

Must not own:

- observation routing
- subscriber registries
- OTLP transport
- OpenTelemetry dependencies

This crate must remain usable on its own by a basic CLI.

### 3.2.1 Query And Follow Extension

The query/follow feature remains part of the logging layer. It does not move
into `sc-observe`, does not depend on `sc-observability-otlp`, and does not
require an async runtime.

Type ownership is split as follows:

- `sc-observability-types` owns `LogQuery`, `LogOrder`,
  `LogFieldMatch`, `LogSnapshot`, `QueryError`,
  `QueryHealthState`, `QueryHealthReport`, and `TelemetryHealthProvider`
- `sc-observability-types` extends `LoggingHealthReport` with
  `query: Option<QueryHealthReport>`
- `sc-observability` owns `Logger::query`, `Logger::follow`,
  `LogFollowSession`, and `JsonlLogReader`

Approved public API surface for this sprint:

```rust
pub enum LogOrder {
    OldestFirst,
    NewestFirst,
}

pub struct LogFieldMatch {
    pub field: String,
    pub value: serde_json::Value,
}

pub struct LogQuery {
    pub service: Option<ServiceName>,
    pub levels: Vec<Level>,
    pub target: Option<TargetCategory>,
    pub action: Option<ActionName>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub since: Option<Timestamp>,
    pub until: Option<Timestamp>,
    pub field_matches: Vec<LogFieldMatch>,
    pub limit: Option<usize>,
    pub order: LogOrder,
}

pub struct LogSnapshot {
    pub events: Vec<LogEvent>,
    pub truncated: bool,
}

pub enum QueryError {
    InvalidQuery(Box<ErrorContext>),
    Io(Box<ErrorContext>),
    Decode(Box<ErrorContext>),
    Unavailable(Box<ErrorContext>),
    Shutdown,
}

pub enum QueryHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

pub struct QueryHealthReport {
    pub state: QueryHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

pub struct LoggingHealthReport {
    pub state: LoggingHealthState,
    pub dropped_events_total: u64,
    pub flush_errors_total: u64,
    pub active_log_path: std::path::PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub query: Option<QueryHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}

pub trait TelemetryHealthProvider: telemetry_health_provider_sealed::Sealed + Send + Sync {
    fn telemetry_health(&self) -> TelemetryHealthReport;
}

impl Logger {
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError>;
    pub fn follow(&self, query: LogQuery) -> Result<LogFollowSession, QueryError>;
}

pub struct LogFollowSession {
    /* opaque */
}

impl LogFollowSession {
    pub fn poll(&mut self) -> Result<LogSnapshot, QueryError>;
    pub fn health(&self) -> QueryHealthReport;
}

pub struct JsonlLogReader {
    /* opaque */
}

impl JsonlLogReader {
    pub fn new(active_log_path: std::path::PathBuf) -> Self;
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError>;
    pub fn follow(&self, query: LogQuery) -> Result<LogFollowSession, QueryError>;
}
```

`QueryError` is backed by the stable error-code constants
`SC_LOG_QUERY_INVALID_QUERY`, `SC_LOG_QUERY_IO`, `SC_LOG_QUERY_DECODE`,
`SC_LOG_QUERY_UNAVAILABLE`, and `SC_LOG_QUERY_SHUTDOWN` per `requirements.md`
TYP-036.

Behavioral boundaries:

- `Logger::query` and `Logger::follow` are convenience entry points over the
  logger's active JSONL path and documented rotation layout
- `JsonlLogReader` is reusable by tools that need offline inspection without a
  live logger instance
- `LogFollowSession` stays synchronous and caller-driven: no runtime-managed
  background work, async executor, or socket-style streaming surface
- `QueryError` stays in `sc-observability-types` so all logging query surfaces
  share one stable error vocabulary

### 3.3 `sc-observe`

This crate is the observation runtime layered on top of logging.

Owns:

- `Observability`
- `ObservabilityBuilder`
- `ObservabilityConfig`
- subscriber registration
- projector registration
- observation routing and fan-out
- `ObservabilityHealthReport` and `ObservationHealthState` defined in
  `sc-observability-types`, re-exported by `sc-observe`

Runtime role:

- accept `Observation<T>`
- route to typed subscribers
- project to `LogEvent`, `SpanSignal`, and `MetricRecord`
- send logs into the logging layer
- expose generic downstream extension points for higher-layer integrations
- expose crate-local observation injection traits implemented by
  `Observability`

Must not own:

- OpenTelemetry transport or OTel-specific configuration
- direct dependency on `sc-observability-otlp`
- application-specific payload taxonomies

The key point is that `sc-observe` is a routing/runtime layer, not an OTLP
layer.

### 3.4 `sc-observability-otlp`

This crate is the top-of-stack OpenTelemetry layer.

Owns:

- `Telemetry`
- `TelemetryConfig`
- `OtelConfig`
- `OtlpProtocol`
- `SpanAssembler`
- `CompleteSpan`
- `LogExporter`, `TraceExporter`, `MetricExporter`
- OTLP batching, retry, timeout, flush, and shutdown
- `TelemetryHealthReport`, `ExporterHealth`, and `ExporterHealthState` defined
  in `sc-observability-types`, re-exported by `sc-observability-otlp`

Runtime role:

- consume lower-layer projected logs, spans, and metrics
- assemble span lifecycle signals into completed exportable spans
- invoke actual OpenTelemetry/OTLP services and transports
- expose crate-local telemetry signal injection traits implemented by
  `Telemetry`

Configuration model:

- `TelemetryConfig` is constructed and owned by the application layer
- `TelemetryConfig` is passed directly to `sc-observability-otlp`
- `TelemetryConfig` is not embedded in or derived from `ObservabilityConfig`

Must not push OTLP concerns into the lower crates.

## 4. Runtime Composition

The layered design supports three normal application shapes.

### 4.1 Logging Only

```text
application -> sc-observability
```

Use when a CLI or tool needs structured logging only.

The query/follow API is part of this shape. An application may use `Logger`,
`Logger::query`, `Logger::follow`, or `JsonlLogReader` without enabling
`sc-observe` or `sc-observability-otlp`.

### 4.2 Logging + Routing

```text
application -> sc-observe -> sc-observability
```

Use when one observation should fan out to logs and typed subscribers without
any OTLP dependency.

### 4.3 Full Stack

```text
application -> sc-observability-otlp
                    |
                    v
               sc-observe
                    |
                    v
              sc-observability
```

Use when the application needs OTel export in addition to routing and logging.

### 4.4 ATM-Shaped Baseline

The shared stack's ATM-shaped out-of-the-box behavior and minimal production
configuration are documented separately in [`atm-quickstart.md`](./atm-quickstart.md).

That document is part of the shared-repo detailed design because ATM is the
first sophisticated adopter, but it does not move ATM-owned compatibility
behavior into the shared crates.

### 4.5 Rotation-Aware Query/Follow

Historical query and follow behavior operate on one logical log stream made
from:

- the active path `<log_root>/<service>/logs/<service>.log.jsonl`
- rotated siblings using the existing `.N` suffix convention

Historical query strategy:

- resolve the active file and its rotated siblings once at query start
- treat that resolved set as a point-in-time snapshot for the duration of the
  query
- scan in oldest-to-newest order for `LogOrder::OldestFirst` and in reverse for
  `LogOrder::NewestFirst`
- apply filtering before limit truncation and report truncation through
  `LogSnapshot.truncated`
- surface malformed JSONL records or contract decode failures as
  `QueryError::Decode` rather than silently dropping them

Follow strategy:

- follow sessions begin at the tail of the currently visible log set and do not
  replay historical backlog; callers needing backlog plus tail must call
  `query()` first
- `LogFollowSession` tracks the active path, file identity, and current read
  offset
- `poll()` reads appended records since the last successful poll
- if the active file shrinks or its file identity changes, the session treats
  that as rotation/truncation, reopens the new active file, and resumes from
  offset `0`
- the follow path remains poll-based and caller-driven; no async watch service
  is introduced

Validation:

- `limit = Some(0)` is invalid and returns `QueryError::InvalidQuery`
- `since > until` is invalid and returns `QueryError::InvalidQuery`
- `field_matches` use exact field-name lookup and exact JSON value equality

This strategy keeps the logging layer self-contained while still making
rotation behavior explicit enough for implementation and QA.

## 5. Producer Wiring

Producer code should be wired at the highest layer it needs:

- logging-only producers inject `Logger` or a narrow logging handle
- routing-aware producers inject `Observability`
- OTel-enabled producers compose the OTLP layer on top of `sc-observe`

The important ownership rule is:

- producers emit one canonical observation
- lower layers do not require knowledge of higher-layer transports

### 5.1 Full-Stack Attachment Model

Under the corrected layering, `sc-observability-otlp` attaches to
`sc-observe` by using the existing open projector extension points.

The attachment model is:

1. the application constructs `ObservabilityBuilder` for `sc-observe`
2. the application constructs `TelemetryConfig` independently for
   `sc-observability-otlp`
3. `sc-observability-otlp` registers its `LogProjector`, `SpanProjector`, and
   `MetricProjector` implementations with `ObservabilityBuilder`
4. `sc-observe` remains generic and routes observations through those
   registrations like any other external projector

Important boundary:

- `sc-observe` does not provide a special internal OTLP handle
- `sc-observability-otlp` plugs in through the same registration model exposed
  to other downstream projector consumers

## 6. Crate Boundary Table

| Crate | Depends On | Must Not Depend On | Public Surface Summary |
| --- | --- | --- | --- |
| `sc-observability-types` | shared support crates only | `sc-observability`, `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | shared contracts, typed identifiers, UTC timestamps, typed durations, diagnostics, shared traits including `TelemetryHealthProvider`, health type definitions, and logging query/follow value and error contracts |
| `sc-observability` | `sc-observability-types` | `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | lightweight logging, sinks, redaction, rotation, `Logger`, `JsonlLogReader`, follow session runtime, and logging health re-exports |
| `sc-observe` | `sc-observability-types`, `sc-observability` | `sc-observability-otlp`, `agent-team-mail-*` | observation routing, subscribers, projectors, top-level health re-exports |
| `sc-observability-otlp` | `sc-observability-types`, `sc-observability`, `sc-observe` | `agent-team-mail-*` | OTel/OTLP transport, telemetry services, exporters, telemetry health re-exports |

## 6.1 Query/Follow Dependency Order

The implementation dependency order for the query/follow work is:

```text
#24 LogQuery
  -> #25 QueryError
    -> (#26 historical query, #27 follow/tail, #28 query health)
      -> #29 JsonlLogReader
```

Consequences:

- the contract and error vocabulary land before runtime behavior
- issues `#26`, `#27`, and `#28` may proceed in parallel once `#24` and `#25`
  are merged
- `#29` finalizes the standalone reader after the logger-facing API and health
  behavior are already fixed
- the public signatures above must remain stable across that sequence

## 7. ADRs

### ADR-001: Observation-First Producers

- **Status**: Accepted
- **Context**: Producers should not emit separate log, span, metric, and domain-event payloads for one fact.
- **Decision**: Producers emit one canonical observation and the layered stack fans it out downstream.
- **Consequences**:
  - producer code stays simple
  - logs and OTLP remain projections of the same observation

### ADR-002: Linear Dependency Order

- **Status**: Accepted
- **Context**: The prior document set collapsed the stack by making `sc-observe` depend on both logging and OTLP layers.
- **Decision**: Enforce the linear dependency order `types <- logging <- observe <- otlp`.
- **Consequences**:
  - OTLP remains optional
  - `sc-observe` can be used without OpenTelemetry
  - lower layers stay readable without upper-layer concerns

### ADR-003: Logging Is Self-Contained

- **Status**: Accepted
- **Context**: `sc-observability` had begun to accumulate requirements from routing and OTLP.
- **Decision**: Keep `sc-observability` limited to logging concerns only.
- **Consequences**:
  - a basic CLI can adopt structured logging without extra runtime cost
  - logging requirements and architecture can be reviewed in isolation

### ADR-004: OTel Belongs Only At The Top

- **Status**: Accepted
- **Context**: OpenTelemetry transport concerns are implementation-heavy and should not pollute lower-layer APIs.
- **Decision**: All actual OpenTelemetry/OTLP dependencies and services belong in `sc-observability-otlp`.
- **Consequences**:
  - lower layers remain generic
  - OTel integration is opt-in
  - transport concerns are isolated where they belong

### ADR-005: Centralized Registries For Error Codes And Constants

- **Status**: Accepted
- **Context**: Scattered error-code definitions and inline policy numbers make review, documentation, and consistency checks harder across a multi-crate workspace.
- **Decision**: Each crate owns one dedicated error-code registry module and one dedicated constants module. Stable error codes are defined in the registry module, shared non-trivial constants are defined in the constants module, and non-trivial magic numbers are prohibited outside those definitions.
- **Consequences**:
  - reviewers have one obvious place to audit error codes per crate
  - documentation and reporting can enumerate public error codes consistently
  - policy limits, thresholds, retry counts, and similar values are named rather than hidden in inline literals
  - error-code registries remain separate from general-purpose constants so semantic stability is easier to enforce

### ADR-006: ATM Adapter Boundary

- **Status**: Accepted
- **Context**: ATM is the first and most sophisticated downstream adopter, but this repo must remain free of ATM production contracts and `agent-team-mail-*` dependencies.
- **Decision**: ATM-specific observability behavior belongs in an ATM-owned adapter boundary named `atm-observability-adapter`. Shared crates in this repo own only generic logging, routing, and OTLP infrastructure. ATM-specific contracts such as `LogEventV1`, daemon fan-in/spool compatibility, ATM-named env parsing, ATM health snapshots, and ATM-specific projector behavior move to the adapter boundary outside this repo.
- **Consequences**:
  - the shared repo remains generic and publishable without ATM coupling
  - ATM integration is still proven here through a separate example document and unpublished proving crate
  - production ATM compatibility logic is implemented in ATM-owned code, not in the shared repo

### ADR-007: Boot-Phase Observability Precedes Plugin Registration

- **Status**: Accepted
- **Context**: Early daemon and process lifecycle events occur before optional plugin or adapter context exists. Observability must be available during that boot phase.
- **Decision**: Core observability initialization happens before plugin registration or adapter-specific augmentation. Early lifecycle events must be recordable through the base logging/routing stack without requiring ATM plugin context.
- **Consequences**:
  - early startup failures remain observable
- adapters enrich the runtime after core observability is already available
- boot sequencing is explicit rather than left to implementation drift

### ADR-008: Shared Approval Is Not ATM Migration Approval

- **Status**: Accepted
- **Context**: The shared workspace can be architecturally sound while still
  leaving ATM-specific migration behavior under-specified.
- **Decision**: Treat the shared-repo document set as approval for generic crate
  boundaries and extension points only. Treat ATM migration completeness as a
  separate approval track owned by the ATM adapter documents.
- **Consequences**:
  - shared boundary cleanup can proceed without over-claiming ATM migration
    readiness
  - ATM-specific compatibility semantics remain owned by ATM adapter documents
  - review language stays precise about what has and has not been approved

### ADR-009: Boundary CI Must Enforce Shared-Repo Purity

- **Status**: Accepted
- **Context**: The shared-repo boundary can drift silently if CI only checks
  crate names and a few high-level doc strings.
- **Decision**: Boundary CI must enforce no ATM-specific imports or env reads in
  shared crates, no home/path discovery in shared crates outside generic config
  helpers, no OTLP/OpenTelemetry dependency outside `sc-observability-otlp`, and
  successful compilation of the unpublished ATM proving artifact.
- **Consequences**:
  - layer violations are caught before merge
  - ATM-specific behavior remains in the ATM-owned adapter boundary
  - the proving artifact remains executable evidence, not dead documentation

## 8. API-Design Consistency

`api-design.md` matches the corrected layering:

- `sc-observe` depends on `sc-observability-types` and `sc-observability` only
- `ObservabilityConfig` no longer owns OTLP configuration
- `TelemetryConfig` is application-constructed and passed directly to
  `sc-observability-otlp`
- OTLP attachment is expressed through projector registration with
  `ObservabilityBuilder`
- the ATM production boundary is explicitly outside this repo in
  `atm-observability-adapter`

## 9. Pre-Implementation Cleanup Status

The document set now reflects the required cleanup:

- requirement and architecture text no longer places OTLP concerns in
  `sc-observability`
- requirement and architecture text no longer requires
  `sc-observe -> sc-observability-otlp`
- OTLP integration is documented as attaching from the top of the stack rather
  than being constructed inside `sc-observe`

## 10. ATM Proving Artifact

The ATM integration proving artifacts owned by this repo are:

- [`docs/atm-adapter-example.md`](./atm-adapter-example.md)
- unpublished crate `examples/atm-adapter-example`

These exist to prove interface sufficiency only. They do not replace the
ATM-owned production adapter boundary.

They are intentionally narrower than a full ATM migration proof:

- they prove that ATM-shaped payloads and adapter-owned mapping layers can be
  wired through the shared crates without `agent-team-mail-*` dependencies
- they do not prove spool semantics, daemon fan-in merge behavior, ATM health
  JSON compatibility, or complete ATM env/config translation
