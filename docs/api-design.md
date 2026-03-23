# SC-Observability API Design

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`

This is the only active design document in the worktree.

`requirements.md` and `architecture.md` are intentionally absent. They will be
written only after this API design is reviewed and accepted.

## 1. Purpose

Define the standalone public API surface for a reusable observability workspace
for Rust applications and CLIs.

The workspace must support:

1. structured logging
2. OpenTelemetry export
3. typed producer observations routed to multiple downstream consumers

The design must remain generic and reusable. Application-specific event models
and compatibility behavior belong in consumer-owned crates, not in the core
observability repo.

## 2. Core Architecture

The architecture is observation-first.

A producer emits one canonical observation. The observability system routes that
observation to one or more downstream consumers.

```text
producer
  -> Observability
     -> subscribers for typed observations
     -> log projectors -> Logger -> log sinks
     -> telemetry projectors -> Telemetry -> OTLP exporters
```

This is intentionally not sink-first.

The producer does not separately emit:

- a log message
- an OTEL signal
- a typed domain event

Instead, the producer emits one observation and the observability system fans it
out.

## 3. Decision Summary

- The producer-facing observation-routing API is one `Observability` service.
- Producers emit typed observations through that service.
- Logging and OTEL are downstream projections of observations.
- Structured logging remains a primary output surface.
- OTEL export remains a primary output surface.
- The lightweight logging crate stays usable on its own without observation
  routing or OTEL dependencies.
- Logging and OTEL are separate infrastructure services behind the
  observation-routing layer.
- Diagnostics are first-class structured data shared by logs, telemetry, and
  CLI rendering.
- Remediation metadata is mandatory in structured diagnostics.
- The core schema remains generic.
- `team`, `agent`, `subagent_id`, and `session_id` are not in the initial core
  schema.
- `service` is the application or tool identity in the core schema.
- Hostname and pid are auto-populated by default, with override allowed.
- Timestamps are UTC-only in the shared contract.
- Caller mistakes fail fast with `Result::Err`.
- Sink/exporter failures during normal emission are fail-open and reflected in
  health and dropped counters.
- Daemon fan-in, socket contracts, spool merge, and runtime-home discovery are
  explicitly out of scope.

## 3.1 Required Baseline Updates

This design targets a 4-crate workspace:

- `sc-observability-types`
- `sc-observability`
- `sc-observability-otlp`
- `sc-observe`

Required baseline updates before implementation begins:

- the main-repo `requirements.md` and `architecture.md` baseline currently
  describe a 3-crate shape and must be updated to the 4-crate workspace
- [`requirements.md`](requirements.md)
  and
  [`architecture.md`](architecture.md)
  must both be written to reflect the 4-crate workspace rather than the older
  3-crate shape
- workspace `Cargo.toml` must add `sc-observe` as a member
- `sc-observe` depends on `sc-observability`, `sc-observability-otlp`, and
  `sc-observability-types`
- `sc-observe` must not introduce any `agent-team-mail-*` dependencies

## 4. Design Goals

- Let a producer emit one canonical observation.
- Route that observation to logs, telemetry, and custom subscribers.
- Keep the core types generic enough for multiple unrelated consumers.
- Make structured diagnostics reusable across CLI, logging, and telemetry.
- Preserve fail-open observability behavior for runtime backend failures.
- Support logging-only, telemetry-only, or combined adoption.
- Leave room for future optional typed extension helpers without contaminating
  the base schema.

## 5. Non-Goals

This repo does not own:

- daemon log fan-in
- socket-based logging
- spool and merge semantics
- runtime-home discovery
- application-specific event taxonomies
- application-specific typed metadata in the initial core contract
- CLI success/error envelopes
- process exit-code conventions
- ATM mailbox/plugin/session contracts

## 6. Crate Roles

### 6.1 `sc-observability-types`

Owns neutral shared contracts only.

Owns:

- diagnostic types
- log, span, and metric data contracts
- observation routing traits and helper types
- health-report contracts
- generic config/value types shared across surfaces

Must not own:

- file sinks
- background workers
- transport implementations
- ATM compatibility helpers
- application-specific event types

### 6.2 `sc-observability`

Owns lightweight local structured logging infrastructure.

Owns:

- `Logger`
- log sinks
- file sink implementation
- validation and redaction for log events
- sink fan-out
- sink health and dropped-event accounting

Design intent:

- this is the minimal logging crate for basic CLI applications
- no OTEL dependency is required
- no observation bus is required
- no heavy runtime or large subscriber graph is required

Must not own:

- OTLP transport
- typed observation routing
- ATM-specific metadata rules
- ATM path conventions

### 6.3 `sc-observe`

Owns typed observation routing and projection.

Owns:

- `Observability`
- observation emitter interfaces
- subscriber registry
- projector registry
- routing from typed observations into logging and telemetry outputs
- top-level health aggregation across the observation runtime

Design intent:

- this is the heavier runtime crate
- applications use this when one emitted observation should fan out to logs,
  telemetry, and typed subscribers
- this crate may depend on `sc-observability` and `sc-observability-otlp`
- v1 scope is intentionally limited to registration, filtering, projection, and
  fan-out
- v1 does not need a large general-purpose workflow engine beyond those routing
  responsibilities

Must not own:

- application-specific observation types
- ATM-specific compatibility behavior

### 6.4 `sc-observability-otlp`

Owns remote telemetry infrastructure.

Owns:

- `Telemetry`
- OTLP exporters
- OTLP transport concerns
- batching, retry, timeout, flush, shutdown
- exporter health and dropped-export accounting

Must not own:

- local file logging
- ATM-specific metadata rules
- ATM compatibility behavior

## 6.5 Dependency Direction

Recommended dependency direction:

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

- a basic CLI may depend only on `sc-observability`
- applications that need observation routing depend on `sc-observe`
- OTEL remains optional and isolated in `sc-observability-otlp`

## 7. Producer-Facing Model

The producer-facing model is based on observations.

### 7.1 `Observability`

`Observability` is the top-level service the producer interacts with.

It belongs in `sc-observe`, not in the lightweight logging crate.

Design direction:

```rust
pub struct Observability { /* opaque */ }
pub struct ObservabilityBuilder { /* opaque */ }

impl Observability {
    pub fn new(config: ObservabilityConfig) -> Result<Self, InitError>;
    pub fn builder(config: ObservabilityConfig) -> ObservabilityBuilder;
    pub fn emit<T>(&self, observation: Observation<T>) -> Result<(), ObservationError>
    where
        T: Observable;
    pub fn flush(&self) -> Result<(), FlushError>;
    pub fn shutdown(&self) -> Result<(), ShutdownError>;
    pub fn health(&self) -> ObservabilityHealthReport;
}

impl ObservabilityBuilder {
    pub fn register_subscriber<T>(self, registration: SubscriberRegistration<T>) -> Self
    where
        T: Observable;
    pub fn register_projection<T>(self, registration: ProjectionRegistration<T>) -> Self
    where
        T: Observable;
    pub fn build(self) -> Result<Observability, InitError>;
}
```

This is the only producer-facing observation emission path in the design.

Rule:

- calling `emit()` after `shutdown()` is invalid behavior
- calling `emit()` after `shutdown()` returns `Err(ObservationError)` with the
  named shutdown semantic case `ObservationError::Shutdown` or an equivalent
  `Diagnostic.code`
- this lifecycle rule is semantic only in this design doc; `Observability` is
  not parameterized by typestate here

Observation emission error inventory:

- `ObservationError::Shutdown`
  recoverable: no
  meaning: caller attempted to emit after shutdown; this variant carries no
  `ErrorContext`
- `ObservationError::QueueFull`
  recoverable: yes
  meaning: the observation runtime could not accept more work within configured
  capacity; this variant carries `ErrorContext`
- `ObservationError::RoutingFailure`
  recoverable: depends on caller policy
  meaning: the observation could not be routed to any active or eligible
  subscriber/projector path; this variant carries `ErrorContext`

### 7.2 `ObservabilityConfig`

`ObservabilityConfig` is the top-level configuration passed to `Observability::new`.

Design direction:

```rust
pub struct ObservabilityConfig {
    pub tool_name: String,
    pub log_root: std::path::PathBuf,
    pub env_prefix: String,
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
    pub otel: Option<OtelConfig>,
}
```

Field semantics:

- `tool_name` — identity of the calling tool or service; used as the log subdirectory name and as the default `service` field in log events and telemetry
- `log_root` — absolute path to the root logging directory; the caller is responsible for providing this; no runtime-home discovery is performed
- `env_prefix` — prefix for environment variable overrides (e.g. `"OTEL"` for standard OTel names, or a tool-specific prefix); must not be ATM-specific in generic deployments
- `queue_capacity` — capacity of the internal async event queue; controls backpressure before dropping
- `rotation` — log rotation policy applied to the built-in file sink
- `otel` — optional OTLP telemetry configuration; when `None`, telemetry is disabled and `TelemetryHealthState` is `Disabled`

Composition rules inside `sc-observe`:

- `sc-observe` derives an internal `LoggerConfig` from `ObservabilityConfig`
- `LoggerConfig.service_name = ObservabilityConfig.tool_name`
- `LoggerConfig.log_root = ObservabilityConfig.log_root`
- `LoggerConfig.queue_capacity = ObservabilityConfig.queue_capacity`
- `LoggerConfig.rotation = ObservabilityConfig.rotation`
- `LoggerConfig.level`, `retention`, `redaction`, and `process_identity` use
  documented `sc-observe` defaults unless those knobs are exposed separately in
  a future expansion of `ObservabilityConfig`
- when `ObservabilityConfig.otel` is `Some(...)`, `sc-observe` derives an
  internal `TelemetryConfig` using `tool_name` as `service_name`
- this derivation rule exists for `sc-observe` composition only; it does not
  remove the direct standalone construction path for `LoggerConfig` in
  `sc-observability` or `TelemetryConfig` in `sc-observability-otlp`

> **Note**: Field names may be refined at implementation time. The intent and semantics of each field are fixed by this design.

Registrations are config-time only:

- subscriber and projector registrations are passed through
  `ObservabilityBuilder` or equivalent construction-time configuration
- registration closes at `Observability::new(...)`
- no runtime registration after construction is part of v1

### 7.3 `ObservabilityHealthReport`

The observation runtime should expose a thin aggregate health view rather than a
separate complex subsystem.

Design direction:

```rust
pub enum ObservationHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

pub struct ObservabilityHealthReport {
    pub state: ObservationHealthState,
    pub dropped_observations_total: u64,
    pub subscriber_failures_total: u64,
    pub projection_failures_total: u64,
    pub logging: Option<LoggingHealthReport>,
    pub telemetry: Option<TelemetryHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}
```

Rules:

- this is an aggregate runtime view for `sc-observe`
- it summarizes routing failures separately from downstream logging and telemetry
  health
- it does not replace `LoggingHealthReport` or `TelemetryHealthReport`

### 7.4 Producer Injection Traits

Producer crates should depend on narrow injected interfaces rather than always
depending on the concrete service types directly.

Recommended traits:

```rust
mod sealed_emitters {
    pub trait Sealed {}
}

pub trait ObservationEmitter<T>: sealed_emitters::Sealed + Send + Sync
where
    T: Observable,
{
    fn emit(&self, observation: Observation<T>) -> Result<(), ObservationError>;
}

pub trait LogEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError>;
}

pub trait SpanEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_span(&self, span: SpanSignal) -> Result<(), EventError>;
}

pub trait MetricEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_metric(&self, metric: MetricRecord) -> Result<(), EventError>;
}
```

Implementation expectations:

- `Observability` implements `ObservationEmitter<T>`
- `Logger` implements `LogEmitter`
- `Telemetry` implements `LogEmitter`, `SpanEmitter`, and `MetricEmitter` where
  appropriate

Recommended usage:

- most application code should inject `ObservationEmitter<T>` for its typed
  observations
- lower-level or specialized code may inject `LogEmitter` or telemetry emitters
  directly when it is intentionally producing projected signals

- `ObservationEmitter<T>` is sealed; it is not intended for external
  implementation. Adding methods is non-breaking.
- `LogEmitter` is sealed; it is not intended for external implementation.
  Adding methods is non-breaking.
- `SpanEmitter` is sealed; it is not intended for external implementation.
  Adding methods is non-breaking.
- `MetricEmitter` is sealed; it is not intended for external implementation.
  Adding methods is non-breaking.

### 7.5 `Observable`

Typed producer observations implement or satisfy an `Observable` contract.

Design direction:

```rust
pub trait Observable: Send + Sync + 'static {}
```

This is intentionally minimal. The core routing system should not require every
application event type to embed observability details directly into the event
definition.

`Observable` is intentionally open. Consumer crates implement it for their own
payload types.

### 7.6 `Observation<T>`

`Observation<T>` is the standard envelope emitted through the routing system.

The shared repo owns the envelope. Consumer crates own the payload type `T`.

Design direction:

```rust
pub struct Observation<T>
where
    T: Observable,
{
    pub version: String,
    pub timestamp: Timestamp,
    pub service: String,
    pub identity: ProcessIdentity,
    pub trace: Option<TraceContext>,
    pub payload: T,
}
```

Rules:

- all producer-facing observation emission uses `Observation<T>`, not raw `T`
- `version` identifies the shared observation envelope schema version, not the
  consumer payload schema version
- shared process and trace metadata live on the envelope, not duplicated in each
  consumer payload
- consumer crates remain free to define payload fields specific to their domain

### 7.7 Observations vs Projections

An observation is the canonical producer-side signal.

A projection is a derived representation of that observation for a specific
output surface, such as:

- structured log event
- OTEL span
- OTEL metric
- direct typed subscriber callback

This distinction is the core architectural rule of the repo.

## 8. Core Shared Types

### 8.1 `ErrorCode`

`ErrorCode` is a stable string-like type, not a global enum shared across all
consumers.

Design direction:

```rust
pub struct ErrorCode(std::borrow::Cow<'static, str>);

impl ErrorCode {
    pub const fn new_static(code: &'static str) -> Self;
    pub fn as_str(&self) -> &str;
}
```

Required rule:

- codes use `SCREAMING_SNAKE_CASE`
- codes include a producer or crate namespace prefix

Recommended ownership pattern:

- each crate/application owns its codes in one source module
- that module exports constants
- that module exposes a registry/list for reporting and docs generation

Example:

```rust
pub mod error_codes {
    use sc_observability_types::ErrorCode;

    pub const CONFIG_INVALID: ErrorCode =
        ErrorCode::new_static("SC_COMPOSE_CONFIG_INVALID");
    pub const TEMPLATE_NOT_FOUND: ErrorCode =
        ErrorCode::new_static("SC_COMPOSE_TEMPLATE_NOT_FOUND");

    pub const ALL: &[ErrorCode] = &[
        CONFIG_INVALID,
        TEMPLATE_NOT_FOUND,
    ];
}
```

### 8.2 `Remediation`

Remediation is mandatory in structured diagnostics.

Design direction:

```rust
pub struct RecoverableSteps { /* private fields */ }

pub enum Remediation {
    Recoverable(RecoverableSteps),
    NotRecoverable { justification: String },
}

impl Remediation {
    pub fn recoverable(
        first: impl Into<String>,
        rest: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self;
}
```

Rules:

- `Recoverable` construction is only through `Remediation::recoverable(...)`
- `Recoverable` must contain at least one concrete step
- `NotRecoverable` must contain a justification
- diagnostics without remediation metadata are invalid

### 8.3 `Diagnostic`

`Diagnostic` is the shared structured error contract.

Design direction:

```rust
pub struct Diagnostic {
    pub code: ErrorCode,
    pub message: String,
    pub cause: Option<String>,
    pub remediation: Remediation,
    pub docs: Option<String>,
    pub details: serde_json::Map<String, serde_json::Value>,
}
```

Semantics:

- `message`: what happened
- `cause`: why it happened
- `remediation`: what to do next, or why recovery is not possible
- `docs`: stable reference URL or identifier
- `details`: extra structured context

### 8.4 `Level`

```rust
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub enum LevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}
```

### 8.5 `Timestamp`

The shared timestamp contract is UTC-only.

Design direction:

```rust
pub type Timestamp = time::OffsetDateTime;
```

Requirements:

- all timestamp values are normalized to `time::UtcOffset::UTC`
- timestamps are stored and serialized in UTC
- serialization uses RFC3339 UTC form
- serialization is stable
- comparison semantics are stable
- local time conversion is a rendering concern, not a wire/storage concern
- human-readable console rendering may optionally format local time, but that
  must not change the canonical stored or emitted UTC timestamp

This closes the timestamp type choice for the shared crates.

### 8.6 `ProcessIdentity`

Hostname and pid are part of the core event contract and are auto-populated by
default.

Design direction:

```rust
pub struct ProcessIdentity {
    pub hostname: Option<String>,
    pub pid: Option<u32>,
}
```

### 8.7 `ProcessIdentityPolicy`

Design direction:

```rust
pub enum ProcessIdentityPolicy {
    Auto,
    Fixed {
        hostname: Option<String>,
        pid: Option<u32>,
    },
    Resolver(std::sync::Arc<dyn ProcessIdentityResolver>),
}

pub trait ProcessIdentityResolver: Send + Sync {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError>;
}
```

`ProcessIdentityResolver` is intentionally open for consumer implementation.
Changes to its signature are breaking.

Rationale:

- most consumers want automatic hostname/pid population
- some environments need a more meaningful pid than the immediate current
  process
- that override belongs in a resolver hook, not in core application-specific
  ancestry logic

### 8.8 `TraceContext`

`TraceContext` expresses correlation and causal position.

Design direction:

```rust
pub struct TraceId(String);
pub struct SpanId(String);
pub struct TraceIdError;
pub struct SpanIdError;

impl TraceId {
    pub fn new(value: impl Into<String>) -> Result<Self, TraceIdError>;
    pub fn as_str(&self) -> &str;
}

impl SpanId {
    pub fn new(value: impl Into<String>) -> Result<Self, SpanIdError>;
    pub fn as_str(&self) -> &str;
}

pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
}
```

Meaning:

- `trace_id`: the broader related operation tree
- `span_id`: the current operation node
- `parent_span_id`: the parent node when nested

This is how logs, spans, and related facts are connected.

Rule:

- `TraceContext` is limited to generic W3C-style trace correlation only
- request/session/runtime/application metadata must not be added here
- `TraceId` validates 32-char lowercase hex W3C trace IDs at construction
- `SpanId` validates 16-char lowercase hex W3C span IDs at construction

### 8.9 `StateTransition`

`StateTransition` expresses a discrete state change.

Design direction:

```rust
pub struct StateTransition {
    pub entity_kind: String,
    pub entity_id: Option<String>,
    pub from_state: String,
    pub to_state: String,
    pub reason: Option<String>,
    pub trigger: Option<String>,
}
```

Meaning:

- `entity_kind`: what changed, such as `task`, `subagent`, `test_run`
- `entity_id`: which specific entity changed
- `from_state` and `to_state`: the edge itself
- `reason`: why the change happened
- `trigger`: what action or event caused it

## 9. Projection Types

These are the generic output-side data contracts.

### 9.1 `LogEvent`

Design direction:

```rust
pub struct LogEvent {
    pub version: String,
    pub timestamp: Timestamp,
    pub level: Level,
    pub service: String,
    pub target: String,
    pub action: String,
    pub message: Option<String>,
    pub identity: ProcessIdentity,
    pub trace: Option<TraceContext>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub outcome: Option<String>,
    pub diagnostic: Option<Diagnostic>,
    pub state_transition: Option<StateTransition>,
    pub fields: serde_json::Map<String, serde_json::Value>,
}
```

Notes:

- `service` is the application/tool identity
- `target` is the subsystem/category namespace
- `action` is the stable event name
- `state_transition` is optional and event-first
- `fields` is the generic extension map

Excluded from the initial core schema:

- `team`
- `agent`
- `subagent_id`
- `session_id`

### 9.2 `SpanStatus`

```rust
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}
```

### 9.3 `SpanState`

Span lifecycle should be encoded in the producer-facing type system.

Runtime/serialized state:

```rust
pub enum SpanState {
    Started,
    Ended,
}
```

Important rule:

- a plain runtime enum is not enough to make illegal transitions a compilation
  error
- compile-time lifecycle guarantees require typestate or equivalent
  state-specific types

Recommended producer-facing markers:

```rust
pub struct SpanStarted;
pub struct SpanEnded;
```

### 9.4 `SpanRecord`

Design direction:

```rust
pub struct SpanRecord<S> { /* private fields */ }

impl SpanRecord<SpanStarted> {
    pub fn new(
        timestamp: Timestamp,
        service: String,
        name: String,
        trace: TraceContext,
        attributes: serde_json::Map<String, serde_json::Value>,
    ) -> Self;

    pub fn end(
        self,
        status: SpanStatus,
        duration_ms: u64,
    ) -> SpanRecord<SpanEnded>;
}

impl<S> SpanRecord<S> {
    pub fn timestamp(&self) -> Timestamp;
    pub fn service(&self) -> &str;
    pub fn name(&self) -> &str;
    pub fn trace(&self) -> &TraceContext;
    pub fn status(&self) -> SpanStatus;
    pub fn diagnostic(&self) -> Option<&Diagnostic>;
    pub fn attributes(&self) -> &serde_json::Map<String, serde_json::Value>;
}

impl SpanRecord<SpanEnded> {
    pub fn duration_ms(&self) -> u64;
}
```

Rules:

- `SpanRecord<SpanStarted>` has the only public constructor
- `SpanRecord<SpanEnded>` has no public constructor and is only reachable via
  `SpanRecord<SpanStarted>::end(...)`
- `SpanRecord<SpanStarted>` has no public duration accessor
- `SpanRecord<SpanEnded>` must carry a final duration and exposes it only via
  `duration_ms()`
- producer APIs should expose only valid transitions per state
- runtime `SpanState` is derived from the typestate parameter `S` during
  serialization and export rather than stored as a public producer-facing field

This keeps span lifecycle correctness in the type system while still supporting
generic serialization and export.

### 9.5 `SpanEvent`

`SpanEvent` represents an in-span fact attached to an existing span context.

Design direction:

```rust
pub struct SpanEvent {
    pub timestamp: Timestamp,
    pub trace: TraceContext,
    pub name: String,
    pub attributes: serde_json::Map<String, serde_json::Value>,
    pub diagnostic: Option<Diagnostic>,
}
```

### 9.6 `SpanSignal`

`SpanSignal` is the projection-time abstraction for trace output.

Design direction:

```rust
pub enum SpanSignal {
    Started(SpanRecord<SpanStarted>),
    Event(SpanEvent),
    Ended(SpanRecord<SpanEnded>),
}
```

Rules:

- in-span events use `SpanSignal::Event`
- child spans are represented by additional `Started`/`Ended` signals whose
  `TraceContext.parent_span_id` points at the parent span
- one observation family may legitimately project all three signal kinds

### 9.7 `MetricKind`

```rust
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}
```

### 9.8 `MetricRecord`

Design direction:

```rust
pub struct MetricRecord {
    pub timestamp: Timestamp,
    pub service: String,
    pub name: String,
    pub kind: MetricKind,
    pub value: f64,
    pub unit: Option<String>,
    pub attributes: serde_json::Map<String, serde_json::Value>,
}
```

`MetricRecord` does not carry a full `Diagnostic` in the initial design.

### 9.9 `DiagnosticSummary`

Design direction:

```rust
pub struct DiagnosticSummary {
    pub code: Option<ErrorCode>,
    pub message: String,
    pub at: Timestamp,
}
```

### 9.10 Public Error Type Pattern

Public crate-surface errors should be structured around diagnostics.

Design direction:

```rust
mod sealed {
    pub trait Sealed {}
}

pub trait DiagnosticInfo: sealed::Sealed {
    fn diagnostic(&self) -> &Diagnostic;
}

pub struct ErrorContext { /* private fields */ }

impl ErrorContext {
    pub fn new(
        code: ErrorCode,
        message: impl Into<String>,
        remediation: Remediation,
    ) -> Self;
    pub fn cause(self, cause: impl Into<String>) -> Self;
    pub fn docs(self, docs: impl Into<String>) -> Self;
    pub fn detail(self, key: impl Into<String>, value: serde_json::Value) -> Self;
    pub fn source(self, source: impl std::error::Error + Send + Sync + 'static) -> Self;
}

pub struct InitError(pub ErrorContext);
pub enum ObservationError {
    Shutdown,
    QueueFull(ErrorContext),
    RoutingFailure(ErrorContext),
}
pub enum TelemetryError {
    Shutdown,
    ExportFailure(ErrorContext),
}
pub struct EventError(pub ErrorContext);
pub struct FlushError(pub ErrorContext);
pub struct ShutdownError(pub ErrorContext);
pub struct ProjectionError(pub ErrorContext);
pub struct SubscriberError(pub ErrorContext);
pub struct LogSinkError(pub ErrorContext);
pub struct ExportError(pub ErrorContext);
pub struct IdentityError(pub ErrorContext);
```

Required pattern:

- most public API errors are named newtypes around `ErrorContext`
- `ObservationError` and `TelemetryError` are enums because they need named
  shutdown/runtime guard variants
- all public API errors implement `std::error::Error` and `Display`
- errors that always carry diagnostics implement `DiagnosticInfo`
- `ObservationError` and `TelemetryError` expose optional diagnostic access only
  on their contextual variants
- `DiagnosticInfo` is defined in `sc-observability-types` and sealed there
- named error newtypes in each crate implement `DiagnosticInfo` by delegating to
  their inner `ErrorContext`
- `ObservationError::Shutdown` and `TelemetryError::Shutdown` do not carry
  `ErrorContext` and therefore do not implement `DiagnosticInfo` directly
- stable machine/actionable meaning is carried by `Diagnostic.code`, not by a
  growing public enum surface
- callers may render the diagnostic directly for CLI output and also attach it
  to logs and spans
- `ErrorContext` is not directly constructible without `Remediation`
- canonical `Display` delegates to `Diagnostic` and prints message, cause when
  present, and remediation steps or non-recoverable justification

Rule:

- fail-fast caller/input errors should be returned directly
- fail-open backend failures should still be recorded through diagnostics and
  health reporting even when they do not fail the producer's core path

## 10. Observation Subscribers and Projectors

The routing layer supports two concepts:

- subscribers for typed observations
- projectors that map typed observations into logging and telemetry outputs

### 10.1 Typed Subscribers

Design direction:

```rust
pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    fn handle(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
}
```

These subscribers receive the original typed observation envelope, not a
projected log or telemetry record.

`T` is fixed at each `Arc<dyn ...<T>>` site. Object erasure is over the
concrete subscriber/projector implementation, not over the observation type.
There is no `Arc<dyn ObservationSubscriber>` erased over `T`.

`ObservationSubscriber<T>` is intentionally open. External crates may implement
it to add custom observation routing.
This trait must remain object-safe for `Arc<dyn ObservationSubscriber<T>>`.

### 10.2 Log Projectors

Design direction:

```rust
pub trait LogProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_logs(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<LogEvent>, ProjectionError>;
}
```

`LogProjector<T>` is intentionally open.
The `T` clarification from §10.1 applies here as well.
This trait must remain object-safe for `Arc<dyn LogProjector<T>>`.

### 10.3 Span Projectors

Design direction:

```rust
pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError>;
}
```

Where:

```rust
pub enum SpanSignal {
    Started(SpanRecord<SpanStarted>),
    Event(SpanEvent),
    Ended(SpanRecord<SpanEnded>),
}
```

`SpanProjector<T>` is intentionally open.
The `T` clarification from §10.1 applies here as well.
This trait must remain object-safe for `Arc<dyn SpanProjector<T>>`.

### 10.4 Metric Projectors

Design direction:

```rust
pub trait MetricProjector<T>: Send + Sync
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}
```

`MetricProjector<T>` is intentionally open.
The `T` clarification from §10.1 applies here as well.
This trait must remain object-safe for `Arc<dyn MetricProjector<T>>`.

### 10.5 Registration and Filtering

`sc-observe` owns per-type registration and filtering for subscribers and
projectors.

Design direction:

```rust
pub trait ObservationFilter<T>: Send + Sync
where
    T: Observable,
{
    fn accepts(&self, observation: &Observation<T>) -> bool;
}

pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    pub subscriber: std::sync::Arc<dyn ObservationSubscriber<T>>,
    pub filter: Option<std::sync::Arc<dyn ObservationFilter<T>>>,
}

pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    pub log_projector: Option<std::sync::Arc<dyn LogProjector<T>>>,
    pub span_projector: Option<std::sync::Arc<dyn SpanProjector<T>>>,
    pub metric_projector: Option<std::sync::Arc<dyn MetricProjector<T>>>,
    pub filter: Option<std::sync::Arc<dyn ObservationFilter<T>>>,
}
```

`ObservationFilter<T>` is intentionally open.
This trait must remain object-safe for `Arc<dyn ObservationFilter<T>>`.

Rules:

- registrations are supplied at construction time through
  `ObservabilityBuilder` or equivalent config wiring
- `SubscriberRegistration<T>` and `ProjectionRegistration<T>` are construction
  inputs and are expected to be `Send + Sync`
- routing is per observation payload type
- filtering is part of runtime registration, not producer burden
- one observation may fan out to multiple subscribers and multiple projectors
- matching registrations are invoked in deterministic registration order
- one subscriber or projector failure must not prevent later matching
  registrations from running
- if no active or eligible subscriber/projector path remains for an observation,
  emission returns `ObservationError::RoutingFailure`
- v1 `sc-observe` scope stops at registration, filtering, projection, and
  fan-out

### 10.6 Why This Split Exists

This split supports the complicated but common pattern where one typed producer
event needs to:

- be logged
- maybe start or end a span
- maybe produce metrics
- also go to one or more typed subscribers

without requiring the producer to emit those outputs separately.

## 11. Structured Logging Surface (`sc-observability`)

The logging surface is a service with pluggable sinks.

### 11.1 `LoggerConfig`

Design direction:

```rust
pub struct LoggerConfig {
    pub service_name: String,
    pub log_root: std::path::PathBuf,
    pub level: LevelFilter,
    pub queue_capacity: usize,
    pub rotation: RotationPolicy,
    pub retention: RetentionPolicy,
    pub redaction: RedactionPolicy,
    pub process_identity: ProcessIdentityPolicy,
}
```

### 11.2 Built-In Path Layout

The built-in file sink uses this default layout:

```text
<log_root>/<service_name>/logs/<service_name>.log.jsonl
```

This is the prescribed default path for the built-in file sink.

The log root must be redirectable by environment helper for tests and controlled
execution environments, with explicit config taking precedence over env.

### 11.3 `RotationPolicy`

```rust
pub struct RotationPolicy {
    pub max_bytes: u64,
    pub max_files: u32,
}
```

### 11.4 `RetentionPolicy`

```rust
pub struct RetentionPolicy {
    pub max_age_days: u32,
}
```

### 11.5 `RedactionPolicy`

```rust
pub trait Redactor: Send + Sync {
    fn redact(&self, key: &str, value: &mut serde_json::Value);
}

pub struct RedactionPolicy {
    pub denylist_keys: Vec<String>,
    pub redact_bearer_tokens: bool,
    pub custom_redactors: Vec<std::sync::Arc<dyn Redactor>>,
}
```

Rules:

- built-in denylist and bearer-token redaction run first
- custom redactors run after built-ins in registration order
- redaction happens before sink fan-out
- sink implementations must receive already-redacted events

### 11.6 `Logger`

Design direction:

```rust
pub struct Logger { /* opaque */ }

impl Logger {
    pub fn new(config: LoggerConfig) -> Result<Self, InitError>;
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError>;
    pub fn flush(&self) -> Result<(), FlushError>;
    pub fn shutdown(&self) -> Result<(), ShutdownError>;
    pub fn health(&self) -> LoggingHealthReport;
}
```

### 11.7 `LogSink`

Design direction:

```rust
pub trait LogSink: Send + Sync {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError>;
    fn flush(&self) -> Result<(), LogSinkError>;
    fn health(&self) -> SinkHealth;
}
```

`LogSink` is intentionally open.
This trait must remain object-safe for `Arc<dyn LogSink>`.

Built-in sink:

- `JsonlFileSink`

The sink model is intentionally open-ended. Consumers may compose:

- file sink only
- file plus console sink
- file plus custom stream sink
- filtered sink chains

This surface is designed to remain lightweight enough for basic CLI use without
pulling in observation routing or OTEL runtime machinery.

V1 built-in sink scope:

- JSONL file sink
- human-readable console sink
- fan-out across multiple sinks

Anything more specialized should build on the sink interfaces rather than
expanding the core lightweight crate aggressively.

### 11.8 Sink Registration and Filtering

The logging service owns sink registration, fan-out, and optional filtering.

Design direction:

```rust
pub struct SinkRegistration {
    pub sink: std::sync::Arc<dyn LogSink>,
    pub filter: Option<std::sync::Arc<dyn LogFilter>>,
}

pub trait LogFilter: Send + Sync {
    fn accepts(&self, event: &LogEvent) -> bool;
}
```

Rules:

- one event may fan out to multiple sinks
- sinks may receive all events or only filtered subsets
- filtering is sink-local policy, not producer burden
- the logger service owns sink invocation order and failure handling

### 11.9 Logging Failure Model

Rules:

- invalid log events return `EventError`
- sink failures after validation are fail-open
- sink failures update health and dropped counters
- sink failures do not fail the caller's core command flow
- no panic-based contract is implied

### 11.10 Logging Health

Design direction:

```rust
pub enum LoggingHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

pub enum SinkHealthState {
    Healthy,
    DegradedDropping,
    Unavailable,
}

pub struct SinkHealth {
    pub name: String,
    pub state: SinkHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

pub struct LoggingHealthReport {
    pub state: LoggingHealthState,
    pub dropped_events_total: u64,
    pub active_log_path: std::path::PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub last_error: Option<DiagnosticSummary>,
}
```

## 12. Telemetry Surface (`sc-observability-otlp`)

In v1, the telemetry surface is OTLP-backed and lives in
`sc-observability-otlp`.

### 12.1 `TelemetryConfig`

Design direction:

```rust
pub struct TelemetryConfig {
    pub service_name: String,
    pub resource: ResourceAttributes,
    pub transport: OtelConfig,
    pub logs: Option<LogsConfig>,
    pub traces: Option<TracesConfig>,
    pub metrics: Option<MetricsConfig>,
}
```

Composition rule:

- when `sc-observe` enables telemetry, it derives `TelemetryConfig` from
  `ObservabilityConfig.otel`
- `TelemetryConfig.service_name = ObservabilityConfig.tool_name`
- `TelemetryConfig.resource` is initialized from default resource attributes and
  the service identity
- `TelemetryConfig.transport = ObservabilityConfig.otel.unwrap()`
- `TelemetryConfig.logs`, `traces`, and `metrics` are initialized from `sc-observe`
  defaults unless the observation runtime later exposes those knobs explicitly
- this derivation rule does not remove the direct standalone `TelemetryConfig`
  construction path in `sc-observability-otlp`

### 12.1.1 `OtelConfig`

The initial OTEL transport configuration should carry forward the proven core
shape from the existing `agent-team-mail` implementation, but without any
ATM-specific env naming baked into the shared API.

Design direction:

```rust
pub enum OtlpProtocol {
    HttpBinary,
    HttpJson,
    Grpc,
}

pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub protocol: OtlpProtocol,
    pub auth_header: Option<String>,
    pub ca_file: Option<std::path::PathBuf>,
    pub insecure_skip_verify: bool,
    pub timeout_ms: u64,
    pub debug_local_export: bool,
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}
```

Initial intent of each field:

- `enabled`: master switch for OTEL export behavior
- `endpoint`: collector endpoint base URL
- `protocol`: typed transport selector
- `auth_header`: optional prebuilt auth header
- `ca_file`: optional custom CA bundle
- `insecure_skip_verify`: debug/controlled-environment TLS override
- `timeout_ms`: per-export timeout budget
- `debug_local_export`: optional local debug export path
- `max_retries`: bounded retry attempts
- `initial_backoff_ms`: initial retry backoff
- `max_backoff_ms`: maximum retry backoff

This shape is a baseline, not a promise that every current field name is final.
The design intent is to preserve the proven transport knobs while neutralizing
the old ATM-specific surface.

Rule:

- invalid OTLP transport configuration, including unsupported protocol values,
  is detected at `Telemetry::new(...)` and returns `InitError`

### 12.2 `ResourceAttributes`

```rust
pub struct ResourceAttributes {
    pub attributes: serde_json::Map<String, serde_json::Value>,
}
```

### 12.3 `Telemetry`

Design direction:

```rust
pub struct Telemetry { /* opaque */ }

impl Telemetry {
    pub fn new(config: TelemetryConfig) -> Result<Self, InitError>;
    pub fn emit_log(&self, event: &LogEvent) -> Result<(), TelemetryError>;
    pub fn emit_span(&self, span: &SpanSignal) -> Result<(), TelemetryError>;
    pub fn emit_metric(&self, metric: &MetricRecord) -> Result<(), TelemetryError>;
    pub fn flush(&self) -> Result<(), FlushError>;
    pub fn shutdown(&self) -> Result<(), ShutdownError>;
    pub fn health(&self) -> TelemetryHealthReport;
}
```

Telemetry receives `SpanSignal` values but exports completed spans only after
assembly.

Rule:

- calling `emit_log()`, `emit_span()`, or `emit_metric()` after `shutdown()`
  returns `Err(TelemetryError::Shutdown)`
- this lifecycle rule is semantic only in this design doc; no telemetry handle
  typestate is required here

### 12.4 Exporter Traits

```rust
pub struct CompleteSpan {
    pub record: SpanRecord<SpanEnded>,
    pub events: Vec<SpanEvent>,
}

pub struct SpanAssembler { /* opaque */ }

impl SpanAssembler {
    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError>;
    pub fn flush_incomplete(&mut self) -> usize;
}

pub trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

pub trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

pub trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}
```

Rules:

- `SpanAssembler` buffers `SpanSignal::Started`
- `SpanAssembler` attaches subsequent `SpanSignal::Event` items to the active
  span by `span_id`
- `SpanAssembler` emits `CompleteSpan` only on `SpanSignal::Ended`
- in-flight started spans without a matching end are dropped at flush/shutdown
  and counted in telemetry dropped-export accounting
- `LogExporter`, `TraceExporter`, and `MetricExporter` are intentionally open
- exporter traits must remain object-safe for `Arc<dyn ...>` usage

### 12.5 Telemetry Failure Model

Rules:

- invalid telemetry emission inputs return `TelemetryError::ExportFailure(...)`
- exporter failures after validation are fail-open
- exporter failures update health and dropped counters
- exporter failures do not fail the caller's core command flow
- no panic-based contract is implied

### 12.6 Telemetry Health

```rust
pub enum TelemetryHealthState {
    Disabled,
    Healthy,
    Degraded,
    Unavailable,
}

pub enum ExporterHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

pub struct ExporterHealth {
    pub name: String,
    pub state: ExporterHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

pub struct TelemetryHealthReport {
    pub state: TelemetryHealthState,
    pub dropped_exports_total: u64,
    pub exporter_statuses: Vec<ExporterHealth>,
    pub last_error: Option<DiagnosticSummary>,
}
```

## 13. Observation Pattern for Spans, Events, and Metrics

The canonical pattern is:

- spans represent long-running work units and causal structure
- log events represent discrete facts and state transitions
- metrics represent aggregate counts, gauges, and distributions

### 13.1 Canonical Rule

If the system needs to answer:

- "what changed?" use an event
- "how long did this work take?" use a span
- "how often or how much?" use a metric

### 13.2 State Transitions

State transitions are event-first.

Recommended pattern:

- project a `LogEvent` with `action = "state_transition"`
- attach a typed `StateTransition`
- include trace context when the transition occurred during a larger work unit
- optionally derive metrics from the same observation

Metrics do not replace the event record, and spans do not replace the
transition fact.

### 13.3 Sub-Agents

Recommended pattern:

- project one span lifecycle per sub-agent run
- project `SpanRecord<SpanStarted>` on the relevant start observation
- project `SpanRecord<SpanEnded>` on the relevant end observation
- project tool calls inside that run as child spans when meaningful
- project important facts, retries, warnings, and transitions as events inside
  the same trace context

### 13.4 Tasks

Recommended pattern:

- project one span lifecycle per task execution when the task has meaningful
  duration
- project `state_transition` events for lifecycle changes
- project child spans or child event sequences for nested work under the task
- project metrics for counts, outcomes, and durations

### 13.5 Test Runs

Recommended pattern:

- project one span lifecycle per test run
- optionally project child spans per suite, shard, or execution group
- project transition/failure events during the run
- project metrics for counts, failures, and duration distributions

## 14. Example Pattern: Consumer-Owned `AgentInfoEvent`

This repo should not define `AgentInfoEvent`.

That type belongs in a consumer-owned crate, such as ATM.

Example conceptual pattern:

- ATM defines `AgentInfoEvent`
- ATM creates an `Observability` runtime from `sc-observe`
- ATM registers:
  - one or more typed subscribers for `AgentInfoEvent`
  - a log projector for `AgentInfoEvent`
  - a span projector for `AgentInfoEvent`
  - a metric projector for `AgentInfoEvent`
- ATM emits one `Observation<AgentInfo>`
- observability fans that out to all relevant outputs

This is a canonical example for the design, not a side note.

The shared repo must treat this pattern as a required proving case for the
architecture.

Required example characteristics:

- a consumer-owned typed payload such as `AgentInfo`
- a shared `Observation<T>` envelope carrying timestamp, service, process
  identity, and optional trace context
- variant-based event typing for hook-like lifecycle events
- variant-specific metadata payloads
- one emitted observation routed to:
  - typed subscribers
  - structured log projection
  - span projection when the variant represents span lifecycle or nested work
  - metric projection when the variant represents countable or duration-bearing
    activity
- support for span attributes, in-span events, and child-span projection from
  the same observation family
- support for typestate-safe span lifecycle projection where invalid transitions
  are unrepresentable

Recommended conceptual shape in a consumer crate:

```rust
pub struct AgentInfo {
    pub agent_id: String,
    pub event: AgentInfoEvent,
}

pub enum AgentInfoEvent {
    SubagentStart {
        agent_type: String,
        args: Option<serde_json::Value>,
    },
    SubagentEnd {
        outcome: String,
    },
    ToolUse {
        tool: String,
        args: serde_json::Value,
        duration_ms: Option<u64>,
    },
    StateTransition {
        from: String,
        to: String,
        reason: Option<String>,
    },
}
```

This exact consumer-owned pattern should be represented:

- in the design
- in integration tests
- in at least one working example fixture used to validate the architecture

The shared crates should prove this pattern works without taking ownership of
the ATM event type itself.

## 14.1 Required Working Example

The repo should include a working example and corresponding integration test
fixture that demonstrates all four layers:

- `sc-observability-types` contracts
- `sc-observability` lightweight logging
- `sc-observe` observation routing
- `sc-observability-otlp` telemetry projection/export

The example must prove that one consumer-owned typed observation can fan out to:

- one or more typed subscribers
- one or more log sinks
- OTEL spans when appropriate
- OTEL metrics when appropriate

Recommended emission shape in that example:

```rust
let observation = Observation {
    version: "v1".to_string(),
    timestamp: now_utc(),
    service: "atm".to_string(),
    identity: ProcessIdentity {
        hostname: Some("host-a".to_string()),
        pid: Some(4242),
    },
    trace: Some(trace_context),
    payload: agent_info,
};

observability.emit(observation)?;
```

## 15. Diagnostics and CLI Integration

The shared crates reinforce good CLI error behavior without owning the outer CLI
response envelope.

One `Diagnostic` should be usable for:

- terminal rendering
- `--json` error rendering
- log event attachment
- span attachment
- health summaries

Recommended application-layer JSON envelope:

```json
{
  "success": false,
  "error": {
    "code": "SC_COMPOSE_CONFIG_INVALID",
    "message": "Config file is invalid",
    "cause": "unknown field `templte` at line 14",
    "remediation": {
      "kind": "recoverable",
      "steps": [
        "Rename `templte` to `template` in sc-compose.toml",
        "Run `sc-compose validate` again"
      ]
    },
    "docs": "https://docs.example.com/sc-compose/config"
  }
}
```

That envelope is recommended, not owned by this repo.

## 16. Environment Loading Policy

The configuration model is explicit-first.

Rules:

- explicit config is primary
- environment-based loading is optional convenience
- explicit config overrides environment
- environment overrides platform/default root resolution
- no ATM-specific env names may appear in generic APIs

### 16.1 Logging Env Policy

The built-in file logger must support redirecting the log root via env helper,
especially for tests and controlled environments.

### 16.2 Telemetry Env Policy

Telemetry env loading must support:

- standard OTel-compatible names
- custom application prefixes

Recommended direction:

```rust
pub enum TelemetryEnvMode {
    Disabled,
    StandardOtel,
    CustomPrefix(String),
}
```

Recommended standard-name mapping for the OTEL transport config includes:

- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL`
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_EXPORTER_OTLP_CERTIFICATE`
- `OTEL_EXPORTER_OTLP_INSECURE`
- `OTEL_EXPORTER_OTLP_TIMEOUT`

Custom-prefix loading may expose an equivalent neutral set for application-owned
config surfaces.

## 17. Extension Strategy

The initial core schema remains generic.

Application-domain metadata belongs in:

- typed consumer observations
- `fields`
- `attributes`

If repeated demand appears across multiple consumers, optional typed extension
helpers may be added later.

Constraint:

- future typed extensions must remain optional
- they must not become required parts of the base schema

## 18. Explicit Rejections from the Prior Design

The standalone API must not reintroduce:

- daemon-owned canonical file writing
- producer-to-daemon socket contracts
- generic spool-write and merge behavior
- runtime-home path derivation
- ATM-specific correlation fields in the core schema
- ATM-specific env prefixes in public core APIs
- health models coupled to one CLI command surface
- transport logic embedded in the local logging crate

## 19. Remaining Work

The main architecture and public API shape are now settled.

Remaining work after design review is implementation work, not unresolved
direction:

- translate this design into definitive `requirements.md`
- translate this design into definitive `architecture.md`
- add `sc-observe` to the workspace and implement the crate boundaries
- build the required `AgentInfo`-style proving example and integration tests

## 20. Review Checklist

This draft is ready for review against these questions:

- Is the observation-first architecture correct?
- Is `Observability` the right producer-facing service?
- Is the shared `Observation<T>` envelope the right producer-facing contract?
- Is the subscriber/projector split correct?
- Is per-type registration and filtering in `sc-observe` the right routing
  model?
- Is deterministic registration-order dispatch the right default?
- Are logging and telemetry correctly modeled as downstream output surfaces?
- Is the 4-crate split correct?
- Is `sc-observability` lightweight enough for basic CLI logging?
- Is `sc-observe` the right place for observation routing and pub/sub?
- Is the core type model minimal enough?
- Is mandatory remediation the right shared diagnostic contract?
- Is the span/event/metric pattern correct for sub-agents, tasks, and test runs?
- Is the service-with-pluggable-sinks model right for logging?
- Is the OTLP-backed telemetry surface acceptable for v1?
- Is the `AgentInfoEvent` example the right boundary for consumer-owned typed
  observations?
- Is the `AgentInfoEvent` pattern defined strongly enough to serve as a required
  implementation test case?
