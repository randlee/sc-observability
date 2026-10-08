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
     -> log projectors -> Logger -> log sinks (file, optional OtelLogSink)
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
  `sc-observability-types <- sc-observability <- sc-observe`, with
  `sc-observability-otlp` depending on `sc-observability-types` and, for
  `log-sink`, `sc-observability`
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
- log data contracts
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

### 6.3 `sc_observe::v2`

Owns typed observation routing and projection.

Owns:

- `v2::Observability`
- typed observation admission via `v2::Observability::emit`
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
- the canonical surface is the explicit `v2` namespace; applications do not
  need a general-purpose workflow engine beyond these routing responsibilities

Must not own:

- application-specific observation types
- ATM-specific compatibility behavior

### 6.4 `sc-observability-otlp`

Owns native OpenTelemetry integration (ADR-023).

Owns:

- `OtelLogSink` (feature `log-sink`): maps core log events to official
  `opentelemetry_sdk` log records; re-exports the `api` and `sdk` types it uses
- `sync::Client` (feature `synchronous-client`): official blocking OTLP/HTTP
  protobuf exporter for frontends without Tokio, used by the `sc-otel` CLI and
  the Python `Telemetry` binding

Design intent:

- this crate sits at the top of the stack
- Tokio hosts use `opentelemetry_sdk` and `opentelemetry-otlp` directly
- OTel log export attaches to the logger through the existing `LogSink`
  extension point; it registers no projector with `ObservabilityBuilder`

Must not own:

- local file logging
- a replacement signal model, provider facade, exporter trait, retry queue or
  durable store
- ATM-specific metadata rules
- ATM compatibility behavior

## 6.5 Dependency Direction

Recommended dependency direction:

```text
sc-observability-types
    ↑
    └── sc-observability
          ↑
          ├── sc-observe
          └── sc-observability-otlp (log-sink)
```

Implications:

- a basic CLI may depend only on `sc-observability`
- applications that need observation routing depend on `sc-observe`
- OTEL remains optional and isolated in `sc-observability-otlp`
- `sc-observe` and `sc-observability-otlp` do not depend on each other

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
- `sc-observe` does not derive or own OpenTelemetry configuration; the
  application configures the official SDK or `sync::Client` directly

Registrations are config-time only:

- subscriber and projector registrations are passed through
  `ObservabilityBuilder` or equivalent construction-time configuration
- registration closes at `Observability::new(...)`
- no runtime registration after construction is part of v1

Full-stack attachment model:

- OTel log export attaches to the logger as an `OtelLogSink` registered
  through the existing `LogSink` extension point (§12.1)
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

### 7.4 Producer Entry Points

Producer code uses the concrete facades that own its admission behavior:

- typed observations call `Observability::emit`
- logging-only code calls `Logger::emit`
- OTel-specific code uses the official OpenTelemetry SDK, or `sync::Client`
  without Tokio (§12.2)

The former crate-local sealed emitter traits had no supported consumers and are
not retained as injection contracts. Open cross-crate extension points remain
in `sc-observability-types` where their consumer-owned implementations are part
of the supported API.

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
pub struct StateName(String);
pub struct EntityId(String);
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
  - used by: `LoggerConfig`, `LogEvent`
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
  - used by: caller-owned metric names
  - invariant: non-empty metric identifier using `[A-Za-z0-9._\\-/]+`
- `StateName`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: `StateTransition.from_state` and `StateTransition.to_state`
  - invariant: non-empty ASCII identifier using `[A-Za-z0-9._-]+`
- `EntityId`
  - owner: `sc-observability-types`
  - underlying type: validated `String`
  - used by: canonical admission validation of `StateTransition.entity_id`; the
    field itself stays `Option<String>` exactly as released in 1.4.1
  - invariant: non-empty ASCII identifier using `[A-Za-z0-9._-]+`

These newtypes should expose:

```rust
impl ToolName {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueValidationError>;
    pub fn as_str(&self) -> &str;
}
```

Equivalent constructors and accessors apply to the other listed newtypes.

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
    pub entity_kind: TargetCategory,
    /// Optional caller-owned identifier for the entity that changed.
    pub entity_id: Option<String>,
    /// Previous stable state label.
    pub from_state: StateName,
    /// New stable state label.
    pub to_state: StateName,
    /// Optional human-readable explanation for why the transition occurred.
    pub reason: Option<String>,
    /// Optional action or event name that triggered the transition.
    pub trigger: Option<ActionName>,
}
```

Meaning:

- `entity_kind`: what changed, such as `task`, `subagent`, `test_run`
- `entity_id`: which specific entity changed. The field type is the released
  `Option<String>`, so one `LogEvent`/query graph serves every facade.
  Canonical (v2) entry points validate it as an `EntityId` at admission, after
  the version, service and level checks, and reject an invalid value as
  `EventError::Validation`.
  Released root facades keep exact 1.4.1 acceptance with no entity check, and
  stored or queried events always decode leniently.
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

### 9.2 `DiagnosticSummary`

Design direction:

```rust
pub struct DiagnosticSummary {
    pub code: Option<ErrorCode>,
    pub message: String,
    pub at: Timestamp,
}
```

### 9.3 Shared Constants And Error Registry Modules

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

### 9.4 Public Error Type Pattern

The published baseline uses diagnostic wrappers as described below. Phase B
records improved errors and migration guidance in §21.

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
pub struct EventError(pub Box<ErrorContext>);
pub struct FlushError(pub Box<ErrorContext>);
pub struct ShutdownError(pub Box<ErrorContext>);
pub struct ProjectionError(pub Box<ErrorContext>);
pub struct SubscriberError(pub Box<ErrorContext>);
pub struct LogSinkError(pub Box<ErrorContext>);
pub struct IdentityError(pub Box<ErrorContext>);
```

Required pattern:

- most public API errors are named newtypes around `ErrorContext`
- `ObservationError` is an enum because it needs a named shutdown guard
  variant
- contextual public error payloads are boxed inside the error types themselves
  so public `Result<_, Error>` surfaces remain small and clippy-clean
- all public API errors implement `std::error::Error` and `Display`
- errors that always carry diagnostics implement `DiagnosticInfo`
- `ObservationError` exposes optional diagnostic access only on its contextual
  variants
- `DiagnosticInfo` is defined in `sc-observability-types` as a sealed trait;
  preserve that published seal and its existing workspace implementations
- named error newtypes in each crate implement `DiagnosticInfo` by delegating to
  their inner `ErrorContext`
- `ObservationError::Shutdown` does not carry `ErrorContext` and therefore
  does not implement `DiagnosticInfo` directly
- baseline machine/actionable meaning is carried by `Diagnostic.code`; the
  proposed additive API also exposes typed classification without changing codes
- callers may render the diagnostic directly for CLI output and also attach it
  to logs
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
- projectors that map typed observations into log events

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
projected log record.

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

### 10.3 Registration and Filtering

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

### 10.4 Why This Split Exists

This split supports the complicated but common pattern where one typed producer
event needs to:

- be logged
- also go to one or more typed subscribers

without requiring the producer to emit those outputs separately.

### 10.5 `sc-observe` Constants And Error Registry Modules

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

`Logger::emit` is the retained logger admission entry point and preserves the
released `EventError` contract.

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

`sc-observability-otlp` is opt-in native OpenTelemetry integration (ADR-023).
It uses the official `opentelemetry`, `opentelemetry_sdk` and
`opentelemetry-otlp` 0.33.0 types; with `log-sink` it re-exports the ones its
contracts name as `api` and `sdk`. Native Tokio hosts use the official crates
directly.

### 12.1 `OtelLogSink` (feature `log-sink`)

```rust
pub struct OtelLogSink { /* private */ }

impl OtelLogSink {
    pub fn new(provider: &sdk::logs::SdkLoggerProvider, scope: api::InstrumentationScope) -> Self;
}

impl LogSink for OtelLogSink { /* write, flush, health */ }
```

Rules:

- maps each already-filtered, already-redacted `LogEvent` to a native SDK log
  record; registered beside the file sink, the same events reach both
- the caller builds, keeps and shuts down the `SdkLoggerProvider`, configured
  with the official nonblocking `BatchLogProcessor`; the sink never shuts it
  down
- `write` only enqueues; admission to the batch processor is not a delivery
  guarantee, and `flush` never waits on the network
- timestamp, severity, message, target and valid trace context map to native
  record fields; the action becomes `event.name`; other event metadata becomes
  `sc.observability.log.*` attributes and event fields keep their own names
- events whose target starts with `opentelemetry` are SDK diagnostics and are
  dropped so they never recurse into the SDK
- sink health is reported under the name `opentelemetry`

### 12.2 `sync::Client` (feature `synchronous-client`)

```rust
pub struct Client { /* private */ }

impl Client {
    pub fn new(endpoint: &str) -> Result<Self, SyncError>;
    pub fn with_header(self, name: &str, value: &str) -> Result<Self, SyncError>;
    pub fn with_timeout(self, timeout: Duration) -> Result<Self, SyncError>;
    pub fn with_root_certificate_pem(self, pem: &[u8]) -> Result<Self, SyncError>;
    pub fn send_log<F>(&mut self, resource: &sdk::Resource, scope: api::InstrumentationScope, record: F)
        -> Result<(), SyncError>
    where
        F: FnOnce(&mut sdk::logs::SdkLogRecord) -> Result<(), SyncError>;
    pub fn send_span(&mut self, resource: &sdk::Resource, span: sdk::trace::SpanData)
        -> Result<(), SyncError>;
    pub fn send_metrics<F>(&mut self, resource: &sdk::Resource, scope: api::InstrumentationScope, record: F)
        -> Result<(), SyncError>
    where
        F: FnOnce(&api::metrics::Meter) -> Result<(), SyncError>;
}

pub enum SyncError {
    Validation { code: &'static str, source: sdk::error::OTelSdkError },
    Export(sdk::error::OTelSdkError),
}

pub fn check_input_limits(input_bytes: usize, records: usize) -> Result<(), SyncError>;
```

Rules:

- exports native records with the official OTLP/HTTP protobuf exporter over the
  blocking `reqwest` transport; no caller-owned Tokio runtime is required
- one client belongs to one CLI invocation or Python call; every call builds
  and drops its own transport and exporter
- construction and every send reject a thread inside an entered Tokio runtime
  with `error_codes::sync::RUNTIME_ENTERED`
- the configured timeout bounds connecting and each request and is the deadline
  of the exporter's native retry sequence; there is no forcible cancellation
- OTLP partial-success responses are handled by the official exporter and
  returned as success
- explicit endpoint, timeout and headers win over `OTEL_EXPORTER_OTLP_*`
  environment settings
- `send_metrics` returns the SDK `force_flush` outcome; the 0.33.0 SDK reports a
  failed flush as `InternalFailure("Failed to flush")` and exposes the HTTP
  cause only in SDK diagnostics
- `Validation` means the input or configuration was rejected, or the caller's
  closure failed; `Export` means the official exporter or SDK lifecycle failed;
  error text never contains header values or endpoint user information

### 12.3 Constants And Error Registry Modules

- `src/constants.rs`
  - `DEFAULT_OTLP_TIMEOUT_MS`
  - `MAX_INPUT_BYTES`
  - `MAX_BATCH_RECORDS`
- `src/error_codes.rs`
  - `TELEMETRY_EXPORT_FAILED`
  - `sync::INVALID_CONFIG`
  - `sync::RUNTIME_ENTERED`
  - `sync::INVALID_RECORD`
  - `sync::INPUT_LIMIT_EXCEEDED`
  - `sync::CALLER_REJECTED`

### 12.4 Telemetry Health

`sc-observability-types` defines the telemetry health shapes carried in the
optional `telemetry` field of `ObservabilityHealthReport`:

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
    pub name: SinkName,
    pub state: ExporterHealthState,
    pub last_error: Option<DiagnosticSummary>,
}

pub struct TelemetryHealthReport {
    pub state: TelemetryHealthState,
    pub dropped_exports_total: u64,
    pub malformed_spans_total: u64,
    pub exporter_statuses: Vec<ExporterHealth>,
    pub last_error: Option<DiagnosticSummary>,
}
```

## 13. Observation Pattern for Events

Log events represent discrete facts and state transitions. Spans and metrics
use the official OpenTelemetry SDK (§12), not observation projection.

### 13.1 Canonical Rule

If the system needs to answer "what changed?", use an event.

### 13.2 State Transitions

State transitions are event-first.

Recommended pattern:

- project a `LogEvent` with `action = "state_transition"`
- attach a typed `StateTransition`
- include trace context when the transition occurred during a larger work unit

### 13.3 Sub-Agents

Recommended pattern:

- project important facts, retries, warnings, and transitions as events
  sharing the run's trace context

### 13.4 Tasks

Recommended pattern:

- project `state_transition` events for lifecycle changes
- project child event sequences for nested work under the task

### 13.5 Test Runs

Recommended pattern:

- project transition/failure events during the run

## 14. Example Pattern: Consumer-Owned `AgentInfoEvent`

This repo should not define `AgentInfoEvent`.

That type belongs in a consumer-owned crate, such as ATM.

Example conceptual pattern:

- ATM defines `AgentInfoEvent`
- ATM creates an `Observability` runtime from `sc-observe`
- ATM registers:
  - one or more typed subscribers for `AgentInfoEvent`
  - a log projector for `AgentInfoEvent`
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
- `sc-observability-otlp` OTel log export through `OtelLogSink`

The example must prove that one consumer-owned typed observation can fan out to:

- one or more typed subscribers
- one or more log sinks

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

OTel exporters read the standard `OTEL_EXPORTER_OTLP_*` environment through
the official SDK. `sync::Client` applies explicit endpoint, timeout and headers
over those settings (§12.2).

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
- Is the event pattern correct for sub-agents, tasks, and test runs?
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

### 21.1 Migration Inventory

The current consumer migration inventory, replacement APIs, and the rule for
removing compatibility surfaces live in the [Phase F migration guide](migration/phase-f.md).
The guide is the authoritative handoff for application teams and downstream
consumer repositories.

### 21.2 Bridge And Runtime Level Contracts

The [target bridge API](plans/phase-b/target-bridge-api.md) specifies the initial
unpublished companion contract; revisions to BTIT's unpublished design do not
permit changes to published core APIs. Accept that contract and BTIT's resulting
reviewed implementation before copy. Bridge-specific errors and health are the
scoped TYP-030 exception in proposed ADR-011; core shared types remain neutral.

The [runtime-level contract](plans/phase-b/runtime-level-contract.md) specifies
additive core owner construction, read-only level snapshots and typed elevate/
reset outcomes. Existing LoggerConfig and LoggingHealthReport remain unchanged.
Its owner deferral is recorded in the phase-B runtime-level contract.
The new OperationDiagnostic provides required code, message, remediation and
timestamp for operation outcomes; existing DiagnosticSummary remains an optional
code plus message/time summary. Conversions preserve available original data
and use explicitly documented fallback remediation only when an operation has
already discarded it. The accepted 1.4.x release supplied the staged core support
consumed by BTIT bridge integration; B.7 owned later publication. Existing standalone
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

D.12 stages canonical contracts in `sc_observability_types::v2`; D.21
coordinates the workspace version transition and D.18 owns the migration
inventory, release gates, and consumer evidence. Applications opt into
canonical error and observation contracts through the explicit `v2` path.

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

Operational/context error serde uses a snake-case `kind` and a `context`
object containing `diagnostic`; source objects and backtraces are deliberately
not serialized. Native chaining preserves them.

The logger checks event version and service, filters by effective level,
validates the entity identifier, then applies redaction and the event-size
limit. Existing root `ObservationError` guards remain unchanged. Flush/shutdown/config
adapters retain canonical failures as typed sources, carrying their preserved
diagnostic codes and remediation to the outer context.

The `SC_LOG_` family is used by three separately owned public registries; it
does not define one universal prefix owner. Core `error_codes.rs` owns
`SC_LOG_SINK_REGISTRATION_DUPLICATE`, `SC_LOG_SINK_REGISTRATION_INVALID`,
`SC_LOG_SINK_REGISTRATION_CLOSED`, and the D.1-owned settings constants
`SC_LOG_SETTINGS_PREFIX_COLLISION`, `SC_LOG_SETTINGS_INVALID_ENVIRONMENT`,
`SC_LOG_SETTINGS_UNKNOWN_KEY`, `SC_LOG_SETTINGS_INVALID_VALUE`, and
`SC_LOG_SETTINGS_RESOLUTION`. The `sc-observability-log` registry owns
`SC_LOG_DETACH_TIMEOUT`, `SC_LOG_DETACH_NOT_INSTALLED`, and
`SC_LOG_FOREIGN_LOGGER_INSTALLED`; the `sc-observability-types` registry owns
the `SC_LOG_QUERY_*` codes. `LOG-001` through `LOG-005` are requirement IDs,
not diagnostic codes. The existing log-registry uniqueness test checks exact
string disjointness among the core, log, and types `error_codes::ALL` lists;
it does not claim workspace-wide uniqueness or cover codes outside those
enumerated registries. DTO and routing registry values retain their existing
meanings.

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
