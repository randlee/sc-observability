# SC-Observability API Design

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`

This document is the source design baseline for the companion
`requirements.md` and `architecture.md` documents, which now exist on their
respective review branches.

This document is aligned to the corrected layering documented in
`requirements.md` and `architecture.md`.

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
- `sc-observe`
- `sc-observability-otlp`

Required baseline updates before implementation begins:

- the main-repo `requirements.md` and `architecture.md` baseline currently
  describe a 3-crate shape and must be updated to the 4-crate workspace
- [`requirements.md`](requirements.md)
  and
  [`architecture.md`](architecture.md)
  must both be written to reflect the 4-crate workspace rather than the older
  3-crate shape
- workspace `Cargo.toml` must add `sc-observe` as a member
- the corrected layering is
  `sc-observability-types <- sc-observability <- sc-observe <- sc-observability-otlp`
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
- health-report contracts, including `LoggingHealthReport`,
  `MaintenanceHealthReport`, `MaintenanceWorkerState`, and `WriterState`
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
- routing from typed observations into logging outputs and generic downstream
  projector/subscriber extension points
- top-level health aggregation across the observation runtime

Design intent:

- this is the heavier runtime crate
- applications use this when one emitted observation should fan out to logs,
  generic projectors, and typed subscribers
- this crate depends on `sc-observability` and `sc-observability-types`
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
- `TelemetryConfig`
- OTLP exporters
- OTLP transport concerns
- batching, retry, timeout, flush, shutdown
- exporter health and dropped-export accounting

Design intent:

- this crate sits at the top of the stack
- the application constructs `TelemetryConfig` independently and passes it
  directly to `sc-observability-otlp`
- this crate attaches to `sc-observe` by registering `LogProjector`,
  `SpanProjector`, and `MetricProjector` implementations with
  `ObservabilityBuilder`

Must not own:

- local file logging
- ATM-specific metadata rules
- ATM compatibility behavior

## 6.5 Dependency Direction

Recommended dependency direction:

```text
sc-observability-types
    ↑
    └── sc-observability
          ↑
          └── sc-observe
                ↑
                └── sc-observability-otlp
```

Implications:

- a basic CLI may depend only on `sc-observability`
- applications that need observation routing depend on `sc-observe`
- OTEL remains optional and isolated in `sc-observability-otlp`
- `sc-observe` does not depend on `sc-observability-otlp`

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
    pub tool_name: ToolName,
    pub log_root: std::path::PathBuf,
    pub env_prefix: EnvPrefix,
    pub queue_capacity: usize,
    pub retained_log_policy: RetainedLogPolicy,
}
```

Field semantics:

- `tool_name` — identity of the calling tool or service; used as the log subdirectory name and as the default service name in log events and telemetry
- `log_root` — absolute path to the root logging directory; the caller is responsible for providing this; no runtime-home discovery is performed
- `env_prefix` — prefix for environment variable overrides (e.g. `"OTEL"` for standard OTel names, or a tool-specific prefix); must not be ATM-specific in generic deployments
- `queue_capacity` — capacity of the internal async event queue; controls backpressure before dropping
- `retained_log_policy` — retained-log rotation, pruning, and maintenance
  policy applied to the built-in file sink

Defaults:

- `env_prefix` defaults to the uppercase `tool_name` value with `-` and `.`
  normalized to `_`
- `queue_capacity` defaults to `1024`
- `retained_log_policy.rotation_max_bytes` defaults to `64 * 1024 * 1024`
- `retained_log_policy.rotation_max_files` defaults to `10`
- `retained_log_policy.retention_max_age` defaults to `7 days`
- `retained_log_policy.maintenance_cadence` defaults to `60s`

Recommended constructor shape:

```rust
impl ObservabilityConfig {
    pub fn default_for(
        tool_name: ToolName,
        log_root: std::path::PathBuf,
    ) -> Self;
}
```

Composition rules inside `sc-observe`:

- `sc-observe` derives an internal `LoggerConfig` from `ObservabilityConfig`
- `LoggerConfig.service_name = ServiceName::new(ObservabilityConfig.tool_name.as_str())?`
- `LoggerConfig.log_root = ObservabilityConfig.log_root`
- `LoggerConfig.queue_capacity = ObservabilityConfig.queue_capacity`
- `LoggerConfig.retained_log_policy = ObservabilityConfig.retained_log_policy`
- `LoggerConfig.level`, `redaction`, and `process_identity` use
  documented `sc-observe` defaults unless those knobs are exposed separately in
  a future expansion of `ObservabilityConfig`
- `sc-observe` does not derive or own `TelemetryConfig`
- `TelemetryConfig` is constructed independently by the application layer and
  passed directly to `sc-observability-otlp`

Registrations are config-time only:

- subscriber and projector registrations are passed through
  `ObservabilityBuilder` or equivalent construction-time configuration
- registration closes at `Observability::new(...)`
- no runtime registration after construction is part of v1

Full-stack attachment model:

- `sc-observability-otlp` attaches to the routing layer by registering its
  `LogProjector`, `SpanProjector`, and `MetricProjector` implementations with
  `ObservabilityBuilder`
- `sc-observe` remains generic and does not expose OTLP-specific internal
  attachment paths

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

One implementation-readiness correction is important here:

- open cross-crate traits remain in `sc-observability-types`
- sealed emitter traits must be crate-local to the crate that implements them

That split is necessary because a trait cannot be both sealed in the base crate
and implemented by public facade types in downstream crates without weakening
the seal.

`sc-observe` therefore owns the producer-facing sealed observation emitter:

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
```

Implementation expectation:

- `Observability` implements `ObservationEmitter<T>`

Related crate-local sealed traits:

- `sc-observability` owns `LogEmitter`
- `sc-observability-otlp` owns `SpanEmitter` and `MetricEmitter`

Recommended usage:

- most application code should inject `ObservationEmitter<T>` for its typed
  observations
- logging-only code may inject `LogEmitter`
- telemetry-specific code may inject telemetry-local signal emitter traits when
  it is intentionally producing projected signals
- `ObservationEmitter<T>` is intentionally per-type. Callers hold one handle
  per observation type; a single type-erased emitter for heterogeneous events
  is not supported by design.

`ObservationEmitter<T>` is sealed inside `sc-observe`; it is not intended for
external implementation. Adding methods is non-breaking.

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
    pub service: ServiceName,
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

`Observation.version` uses the shared constant
`OBSERVATION_ENVELOPE_VERSION = "v1"`.

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

### 8.1.1 Shared Name Newtypes

The carry-forward QA-5 newtypes belong in `sc-observability-types`.

They exist to make the public API explicit and to keep stringly-typed
configuration from spreading across the crate boundaries.

Design direction:

```rust
pub struct ToolName(String);
pub struct EnvPrefix(String);
pub struct ServiceName(String);
pub struct TargetCategory(String);
pub struct ActionName(String);
pub struct MetricName(String);
```

Ownership and usage:

- `ToolName`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `ObservabilityConfig`
  - invariant: non-empty ASCII identifier using `[A-Za-z0-9._-]+`
- `EnvPrefix`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: env-loading helpers
  - invariant: uppercase ASCII identifier using `[A-Z0-9_]+` with no trailing `_`
- `ServiceName`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `LoggerConfig`, `TelemetryConfig`, `LogEvent`, `MetricRecord`,
    `SpanRecord`
  - invariant: non-empty ASCII identifier using `[A-Za-z0-9._-]+`
- `TargetCategory`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `LogEvent.target`
  - invariant: non-empty dotted, dashed, underscored, or slash-free category
    identifier using `[A-Za-z0-9._-]+`
- `ActionName`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `LogEvent.action`
  - invariant: non-empty dotted, dashed, or underscored action identifier using
    `[A-Za-z0-9._-]+`
- `MetricName`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `MetricRecord.name`
  - invariant: non-empty metric identifier using `[A-Za-z0-9._\\-/]+`

These newtypes should expose:

```rust
impl ToolName {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError>;
    pub fn as_str(&self) -> &str;
}
```

Equivalent constructors and accessors apply to the other four newtypes.

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
pub struct Timestamp(time::OffsetDateTime);

impl Timestamp {
    pub const UNIX_EPOCH: Self;
    pub fn now_utc() -> Self;
    pub fn from_offset_date_time(value: time::OffsetDateTime) -> Self;
}
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
    /// Stable category describing what changed, such as `task` or `subagent`.
    pub entity_kind: String,
    pub entity_id: Option<String>,
    /// Previous stable state label.
    pub from_state: String,
    /// New stable state label.
    pub to_state: String,
    /// Optional human-readable explanation for why the transition occurred.
    pub reason: Option<String>,
    /// Optional action or event name that triggered the transition.
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
    pub version: SchemaVersion,
    pub timestamp: Timestamp,
    pub level: Level,
    pub service: ServiceName,
    pub target: TargetCategory,
    pub action: ActionName,
    pub message: Option<String>,
    pub identity: ProcessIdentity,
    pub trace: Option<TraceContext>,
    pub request_id: Option<CorrelationId>,
    pub correlation_id: Option<CorrelationId>,
    pub outcome: Option<OutcomeLabel>,
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

### 9.3 Span Typestate Markers

Span lifecycle should be encoded in the producer-facing type system rather than
as a public runtime enum.

Producer-facing markers:

```rust
pub struct SpanStarted;
pub struct SpanEnded;
```

### 9.4 `SpanRecord`

Design direction:

```rust
pub struct DurationMs(u64);

pub struct SpanRecord<S> { /* private fields */ }

impl SpanRecord<SpanStarted> {
    pub fn new(
        timestamp: Timestamp,
        service: ServiceName,
        name: ActionName,
        trace: TraceContext,
        attributes: serde_json::Map<String, serde_json::Value>,
    ) -> Self;

    pub fn end(
        self,
        status: SpanStatus,
        duration: DurationMs,
    ) -> SpanRecord<SpanEnded>;
}

impl<S> SpanRecord<S> {
    pub fn timestamp(&self) -> Timestamp;
    pub fn service(&self) -> &ServiceName;
    pub fn name(&self) -> &ActionName;
    pub fn trace(&self) -> &TraceContext;
    pub fn status(&self) -> SpanStatus;
    pub fn diagnostic(&self) -> Option<&Diagnostic>;
    pub fn attributes(&self) -> &serde_json::Map<String, serde_json::Value>;
}

impl SpanRecord<SpanEnded> {
    pub fn duration_ms(&self) -> Option<DurationMs>;
}
```

Rules:

- `SpanRecord<SpanStarted>` has the only public constructor
- `SpanRecord<SpanEnded>` has no public constructor and is only reachable via
  `SpanRecord<SpanStarted>::end(...)`
- `SpanRecord<SpanStarted>` has no public duration accessor
- `SpanRecord<SpanEnded>` exposes final duration only via `duration_ms()`,
  which returns `None` only for malformed deserialized data that bypassed the
  producer-facing typestate API
- producer APIs should expose only valid transitions per state
- runtime started/ended state is derived from the typestate parameter `S`
  during serialization and export rather than stored as a public
  producer-facing field

This keeps span lifecycle correctness in the type system while still supporting
generic serialization and export.

### 9.5 `SpanEvent`

`SpanEvent` represents an in-span fact attached to an existing span context.

Design direction:

```rust
pub struct SpanEvent {
    pub timestamp: Timestamp,
    pub trace: TraceContext,
    pub name: ActionName,
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
    pub service: ServiceName,
    pub name: MetricName,
    pub kind: MetricKind,
    pub value: f64,
    /// Optional UCUM unit string, for example `ms`, `By`, or `1`.
    pub unit: Option<MetricUnit>,
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

### 9.10 Shared Constants And Error Registry Modules

`sc-observability-types` should ship:

- `src/constants.rs`
  - `OBSERVATION_ENVELOPE_VERSION`
  - `TRACE_ID_LEN`
  - `SPAN_ID_LEN`
  - `DEFAULT_ENV_PREFIX_SEPARATOR`
- `src/error_codes.rs`
  - `VALUE_VALIDATION_FAILED`
  - `TRACE_ID_INVALID`
  - `SPAN_ID_INVALID`
  - `IDENTITY_RESOLUTION_FAILED`
  - `DIAGNOSTIC_INVALID`

### 9.11 Public Error Type Pattern

The published baseline uses diagnostic wrappers as described below. Phase B
proposes additive improved errors and warning-only migration in §21; these
baseline definitions are preserved, not replaced in place.

Design direction:

```rust
pub trait DiagnosticInfo: sealed::Sealed {
    fn diagnostic(&self) -> &Diagnostic;
}

pub struct ErrorContext { /* private fields */ }

impl ErrorContext {
    pub fn new(code: ErrorCode, message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn cause(self, cause: impl Into<String>) -> Self;
    pub fn docs(self, docs: impl Into<String>) -> Self;
    pub fn detail(self, key: impl Into<String>, value: serde_json::Value) -> Self;
    pub fn source(
        self,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    ) -> Self;
}

pub struct InitError(pub Box<ErrorContext>);
pub enum ObservationError {
    Shutdown,
    QueueFull(Box<ErrorContext>),
    RoutingFailure(Box<ErrorContext>),
}
pub enum TelemetryError {
    Shutdown,
    ExportFailure(Box<ErrorContext>),
}
pub struct EventError(pub Box<ErrorContext>);
pub struct FlushError(pub Box<ErrorContext>);
pub struct ShutdownError(pub Box<ErrorContext>);
pub struct ProjectionError(pub Box<ErrorContext>);
pub struct SubscriberError(pub Box<ErrorContext>);
pub struct LogSinkError(pub Box<ErrorContext>);
pub struct ExportError(pub Box<ErrorContext>);
pub struct IdentityError(pub Box<ErrorContext>);
```

Required pattern:

- most public API errors are named newtypes around `ErrorContext`
- `ObservationError` and `TelemetryError` are enums because they need named
  shutdown/runtime guard variants
- contextual public error payloads are boxed inside the error types themselves
  so public `Result<_, Error>` surfaces remain small and clippy-clean
- all public API errors implement `std::error::Error` and `Display`
- errors that always carry diagnostics implement `DiagnosticInfo`
- `ObservationError` and `TelemetryError` expose optional diagnostic access only
  on their contextual variants
- `DiagnosticInfo` is defined in `sc-observability-types` as a sealed trait;
  preserve that published seal and its existing workspace implementations
- named error newtypes in each crate implement `DiagnosticInfo` by delegating to
  their inner `ErrorContext`
- `ObservationError::Shutdown` and `TelemetryError::Shutdown` do not carry
  `ErrorContext` and therefore do not implement `DiagnosticInfo` directly
- baseline machine/actionable meaning is carried by `Diagnostic.code`; the
  proposed additive API also exposes typed classification without changing codes
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
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
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
    pub fn new(subscriber: std::sync::Arc<dyn ObservationSubscriber<T>>) -> Self;
    pub fn with_filter(self, filter: std::sync::Arc<dyn ObservationFilter<T>>) -> Self;
}

pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    pub fn new() -> Self;
    pub fn with_log_projector(self, projector: std::sync::Arc<dyn LogProjector<T>>) -> Self;
    pub fn with_span_projector(self, projector: std::sync::Arc<dyn SpanProjector<T>>) -> Self;
    pub fn with_metric_projector(self, projector: std::sync::Arc<dyn MetricProjector<T>>) -> Self;
    pub fn with_filter(self, filter: std::sync::Arc<dyn ObservationFilter<T>>) -> Self;
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

### 10.7 `sc-observe` Constants And Error Registry Modules

`sc-observe` should ship:

- `src/constants.rs`
  - `DEFAULT_OBSERVATION_QUEUE_CAPACITY`
- `src/error_codes.rs`
  - `OBSERVATION_SHUTDOWN`
  - `OBSERVATION_QUEUE_FULL`
  - `OBSERVATION_ROUTING_FAILURE`
  - `OBSERVABILITY_INIT_FAILED`
  - `OBSERVABILITY_FLUSH_FAILED`

## 11. Structured Logging Surface (`sc-observability`)

The logging surface is a service with pluggable sinks.

### 11.1 `LoggerConfig`

Design direction:

```rust
pub struct LoggerConfig {
    pub service_name: ServiceName,
    pub log_root: std::path::PathBuf,
    pub level: LevelFilter,
    pub queue_capacity: usize,
    pub retained_log_policy: RetainedLogPolicy,
    pub redaction: RedactionPolicy,
    pub process_identity: ProcessIdentityPolicy,
    pub enable_file_sink: bool,
    pub enable_console_sink: bool,
}
```

Defaults:

- `level = LevelFilter::Info`
- `queue_capacity = 1024`
- `retained_log_policy.rotation_max_bytes = ByteCount::from_mib(64)`
- `retained_log_policy.rotation_max_files = FileCount::from_usize(10)`
- `retained_log_policy.retention_max_age = RetentionMaxAge::from_days(7)`
- `retained_log_policy.maintenance_cadence = MaintenanceCadence::new(60s)`
- `retained_log_policy.writer_shutdown_timeout = WriterShutdownTimeout::new(5s)`
- `redact_bearer_tokens = true`
- `enable_file_sink = true`
- `enable_console_sink = false`

Recommended constructor shape:

```rust
impl LoggerConfig {
    pub fn default_for(
        service_name: ServiceName,
        log_root: std::path::PathBuf,
    ) -> Self;
}
```

`service_name` is required. `LoggerConfig` does not provide a shape where
service identity is absent.

### 11.2 Built-In Path Layout

The built-in file sink uses this default layout:

```text
<log_root>/logs/<service_name>.log.jsonl
```

This is the prescribed default path for the built-in file sink.

The log root must be redirectable by environment helper for tests and controlled
execution environments, with explicit config taking precedence over env.

### 11.3 `RetainedLogPolicy`

```rust
pub struct RetainedLogPolicy {
    pub rotation_max_bytes: ByteCount,
    pub rotation_max_files: FileCount,
    pub retention_max_age: RetentionMaxAge,
    pub maintenance_cadence: MaintenanceCadence,
    pub writer_shutdown_timeout: WriterShutdownTimeout,
    pub maintenance_max_work_per_pass: Option<usize>,
}
```

Defaults:

- `rotation_max_bytes = ByteCount::from_mib(64)`
- `rotation_max_files = FileCount::from_usize(10)`
- `retention_max_age = RetentionMaxAge::from_days(7)`
- `maintenance_cadence = MaintenanceCadence::new(60s)`
- `writer_shutdown_timeout = WriterShutdownTimeout::new(5s)`

### 11.4 Legacy Direct-Sink Helpers

`RotationPolicy` and `RetentionPolicy` remain available for direct
`JsonlFileSink` construction, but logger-managed retained-log configuration
flows through `RetainedLogPolicy`.

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

`Redactor` is intentionally open. External crates may implement it.

Rules:

- built-in denylist and bearer-token redaction run first
- custom redactors run after built-ins in registration order
- redaction happens before sink fan-out
- sink implementations must receive already-redacted events

### 11.6 `Logger`

Design direction:

```rust
pub struct Logger<State = Running> { /* opaque */ }

impl Logger<Running> {
    pub fn new(config: LoggerConfig) -> Result<Self, InitError>;
    pub fn log(&self, event: LogEvent) -> Result<(), LogError>;
    pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError>;
    #[deprecated(
        since = "1.2.0",
        note = "Use log() for blocking queue admission or try_log() for non-blocking logging."
    )]
    pub fn emit(&self, event: LogEvent) -> Result<(), EventError>;
    pub fn flush(&self) -> Result<(), FlushError>;
    pub fn shutdown(self) -> Logger<Stopped>;
}

impl<State> Logger<State> {
    pub fn health(&self) -> LoggingHealthReport;
}
```

Lifecycle rules:

- `log()` validates and redacts before queue admission, blocks until the event
  is accepted by the writer runtime, and does not guarantee write durability
- `try_log()` validates and redacts before queue admission and returns
  `TryLogError::QueueFull` rather than blocking when the queue is saturated
- deprecated `emit()` remains available as a compatibility path while new
  consumers migrate to `log()` and `try_log()`
- deprecated `emit()` remains fail-open and performs a best-effort flush when
  the writer is not inside an active retained-log maintenance pass so existing
  logger-only consumers keep synchronous visibility expectations where
  practical without regressing queue admission during maintenance work
- `Logger::shutdown()` consumes `Logger<Running>` and returns `Logger<Stopped>`
- `Logger::shutdown()` drains already-queued events and does not return until
  the writer thread has definitively joined
- the configured shutdown timeout is a degradation threshold recorded in
  health/error reporting; if it is exceeded, shutdown still waits for writer
  completion before returning `Logger<Stopped>`
- post-shutdown `emit()`, `query()`, and `follow()` misuse becomes a compile-time error

Logger error inventory:

```rust
pub enum LogError {
    InvalidEvent(EventError),
    WriterDegraded(#[source] Box<ErrorContext>),
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}

pub enum TryLogError {
    InvalidEvent(EventError),
    QueueFull(#[source] Box<ErrorContext>),
    WriterDegraded(#[source] Box<ErrorContext>),
    ShutdownTimedOut(#[source] Box<ErrorContext>),
}
```

Crate-local producer injection trait:

```rust
mod sealed_emitters {
    pub trait Sealed {}
}

pub trait LogEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_log(&self, event: LogEvent) -> Result<(), EventError>;
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

`LogFilter` is intentionally open. External crates may implement it.

Rules:

- one event may fan out to multiple sinks
- sinks may receive all events or only filtered subsets
- filtering is sink-local policy, not producer burden
- the logger service owns sink invocation order and failure handling

### 11.9 Constants And Error Registry Modules

`sc-observability` should ship:

- `src/constants.rs`
  - `DEFAULT_LOG_QUEUE_CAPACITY`
  - `DEFAULT_ROTATION_MAX_BYTES`
  - `DEFAULT_ROTATION_MAX_FILES`
  - `DEFAULT_RETENTION_MAX_AGE_DAYS`
  - `DEFAULT_ENABLE_FILE_SINK`
  - `DEFAULT_ENABLE_CONSOLE_SINK`
- `src/error_codes.rs`
  - `LOGGER_INVALID_EVENT`
  - `LOGGER_QUEUE_FULL`
  - `LOGGER_WRITER_DEGRADED`
  - `LOGGER_SHUTDOWN_TIMED_OUT`
  - `LOGGER_SHUTDOWN`
  - `LOGGER_SINK_WRITE_FAILED`
  - `LOGGER_INIT_FAILED`
  - `LOGGER_FLUSH_FAILED`

Writer-thread batching is intentionally internal-only in phase A. The locked
public configuration surface ends at `LoggerConfig.queue_capacity`; batch size
and batch-delay tuning remain implementation-owned constants rather than
consumer-configurable API.

### 11.10 Logging Failure Model

Rules:

- invalid log events return `EventError`
- sink failures after validation are fail-open
- sink failures update health and dropped counters
- sink failures do not fail the caller's core command flow
- no panic-based contract is implied

### 11.11 Logging Health

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
    pub flush_errors_total: u64,
    pub active_log_path: std::path::PathBuf,
    pub sink_statuses: Vec<SinkHealth>,
    pub queue_depth: u64,
    pub queue_capacity: u64,
    pub queue_high_water_mark: u64,
    pub queue_full_drops_total: u64,
    pub writer_state: WriterState,
    pub last_writer_error: Option<DiagnosticSummary>,
    pub query: Option<QueryHealthReport>,
    pub maintenance: Option<MaintenanceHealthReport>,
    pub last_error: Option<DiagnosticSummary>,
}

pub enum WriterState {
    Running,
    Degraded,
    Stopped,
}
```

Health rules:

- `WriterState` is part of the shared health-contract surface owned by
  `sc-observability-types` and re-exported by `sc-observability`
- `queue_depth` is the current admitted-but-not-yet-written record count
- `queue_capacity` is the configured bounded queue size
- `queue_high_water_mark` records the highest observed queue depth since
  startup
- `queue_full_drops_total` records explicit queue-full drops from
  non-blocking logging calls
- `writer_state` and `last_writer_error` expose background runtime degradation
  without forcing consumers to infer writer health from sink-local failures

## 12. Telemetry Surface (`sc-observability-otlp`)

In v1, the telemetry surface is OTLP-backed and lives in
`sc-observability-otlp`.

### 12.1 `TelemetryConfig`

Design direction:

```rust
pub struct TelemetryConfig {
    pub service_name: ServiceName,
    pub resource: ResourceAttributes,
    pub transport: OtelConfig,
    pub logs: Option<LogsConfig>,
    pub traces: Option<TracesConfig>,
    pub metrics: Option<MetricsConfig>,
}
```

Composition rule:

- `TelemetryConfig` is constructed independently by the application layer
- `TelemetryConfig` is passed directly to `sc-observability-otlp`
- `TelemetryConfig.service_name`, `resource`, `transport`, and signal-specific
  settings are owned by the OTLP setup path, not by `ObservabilityConfig`

Recommended construction shape:

```rust
pub struct TelemetryConfigBuilder { /* opaque */ }

impl TelemetryConfigBuilder {
    pub fn new(service_name: ServiceName) -> Self;
    pub fn with_resource(self, resource: ResourceAttributes) -> Self;
    pub fn with_transport(self, transport: OtelConfig) -> Self;
    pub fn enable_logs(self, config: LogsConfig) -> Self;
    pub fn enable_traces(self, config: TracesConfig) -> Self;
    pub fn enable_metrics(self, config: MetricsConfig) -> Self;
    pub fn build(self) -> TelemetryConfig;
}
```

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
    pub timeout_ms: DurationMs,
    pub debug_local_export: bool,
    pub max_retries: u32,
    pub initial_backoff_ms: DurationMs,
    pub max_backoff_ms: DurationMs,
}
```

Defaults:

- `enabled = false`
- `endpoint = None`
- `protocol = OtlpProtocol::HttpBinary`
- `auth_header = None`
- `ca_file = None`
- `insecure_skip_verify = false`
- `timeout_ms = 3000`
- `debug_local_export = false`
- `max_retries = 3`
- `initial_backoff_ms = 250`
- `max_backoff_ms = 5000`

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

This shape is the v1 transport contract. It preserves the proven transport
knobs while neutralizing the old ATM-specific surface.

This section and its unconditional retry defaults are the frozen 1.x baseline,
not the Phase D 2.0 candidate contract. D.21 exclusively owns the 2.0
backend-aware optional fields, defaults, validation, public payload types, and
stable errors; D.6 must revise this section as part of ADR-018/API approval.
Later sprints reference that revision rather than redefining it.

Rule:

- invalid OTLP transport configuration, including unsupported protocol values,
  is detected at `Telemetry::new(...)` and returns `InitError`

### 12.1.2 Signal Configs

```rust
pub struct LogsConfig {
    pub batch_size: usize,
}

pub struct TracesConfig {
    pub batch_size: usize,
}

pub struct MetricsConfig {
    pub batch_size: usize,
    pub export_interval_ms: DurationMs,
}
```

Defaults:

- `LogsConfig.batch_size = 256`
- `TracesConfig.batch_size = 256`
- `MetricsConfig.batch_size = 256`
- `MetricsConfig.export_interval_ms = 5000`

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
- `flush()` attempts export of all ready batches
- `shutdown()` performs a final flush, drops incomplete spans, and is idempotent

Crate-local direct-signal injection traits:

```rust
mod sealed_emitters {
    pub trait Sealed {}
}

pub trait SpanEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_span(&self, span: SpanSignal) -> Result<(), TelemetryError>;
}

pub trait MetricEmitter: sealed_emitters::Sealed + Send + Sync {
    fn emit_metric(&self, metric: MetricRecord) -> Result<(), TelemetryError>;
}
```

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

pub(crate) trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

pub(crate) trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

pub(crate) trait MetricExporter: Send + Sync {
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
- `LogExporter`, `TraceExporter`, and `MetricExporter` are crate-local runtime
  contracts
- exporter traits remain object-safe for crate-local `Arc<dyn ...>` usage

### 12.5 Constants And Error Registry Modules

`sc-observability-otlp` should ship:

The retry constants below describe the frozen 1.x baseline. D.6 owns their
2.0 disposition and any replacement constants through its reviewed API/semver
update; D.7 and D.8 do not independently redefine them.

- `src/constants.rs`
  - `DEFAULT_OTLP_TIMEOUT_MS`
  - `DEFAULT_OTLP_MAX_RETRIES`
  - `DEFAULT_OTLP_INITIAL_BACKOFF_MS`
  - `DEFAULT_OTLP_MAX_BACKOFF_MS`
  - `DEFAULT_LOG_BATCH_SIZE`
  - `DEFAULT_TRACE_BATCH_SIZE`
  - `DEFAULT_METRIC_BATCH_SIZE`
  - `DEFAULT_METRIC_EXPORT_INTERVAL_MS`
- `src/error_codes.rs`
  - `TELEMETRY_SHUTDOWN`
  - `TELEMETRY_INVALID_PROTOCOL`
  - `TELEMETRY_EXPORT_FAILED`
  - `TELEMETRY_FLUSH_FAILED`
  - `TELEMETRY_EXPORTER_INIT_FAILED`
### 12.6 Telemetry Failure Model

Rules:

- invalid telemetry emission inputs return `TelemetryError::ExportFailure(...)`
- exporter failures after validation are fail-open
- exporter failures update health and dropped counters
- exporter failures do not fail the caller's core command flow
- no panic-based contract is implied

### 12.7 Telemetry Health

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

## 19. Implementation-Readiness Summary

The baseline public API shape is specified below; this readiness statement
applies to the original core scope only. Proposed Phase B additions in §21
require their own contract review and do not inherit baseline approval.

The remaining effort after this document is:

- code implementation of the documented crate surfaces
- integration tests for the required ATM-shaped proving case
- ATM-owned adapter implementation against the shared contracts

This baseline statement does not assert Phase B contract approval or execution
readiness.

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


## 21. Phase B API Evolution — Scoped Runtime Implementation Proposed

[PHB-001–014](requirements.md#10-phase-b-additions--proposed-for-review) and
[ADR-011–015](architecture.md#adr-011-companion-boundaries-and-pre-copy-contract)
record the requested scope and proposed architecture. The [Phase B index](plans/phase-b/plan-phase-b.md)
routes authoritative sprint signatures, deliverables and validation. This section
is a compatibility boundary, not a second competing signature inventory. The
B.P1 runtime-level core acceptance is owner-deferred to Phase B completion
(ATM `01M2PKX8R4J4VJP5V6RRV9JJPB`); all Phase B API evolution remains proposed
unless its own contract says otherwise. This does not approve registry
publication, an owner signature, or an independent QA PASS.

### 21.1 Published API Preservation And Issue #92

The nine diagnostic wrappers (Identity, Init, Event, Flush, Shutdown, Projection,
Subscriber, LogSink and Export) remain available with their existing construction,
trait implementations, metadata and Serde representation. New typed failure
implementations and distinctly named operation/extension entry points coexist.
Classification of custom/unknown legacy diagnostics is total and preserves the
original error. DiagnosticInfo stays sealed; no new required method or bound is
added to an existing consumer-implemented trait. New trait implementations must
not make existing unqualified method calls ambiguous, including glob-import
consumers; compatibility fixtures exercise unchanged source.

Improved methods use typed errors from the failure site and share the runtime
with legacy adapters. Working replacements precede actionable compiler
`#[deprecated]` warnings; removal has no scheduled release. Rust warning-denial
policies may require migration, which the adoption guide explains explicitly.
No in-place struct-to-enum conversion, existing-enum non-exhaustive annotation,
required public-struct field, changed return type or wire-shape change is planned.
Review the [error sprint contract](plans/phase-b/sprint-b-1a-error-api.md) and its
successors for the exact replacement inventory before implementation approval.

B.1e migration implementation records that inventory in
[`plans/phase-b/warning-inventory-b-1e.md`](plans/phase-b/warning-inventory-b-1e.md)
and routes adopters through
`.claude/skills/sc-observability-adopting/references/migrate-error-api.md`.
The record activates the authorized warning attributes at the exact next-minor
version after the B.P2 staged prerequisite; B.2 qualifies that
result. The two B.P1 owner constructors remain method-level
exemptions, while explicit `InitError` wrapper use is documented separately.

### 21.2 Bridge And Runtime Level Contracts

The [target bridge API](plans/phase-b/target-bridge-api.md) specifies the initial
unpublished companion contract; revisions to BTIT's unpublished design do not
permit changes to published core APIs. Accept that contract and BTIT's resulting
reviewed implementation before copy. Bridge-specific errors and health are the
scoped TYP-030 exception in proposed ADR-011; core shared types remain neutral.

The [runtime-level contract](plans/phase-b/runtime-level-contract.md) specifies
additive core owner construction, read-only level snapshots and typed elevate/
reset outcomes. Existing LoggerConfig and LoggingHealthReport remain unchanged.
Its owner deferral is recorded in
[`api-approvals/phase-b-runtime-level.md`](api-approvals/phase-b-runtime-level.md).
The new OperationDiagnostic provides required code, message, remediation and
timestamp for operation outcomes; existing DiagnosticSummary remains an optional
code plus message/time summary. Conversions preserve available original data
and use explicitly documented fallback remediation only when an operation has
already discarded it. B.P2-qualified staged core support is consumed before BTIT
bridge integration; B.7 owns later publication. Existing standalone
constructors preserve baseline filtering without acquiring an external owner.
The core and every adapter use the same effective admission level. Mutation is
serialized against shutdown; diagnostic admission is reported separately and
cannot convert a successful transition into a failure or defeat redaction.

### 21.3 Binding And Nonfatal Result Contracts

[Shared DTO/schema](plans/phase-b/sprint-b-3-schema.md) owns the wire contract;
[Tauri TypeScript](plans/phase-b/sprint-b-3a-typescript.md) and
[Python runtime](plans/phase-b/sprint-b-4-python.md) consume it.
[Shared native backends](plans/phase-b/native-binding-runtime.md) own runtime
conversion and bounded core-host operations for both adapters; the bridge keeps
its accepted pre-copy coordinator. [Python packaging](plans/phase-b/sprint-b-4a-python-packaging.md) separately
qualifies distributions on the full support matrix. Adapters, not core,
own runtime integration. New operational paths return tagged values; foreign
exceptions are converted at boundaries, and ordinary failures do not throw,
raise or reject. Existing infallible accessors retain plain values. Standard
facade/handler protocol methods retain required unit returns with inspectable
bounded outcome accounting.

A filtered event is a distinct successful no-enqueue outcome, not accepted
admission. Local scheduling is not host admission; neither is persistence.
Attached Python and TypeScript cannot shut down or mutate the host logger.
Python-owned handles may hold the corresponding owner capabilities. Optional
receipt awaits inspect synchronous admission; async flush observer waits do
not cancel native operations on timeout/cancellation. Shutdown retains its
terminal result, while an earlier timed-out flush has no result accessor. A
bridge-native timeout ends its adapter call only; the native slot may still
reject a new explicit flush until completion. Wire variants, diagnostic projections, integer/path
conversion, unknown-result handling and package versions follow the sprint
schema contract without changing native published serialization.


## Phase D canonical types and wire handoff

D.12 stages this contract in `sc_observability_types::v2` at the current
workspace package version. D.21 activates workspace version 2.0 atomically;
D.18 activates root exports and retires compatibility after consumers migrate.
ADR-017/018 were accepted through PR #225 and ADR-019 through PR #227.
PHB-003/004/005 continue governing 1.x; PHD-001/002 govern the reviewed major
migration. A staged module is not a release-baseline approval.

### Canonical errors

Every canonical enum is non-exhaustive and every named variant carries
`context: Box<ErrorContext>`. `context()`, `diagnostic()` and `into_context()`
borrow or move that exact object, preserving its source and construction
backtrace. `DiagnosticInfo` retains its existing seal. No conversion parses
Display output, invents context fields, or replaces an unknown code with a
success. Diagnostic details carry bounded, redacted metadata; credentials,
header values, response bodies and file contents must not enter diagnostics.

| Cause | Canonical variant |
| --- | --- |
| process identity validation | IdentityError::Process |
| invalid configuration before construction | InitError::Configuration |
| thread/client/provider startup failure | InitError::Runtime |
| invalid event payload | EventError::Validation |
| event routing failure | EventError::Routing |
| flush drain/export failure, including timeout at flush | FlushError::Drain |
| shutdown deadline exceeded | ShutdownError::Timeout |
| other shutdown drain/provider failure | ShutdownError::Drain |
| projection/subscriber callback failure | ProjectionError::Projection / SubscriberError::Subscriber respectively |
| sink write / flush failure | LogSinkError::Write / LogSinkError::Flush respectively |
| OTLP runtime cause | the identically named ExportError variant in the stable failure inventory |

`MetricModelError::{InvalidHistogram, InvalidTemporality, InvalidInterval}`
uses `SC_METRIC_INVALID_HISTOGRAM`, `SC_METRIC_INVALID_TEMPORALITY` and
`SC_METRIC_INVALID_INTERVAL`. `FiniteF64::new` rejects NaN/infinities using
`SC_METRIC_NON_FINITE` in `ValueValidationError`. These constants live in the
shared `error_codes.rs`. Operational/context error serde uses a snake-case
`kind` and a `context` object containing `diagnostic`; source objects and
backtraces are deliberately not serialized. Native chaining preserves them.

`v2::TelemetryError::Shutdown` remains a unit runtime guard;
`From<v2::ExportError>` wraps the exact error in `ExportFailure` and `code()`
returns its diagnostic code. Existing root `ObservationError` guards remain
unchanged. Flush/shutdown/config adapters retain canonical failures as typed
sources, carrying their diagnostic codes and remediation to the outer context.

All ConfigFailure and ExportError variants below are types-owned. The single
OTLP registry is `sc_observability_types::error_codes::otlp`; the OTLP crate
re-exports it. `ExportError::Transport` remains the generic transport cause
and preserves the underlying transport's registered code. It does not replace
a more precise row below.

| Variant | Stable code | Owning error type | Cause | Recovery | Redaction | Retryability |
| --- | --- | --- | --- | --- | --- | --- |
| `ZeroDuration` | `OTLP_CONFIG_ZERO_DURATION` | `ConfigFailure` | required duration is zero | provide a positive value | field/value only | after config correction |
| `DurationOverflow` | `OTLP_CONFIG_DURATION_OVERFLOW` | `ConfigFailure` | milliseconds cannot convert safely | reduce the field | field/value only | after config correction |
| `InvalidBoundOrdering` | `OTLP_CONFIG_BOUND_ORDER` | `ConfigFailure` | resolved ordering rule fails | correct the named explicit/defaulted fields | field/value/origin only | after config correction |
| `InvalidJitterPercent` | `OTLP_CONFIG_JITTER_PERCENT` | `ConfigFailure` | jitter exceeds 100 | use `0..=100` | field/value only | after config correction |
| `InvalidQueueCapacity` | `OTLP_CONFIG_QUEUE_CAPACITY` | `ConfigFailure` | queue capacity is outside `1..=65_536` | choose a bounded capacity | field/value only | after config correction |
| `InvalidQueueByteCapacity` | `OTLP_CONFIG_QUEUE_BYTE_CAPACITY` | `ConfigFailure` | zero, overflow or aggregate byte bound above 64 MiB | choose 1..=64 MiB (default 16 MiB) | field/value only | after config correction |
| `ConfigFieldNotApplicable` | `OTLP_CONFIG_FIELD_NOT_APPLICABLE` | `ConfigFailure` | field is inapplicable to disabled transport or the selected backend | omit it, enable transport, or select its applicable backend | field/closed target only | after config correction |
| `InsecureTransportRejected` | `OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED` | `ConfigFailure` | selected backend does not implement the requested insecure verification override | disable the override or choose an explicitly supporting backend | backend only | after config correction |
| `InvalidEndpoint` | `OTLP_CONFIG_INVALID_ENDPOINT` | `ConfigFailure` | endpoint URL syntax is invalid | provide a valid endpoint URL | field only | after config correction |
| `InvalidHeader` | `OTLP_CONFIG_INVALID_HEADER` | `ConfigFailure` | header/auth syntax or credential placement is invalid | correct the header/auth configuration | field only | after config correction |
| `TransportConstructionFailed` | `OTLP_TRANSPORT_CONSTRUCTION_FAILED` | `ConfigFailure` | CA/auth/client/provider/legacy-worker initialization failed | correct the bounded typed source and reconstruct | bounded typed source; never path contents, credentials, header values, or response bodies | after config/environment correction |
| `UnsupportedBackend` | `OTLP_UNSUPPORTED_BACKEND` | `ConfigFailure` | feature/backend unavailable | enable/select a supported backend | enum values only | after build/config correction |
| `UnsupportedProtocol` | `OTLP_UNSUPPORTED_PROTOCOL` | `ConfigFailure` | protocol invalid for backend | select a matrix-supported protocol | enum values only | after config correction |
| `TokioRuntimeRequired` | `OTLP_TOKIO_RUNTIME_REQUIRED` | `ConfigFailure` | SDK construction lacks an entered Tokio runtime | construct inside the host runtime | no dynamic data | after entering a runtime |
| `BlockingBackendInAsyncContext` | `OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT` | `ExportError` | legacy synchronous lifecycle entered Tokio; construction preserves this condition as the redacted source of `TransportConstructionFailed` | use a plain thread or async lifecycle | no dynamic data | in a supported context |
| `AsyncLifecycleRequired` | `OTLP_ASYNC_LIFECYCLE_REQUIRED` | `ExportError` | SDK synchronous completion requested | await the typed async operation | no dynamic data | through async lifecycle |
| `RuntimeTerminated` | `OTLP_RUNTIME_TERMINATED` | `ExportError` | host runtime ended before completion | keep the runtime alive through awaited shutdown | bounded state/counts | with a live replacement runtime/instance |
| `LifecycleTimeout` | `OTLP_LIFECYCLE_TIMEOUT` | `ExportError` | monotonic lifecycle deadline elapsed | inspect terminal health and transport/provider | duration/state only | operation-specific |
| `QueueFull` | `OTLP_QUEUE_FULL` | `ExportError` | bounded admission queue saturated | preserve fail-open behavior and inspect health | capacity/depth only | yes, later admission |
| `WorkerTerminated` | `OTLP_WORKER_TERMINATED` | `ExportError` | SDK dispatcher or legacy worker terminated unexpectedly | correct the terminal cause and construct a new instance | bounded typed source; no credentials | only with a new instance |
| `ShutdownCancelledRetry` | `OTLP_SHUTDOWN_CANCELLED_RETRY` | `ExportError` | shutdown cancelled a retryable pre-barrier legacy sequence | inspect terminal health; resend only if duplicates are acceptable | attempt/count only | caller decision; duplicates possible |
| `RetryDeadlineExhausted` | `OTLP_RETRY_DEADLINE_EXHAUSTED` | `ExportError` | no legacy sequence budget remains | increase the validated sequence bound or restore collector health | budget/attempt only | new operation after recovery |
| `NonRetryableHttpStatus` | `OTLP_HTTP_STATUS_TERMINAL` | `ExportError` | collector returned a non-retryable HTTP status | correct request/auth/config before retrying | status/category only; no body/headers | after cause correction |
| `RetryAttemptsExhausted` | `OTLP_RETRY_ATTEMPTS_EXHAUSTED` | `ExportError` | legacy maximum attempts ended before success | restore collector health or adjust the validated policy | attempt/count only | new operation after recovery |
| `TerminalExportFailure` | `OTLP_EXPORT_TERMINAL` | `ExportError` | SDK or legacy provider returned a terminal export failure | inspect the preserved source and collector state | bounded typed source; no credentials | source-dependent |
| `Shutdown` | `OTLP_TELEMETRY_SHUTDOWN` | `TelemetryError` | emit was attempted after shutdown began | construct a new telemetry instance | no dynamic data | only on a new instance |

Core `error_codes.rs` owns `SC_LOG_SINK_REGISTRATION_DUPLICATE`,
`SC_LOG_SINK_REGISTRATION_INVALID`, `SC_LOG_SINK_REGISTRATION_CLOSED`, and
settings constants `LOG_PREFIX_COLLISION` (`LOG-001`),
`LOG_INVALID_ENVIRONMENT` (`LOG-002`), `LOG_UNKNOWN_KEY` (`LOG-003`),
`LOG_INVALID_VALUE` (`LOG-004`), `LOG_RESOLUTION` (`LOG-005`). The existing
settings code spellings are retained; these are diagnostics, not requirement
IDs. The bridge registry owns `SC_LOG_DETACH_TIMEOUT`,
`SC_LOG_DETACH_NOT_INSTALLED`, `SC_LOG_FOREIGN_LOGGER_INSTALLED`.
DTO and routing registry values retain their existing meanings.

### Neutral signals

All names in this subsection are staged in `v2`. `TraceFlags::new(u8)` keeps
all input bits; `sampled()` reads bit 0. Serde encodes flags as a byte number.
`TraceContext::new(trace_id, span_id, flags)` has no parent until
`with_parent` is called. Trace/span identifiers validate on serde input as
well as construction. `SpanLink::new(trace_id, span_id, flags, attributes)`
contains no nested trace context. `SpanKind` uses `internal`, `server`,
`client`, `producer`, `consumer` serde tokens.

`Attributes` is an ordered string-keyed map of neutral `AttributeValue`
boolean, signed/unsigned integer, finite float, string, array, object or null
values. Its public API has no serde_json, runtime or transport type dependency.
The existing crate dependency on serde_json remains for 1.x diagnostics.

`SpanRecord<SpanStarted>::new(timestamp, service, name, trace, attributes)`
creates an internal span with no links. `with_kind` and `with_links` populate
those fields; `end(status, duration)` is the only route to
`SpanRecord<SpanEnded>`. Fields remain private; only ended records expose
`duration_ms`. No deserializer can synthesize an ended producer record.
`SpanSignal::{Started, Event, Ended}` supplies the export state discriminant;
serialization preserves kind, links and flags. Consumers constructing native
records from a wire DTO must replay checked construction and `end`.

`MetricRecord::try_new(timestamp, service, name, value)` checks the interval;
`with_unit` and `with_attributes` set validated metadata. Its fields are
private and serde goes through the same checked constructor. Gauge has no
start time. Sum and histogram require explicit Delta/Cumulative temporality
and start time. Start after timestamp is `InvalidInterval`; an empty Delta
interval and negative monotonic sum are `InvalidTemporality`. Cumulative may
have an initial zero-length interval. Sequence continuity across points is a
producer concern, not a single-record invariant.

`HistogramPoint::try_new(bounds, buckets, count, sum)` requires finite,
strictly increasing bounds; exactly one more bucket than bounds; checked
bucket addition equal to count; and zero sum when count is zero. Empty bounds
with one bucket are valid. Negative samples/sums are valid. Sum is FiniteF64.
`explicit_bounds`, `bucket_counts`, `count`, `sum` are read-only accessors.
Serde uses that constructor and cannot bypass these checks.

Native metric serde is frozen as `{"kind":"gauge","data":1.5}`,
`{"kind":"sum","data":{"value":1.5,"monotonic":true,
"temporality":"delta","start_time":"1970-01-01T00:00:00Z"}}`, or
`{"kind":"histogram","data":{"point":{"explicit_bounds":[1.0],
"bucket_counts":[1,0],"count":1,"sum":0.5},"temporality":"cumulative",
"start_time":"1970-01-01T00:00:00Z"}}`. The record adds `timestamp`,
`service`, `name`, `value`, `unit`, and `attributes`. Timestamps use the shared
UTC-normalizing RFC3339 codec.

### DTO and language conversion contract

D.19 owns checked DTO/schema conversions and generated models. D.20 consumes
this compatible operational envelope independently using local fixtures.
Both retain schema version 1 and exact outer field names/discriminants:
`{"kind":"ok","schema_version":1,"value":...}` or
`{"kind":"error","schema_version":1,"error":...}`. Internal ResultDto
has the same `kind`/`value`/`error` fields without `schema_version`.
Admission is `{"kind":"accepted"}` or `{"kind":"filtered"}`;
completion is `{"kind":"completed"}`. Admission never claims persistence.

Failure keeps its existing `kind` plus flattened Diagnostic fields (`at`,
`code`, `message`, `remediation`). D.19 may add optional `cause`, `docs` and
`details` fields to retain redacted metadata; their absence remains valid and
existing adapters continue using the required four-field envelope. Native causes map to
existing wire categories: payload/config/model validation to `validation`,
admission saturation to `queue_full`, shutdown guards to `closed`, deadlines
to `timeout`, cancellation to `cancelled`, I/O to `io`, and unavailable worker
or runtime to `unavailable`. Preserve the original registered diagnostic code,
message, remediation, docs and bounded details; a category is not a replacement
code. Unknown remote discriminants use `unknown_remote` with `remote_kind` and
the received diagnostic. Never convert an unknown or malformed failure to ok.
Unexpected local failures use the existing `internal` diagnostic boundary.

Native source objects/backtraces stay native. Only deliberately redacted
cause/details are projected. Existing DTO size and field validation stays in
force, with its existing binding error registry. Unknown schema versions
return `unsupported_version`; malformed fields return `validation`. Tauri
commands resolve tagged operational errors; Python returns them as data.
Neither expected failures nor observer cancellation throw or cancel native work.

New neutral signals are staged/additive until D.18 activation. Signal DTO
field names and discriminants match the native specification above, except
all i64/u64 values (including histogram count/buckets and integer attributes)
use the existing canonical decimal-string DTO codec. Never round through
JavaScript Number. Floats remain finite numbers, flags remain a byte, and
bounds/counts/sum/temporality/start_time must all survive conversion. Invalid
histograms or intervals are rejected through checked native constructors;
no synthetic scalar histogram or inferred interval is permitted.

### Reviewed OTLP reference specification (D.21 handoff)

D.21 owns implementation of the following reviewed contract; this section is
read-only input for that sprint. Its configuration validation produces the
types-owned ConfigFailure enum above, with field/value/origin/target metadata
in bounded Diagnostic.details, not extra enum fields. Lifecycle and exporter
traits remain crate-private, Send + Sync, with Send lifecycle futures.

The factory validates this closed matrix before allocating providers/workers:

| Backend | Valid protocol | Required feature/runtime | Invalid result |
| --- | --- | --- | --- |
| disabled (transport disabled) | none | none | the sole no-network disabled implementation |
| `OpenTelemetrySdk` | SDK-supported gRPC or HTTP/protobuf | `otlp-sdk`; entered caller Tokio runtime | stable unsupported-protocol/runtime error |
| `LegacyHttpJson` | `HttpJson` only | `legacy-http-json`; plain-thread construction | reserved typed error until D.8 |

Delete public/production `Noop*Exporter` fallbacks; disabled construction is an
explicit private disabled set and an enabled selection can never reach it.
Every existing `OtelConfig` field receives one disposition: endpoint,
headers/auth, CA/TLS and `timeout_ms` map to the SDK/legacy builders;
`debug_local_export` is a separate diagnostic mirror outside exporter
selection; `insecure_skip_verify` is either implemented by the backend with an
explicit security warning or rejected at construction—never ignored.
`timeout_ms` covers the entire legacy HTTP request, including connect, TLS,
request write, response headers, and response read. Endpoint and header/auth
values are validated before provider/worker construction; malformed endpoints,
invalid header syntax, and forbidden credential placement return named
construction failures without retaining secret values.

### Validated transport contract

D.21 owns transport field/default/validation implementation; D.12 owns the
canonical error definitions and registry. The 2.0 wire surface uses direct shared transport
fields plus a grouped `legacy_retry` object:

| Field | Applicability | Default when absent |
| --- | --- | --- |
| `timeout_ms` | both backends; maps to request/export timeout | `3_000` |
| `lifecycle_flush_timeout_ms` | both backends | `30_000` |
| `lifecycle_shutdown_timeout_ms` | both backends | `30_000` |
| `queue_capacity` | both backends; bounded admission queue | `1_024` |
| `legacy_retry.max_retries` | legacy only, optional on wire | `3` |
| `legacy_retry.initial_backoff_ms` | legacy only, optional on wire | `250` |
| `legacy_retry.max_backoff_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_sequence_timeout_ms` | legacy only, optional on wire | `30_000` |
| `legacy_retry.retry_after_cap_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_jitter_percent` | legacy only, optional on wire | `20` |

`queue_capacity` counts admitted records, not batches, and is validated as `1..=65_536`. A separate checked `queue_byte_capacity` defaults to 16 MiB, has a hard 64 MiB maximum, and bounds the serialized payload bytes held by all queued/in-flight batches. Admission reserves both record and byte credits atomically; either exhausted budget returns QueueFull. Records larger than 1 MiB are rejected before enqueue; batches split at 512 records or 1 MiB. The queue cannot retain 65,536 one-MiB batches. A 413 is terminal for that split batch,
which is counted once as failed/dropped rather than retried as a larger batch.
`shutdown_async_typed` has one drain budget: it starts at shutdown entry and
covers cancellation, the in-flight request, barrier, and worker join. On
expiry it returns `LifecycleTimeout` with remaining admitted work accounted.

```rust
#[non_exhaustive]
pub struct TelemetryHealth {
    pub queue_depth: usize,
    pub queue_capacity: usize,
    pub worker_state: WorkerState,
    pub last_terminal_failure: Option<Diagnostic>,
    pub last_success: Option<Timestamp>,
}
```

For `OpenTelemetrySdk`, the three shared timeout fields map to SDK lifecycle /
export construction. Any explicit legacy-only field—including the pre-existing
`max_retries`, `initial_backoff_ms`, and `max_backoff_ms`—returns
`ConfigFieldNotApplicable`. Nothing is ignored. This 2.0 optional-field change
and its migration from the former unconditional retry defaults are documented.

Defaults are resolved **before** validation. Each resolved value retains
`ValueOrigin::{Default, Explicit}` so an error identifies both the offending
field and whether a conflicting peer was defaulted. Partial overrides are
therefore deterministic and reviewable.

All raw serialized millisecond/percent fields are converted exactly once:

```rust
#[non_exhaustive]
pub enum OtlpConfigField {
    Endpoint,
    Header,
    Timeout,
    LifecycleFlushTimeout,
    LifecycleShutdownTimeout,
    QueueCapacity,
    QueueByteCapacity,
    MaxRetries,
    InitialBackoff,
    MaxBackoff,
    RetrySequenceTimeout,
    RetryAfterCap,
    RetryJitterPercent,
}

#[non_exhaustive]
pub enum ValueOrigin { Default, Explicit }

#[non_exhaustive]
pub struct ResolvedField<T> {
    pub field: OtlpConfigField,
    pub value: T,
    pub origin: ValueOrigin,
}

#[non_exhaustive]
pub enum OtlpConfigTarget { Disabled, Backend(ExporterBackend) }

pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    fn try_from_millis(field: OtlpConfigField, value: u64)
        -> Result<Self, ConfigFailure>;
}

pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

pub(crate) struct BoundedPercent(u8); // checked 0..=100

pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: PositiveDuration,
    max_backoff: PositiveDuration,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

pub(crate) struct ValidatedTransportBounds {
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

impl ValidatedTransportBounds {
    fn try_from_config(config: &OtelConfig) -> Result<Self, ConfigFailure>;
}
```

`TelemetryHealth`, `OtlpConfigField`, `ValueOrigin`, `ResolvedField`, and
`OtlpConfigTarget` are `#[non_exhaustive]` public types so their 2.0 contracts
can add fields or variants without a further breaking release.

The constructor derives `Disabled` from `config.enabled == false`; otherwise
it derives the backend only from `config.backend`.
`LifecycleBounds` holds checked positive flush/shutdown durations;
`RetryPolicy` holds `max_retries`, checked initial/max/sequence/Retry-After
durations, and `BoundedPercent(0..=100)`. `BackendTransportBounds` makes legacy
retry state unrepresentable for SDK. Validation, using checked arithmetic, is:

- every millisecond duration is positive and convertible to `Duration`;
- `lifecycle_shutdown_timeout_ms >= timeout_ms`;
- `lifecycle_flush_timeout_ms >= timeout_ms`;
- `queue_capacity` is in `1..=65_536`, otherwise `InvalidQueueCapacity`;
- for legacy, `max_backoff_ms >= initial_backoff_ms`;
- for legacy, `retry_sequence_timeout_ms >= timeout_ms`;
- for legacy, `0 < retry_after_cap_ms <= retry_sequence_timeout_ms`;
- for legacy, `retry_jitter_percent <= 100`;
- reject every explicit field inapplicable to disabled transport or the
  selected backend with `ConfigFieldNotApplicable`;
- reject a requested insecure verification override when the selected backend
  does not explicitly support it with `InsecureTransportRejected`.
- validate endpoint URL syntax, header/auth syntax, and credential placement;
  otherwise return `InvalidEndpoint` or `InvalidHeader` with a redacted,
  field-only payload.

Checks execute in exactly this listed order and return the first failure; they
are not aggregated. Within the first bullet, fields are checked in the wire
table's top-to-bottom order. This makes every multi-violation diagnostic
deterministic.

Both factories receive only `ValidatedTransportBounds` and cannot inspect or
reparse raw fields. Deadlines use a monotonic injectable clock and start when
the public operation is admitted.

Construction order is fixed: resolve defaults and create `ResolvedField`
values; run the ordered validation list above; build
`ValidatedTransportBounds`; then check feature/backend/protocol availability.
Thus a malformed legacy config fails deterministically before D.6's reserved
`UnsupportedBackend`. Disabled transport still validates explicitly supplied
shared fields, rejects every explicit legacy-only retry field with
`ConfigFieldNotApplicable { target: OtlpConfigTarget::Disabled, .. }`, yields
`BackendTransportBounds::Disabled`, and never constructs a network
provider/worker. SDK inapplicability instead records
`OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk)`.


The complete private signatures and module handoff are recorded in
[sprint D.21](plans/phase-d/sprint-d-21-otlp-contract.md).
