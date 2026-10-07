# SC-Observability Requirements

**Status**: Approved baseline; Phase B additions in §10 and Phase C additions
in §11 proposed for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Source of truth**: [`api-design.md`](./api-design.md)
**Related ATM adapter docs**:
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)
- [`atm-quickstart.md`](./atm-quickstart.md)

## 1. Purpose And Scope

This document defines enforceable requirements for the standalone
`sc-observability` workspace.

The workspace exists to provide reusable Rust observability infrastructure with
clear layering:

1. neutral shared contracts
2. lightweight structured logging
3. typed observation routing layered on logging
4. OpenTelemetry/OTLP export layered on the lower-level crates

This workspace is explicitly not:

- a daemon-aware logging system
- an ATM-specific library
- a socket/spool/merge transport
- a runtime-home discovery mechanism

## 1.1 Approval Scope

This requirements document is sufficient to approve the shared workspace
direction and enforce the standalone crate boundaries.

It is not, by itself, the full ATM migration specification. ATM-specific
compatibility, durability, health projection, and env/config translation
requirements are defined separately in [`atm-adapter-requirements.md`](./atm-adapter-requirements.md).

## 2. Layered Dependency Order

The required dependency order is:

```text
sc-observability-types
  <- sc-observability
    <- sc-observe
      <- sc-observability-otlp
```

Layering requirements:

- LAY-001 `sc-observability-types` shall be the shared neutral base and shall not depend on any other workspace crate.
- LAY-002 `sc-observability` shall depend on `sc-observability-types` only.
- LAY-003 `sc-observe` shall depend on `sc-observability-types` and `sc-observability`.
- LAY-004 `sc-observe` shall not depend on `sc-observability-otlp`.
- LAY-005 `sc-observability-otlp`'s only normal sc-* dependency is `sc-observability-types`; `sc-observability` and `sc-observe` are dev-dependencies of its tests.
- LAY-006 Higher-layer concerns shall not be required to understand or use lower-layer crates.
- LAY-007 `sc-observability` requirements shall remain fully self-contained and shall not include routing or OTLP concerns.

## 3. `sc-observability-types` Requirements

This crate owns shared neutral contracts only.

- TYP-001 `sc-observability-types` shall own shared value types, identifiers, diagnostics, health contracts, and the open cross-crate traits used across the workspace.
- TYP-002 `sc-observability-types` shall not own sinks, routing runtimes, exporters, OTLP transports, ATM helpers, or application-specific event types.
- TYP-003 `ErrorCode` shall be a stable string-like type using namespace prefixes and `SCREAMING_SNAKE_CASE`.
- TYP-004 `Diagnostic` shall carry code, message, optional cause, mandatory remediation, optional docs reference, and structured details.
- TYP-005 One `Diagnostic` shall be reusable across CLI rendering, JSON error rendering, log attachment, span attachment, and health summaries.
- TYP-006 `DiagnosticInfo` shall retain its published sealed trait contract for workspace-defined errors that expose a `Diagnostic`; Phase B shall not change its sealing or add required methods.
- TYP-007 `ErrorContext` shall not be directly constructible without remediation.
- TYP-008 `Timestamp` shall be a UTC-enforcing public newtype. Public constructors and serde input shall normalize to UTC, serde output shall use stable UTC-only RFC3339 form, and raw non-UTC `OffsetDateTime` values shall not cross the public API boundary.
- TYP-009 `TraceContext` shall be limited to generic W3C-style trace correlation only.
- TYP-010 `TraceContext` shall use `TraceId` and `SpanId` newtypes rather than raw strings.
- TYP-011 `TraceId` shall validate 32-character lowercase hex W3C trace IDs.
- TYP-012 `SpanId` shall validate 16-character lowercase hex W3C span IDs.
- TYP-013 Request, session, runtime, and application metadata shall not be part of `TraceContext`.
- TYP-014 Span lifecycle shall use the canonical `sc_observability_types::v2` span record types.
- TYP-015 `SpanRecord<SpanStarted>` shall have the only public constructor.
- TYP-016 `SpanRecord<SpanEnded>` shall be reachable only through `SpanRecord<SpanStarted>::end(...)`.
- TYP-017 Producer-facing `SpanRecord<S>` fields shall be private, with read access through accessors.
- TYP-018 Final span duration shall be exposed only on `SpanRecord<SpanEnded>`.
- TYP-019 Canonical `sc_observability_types::v2` span-state serialization shall not expose a producer-facing mutable state field.
- TYP-020 `Observable` shall remain an open trait for consumer-owned payload types.
- TYP-021 `ObservationSubscriber<T>`, `ObservationFilter<T>`, `LogProjector<T>`, `SpanProjector<T>`, and `MetricProjector<T>` shall remain open extension points.
- TYP-023 Traits used behind `Arc<dyn ...>` shall remain object-safe, with `T` fixed at each usage site.
- TYP-024 Traits used in concurrent routing or injection contexts shall be `Send + Sync`.
- TYP-025 `ToolName` shall be owned by `sc-observability-types`, wrap a validated string identifier, and represent the top-level tool or executable identity used for config and path derivation.
- TYP-026 `EnvPrefix` shall be owned by `sc-observability-types`, wrap an uppercase validated prefix without a trailing underscore, and represent environment-loading namespaces.
- TYP-027 `ServiceName` shall be owned by `sc-observability-types`, wrap a validated string identifier, and represent the service name carried in logs and telemetry.
- TYP-028 `TargetCategory` shall be owned by `sc-observability-types`, wrap a validated dotted or snake-compatible category identifier, and represent the stable subsystem namespace on `LogEvent`.
- TYP-029 `ActionName` shall be owned by `sc-observability-types`, wrap a validated dotted or snake-compatible action identifier, and represent the stable event action name on `LogEvent`.
- TYP-030 All shared error types, health report types, and shared constants shall be owned by `sc-observability-types` as a single source of truth. Crate-specific error enums and concrete health report types are defined here and re-exported by their respective crates where needed. The proposed companion-only exception is scoped in PHB-002 and ADR-011; it does not relocate any published core type.
- TYP-031 Per-crate `constants.rs` files in higher-layer crates may exist only for crate-local values that are not shared across crate boundaries.
- TYP-032 `sc-observability-types` shall own the stable historical/follow query contracts: `LogQuery`, `LogOrder`, and `LogFieldMatch`.
- TYP-033 `LogQuery` shall support filtering by `service`, `levels`, `target`, `action`, `request_id`, `correlation_id`, `since`, `until`, `field_matches`, `limit`, and `order`.
- TYP-034 `sc-observability-types` shall own `LogSnapshot` as the stable synchronous result contract returned by historical query and follow polling APIs, with `events` and `truncated` as its stable fields.
- TYP-035 `sc-observability-types` shall own `QueryError` with variants `InvalidQuery`, `Io`, `Decode`, `Unavailable`, and `Shutdown`.
- TYP-036 `QueryError` shall map to stable error codes `SC_LOG_QUERY_INVALID_QUERY`, `SC_LOG_QUERY_IO`, `SC_LOG_QUERY_DECODE`, `SC_LOG_QUERY_UNAVAILABLE`, and `SC_LOG_QUERY_SHUTDOWN`.
- TYP-037 `sc-observability-types` shall own `QueryHealthReport` and `QueryHealthState` as the shared health contract for log query/follow availability.
- TYP-038 `sc-observability-types` shall own `ObservabilityHealthProvider` as a shared telemetry-health trait supported for workspace-owned implementations. External implementations are technically possible but unsupported and undertaken at the implementor's own risk; compatibility for external implementations is not guaranteed. The hidden implementation hooks do not enforce compiler-level sealing.
- TYP-039 `sc-observability-types` shall not expose concrete logging runtime
  behavior such as `Logger`, `LoggerBuilder`, `LogSink`, `SinkRegistration`,
  built-in sink implementations, or sink-configuration toggles. Downstream
  consumers that need those behaviors shall depend on `sc-observability`.
- TYP-040 `sc-observability-types` shall own `WriterState` as the shared
  logging-runtime writer-health enum used by `LoggingHealthReport`.

## 4. `sc-observability` Requirements

This crate is the lightweight logging layer.

- LOG-001 `sc-observability` shall provide a lightweight logging surface usable without `sc-observe` or `sc-observability-otlp`.
- LOG-002 `sc-observability` shall expose `Logger` and `LoggerConfig`.
- LOG-003 `sc-observability` shall own local structured logging concerns only.
- LOG-004 `sc-observability` shall expose `LogSink` as an open extension point and preserve object-safety for `Arc<dyn LogSink>`.
- LOG-005 `sc-observability` shall provide built-in JSONL file sink support.
- LOG-006 `sc-observability` shall provide a built-in human-readable console sink.
- LOG-007 `sc-observability` shall support multi-sink fan-out.
- LOG-008 The built-in file sink shall use the default layout `<log_root>/logs/<service_name>.log.jsonl`.
- LOG-009 The log root shall be redirectable via environment helper, with explicit config taking precedence.
- LOG-010 Redaction shall run before sink fan-out.
- LOG-011 `RedactionPolicy` shall support built-in denylist and bearer-token redaction.
- LOG-012 `RedactionPolicy` shall support consumer-provided `Redactor` implementations.
- LOG-013 Sink filtering shall be sink-local policy, not producer burden.
- LOG-014 Invalid log events shall fail fast, surfacing as
  `LogError::InvalidEvent(EventError)` from `log()` and
  `TryLogError::InvalidEvent(EventError)` from `try_log()`.
- LOG-015 Sink failures after validation shall be fail-open and shall not block the caller’s core flow.
- LOG-016 Logging health shall expose `LoggingHealthReport`,
  `LoggingHealthState`, `SinkHealth`, and typed `SinkHealthState` (defined in
  `sc-observability-types` and re-exported by `sc-observability`). For the
  writer-thread runtime, `LoggingHealthReport` shall also expose `queue_depth`,
  `queue_capacity`, `queue_high_water_mark`, `queue_full_drops_total`,
  `WriterState`, and `last_writer_error`.
- LOG-017 `sc-observability` shall not own typed observation routing.
- LOG-018 `sc-observability` shall not own OTLP transport or any OpenTelemetry dependency.
- LOG-019 `sc-observability` shall not own ATM-specific metadata rules, path conventions, or compatibility behavior.
- LOG-020 `LoggerConfig` shall define documented defaults for v1:
  - `level = Info`
  - `queue_capacity = 1024`
  - `rotation_max_bytes = ByteCount::from_mib(64)`
  - `rotation_max_files = FileCount::from_usize(10)`
  - `retention_max_age = RetentionMaxAge::from_days(7)`
  - `maintenance_cadence = MaintenanceCadence::new(60 s)`
  - `writer_shutdown_timeout = WriterShutdownTimeout::new(5 s)`
  - `maintenance_max_work_per_pass = None`
  - bearer-token redaction enabled
  - built-in file sink enabled
  - built-in console sink disabled
- LOG-021 Zero-configuration logging shall produce structured JSONL output using the built-in file sink and shall not require any OTLP or routing configuration.
- LOG-022 The logging layer shall not expose or assume an HTTP health endpoint; health is available through in-process health objects only.
- LOG-023 `Logger` lifecycle behavior shall be explicit:
  - `Logger::shutdown()` consumes `Logger<Running>` and returns `Logger<Stopped>`
  - `Logger::shutdown()` waits for definitive writer-thread completion before
    returning `Logger<Stopped>`
  - `log()`, `try_log()`, `flush()`, deprecated `emit()`, `query()`, and
    `follow()` are available only on `Logger<Running>`
  - `log()` blocks until queue admission and does not guarantee durability
  - `try_log()` is non-blocking and returns explicit queue-full failure
  - exceeding `writer_shutdown_timeout` records degraded shutdown health but
    does not permit the writer thread to continue detached after
    `Logger::shutdown()` returns
  - `Logger<Stopped>` remains usable for health inspection only
  - logger-created `LogFollowSession::poll()` after `shutdown()` returns `QueryError::Shutdown`
- LOG-024 `Logger::emit` shall remain the retained logger entry point for event admission and preserve its `EventError` behavior.
- LOG-025 `Logger` shall expose a synchronous historical query API `query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError>`.
- LOG-026 `Logger` shall expose a synchronous follow/tail API `follow(&self, query: LogQuery) -> Result<LogFollowSession, QueryError>`.
- LOG-027 `LogFollowSession` shall expose synchronous polling and shall not require an async runtime, background task, or file watcher to deliver new records.
- LOG-028 `sc-observability` shall provide `JsonlLogReader` as an independent JSONL file reader for historical query and follow operations without requiring a live `Logger`, `sc-observe`, or `sc-observability-otlp`.
- LOG-029 Historical query and follow behavior shall operate over the active JSONL log and its rotation set using the documented `sc-observability` naming/layout rules.
- LOG-030 Rotation handling for query/follow shall avoid duplicating or silently skipping committed log records when the active file is renamed or recreated on Unix-family platforms. On Windows, the implementation shall use stable filesystem identity metadata when available so append-vs-recreate detection does not degrade into length-based best-effort behavior.
- LOG-031 `LoggingHealthReport` shall expose query/follow availability through an optional `QueryHealthReport`.
- LOG-032 Query/follow APIs shall remain usable in logging-only deployments and shall not introduce ATM-specific types, daemon requirements, or `agent-team-mail-*` dependencies.
- LOG-033 `JsonlLogReader` query/follow operations shall remain independent of `Logger` lifecycle and shall stay usable for offline inspection after a logger-owned runtime shuts down.
- LOG-034 `ConsoleSink` shall expose public `stdout()` and `stderr()` convenience constructors. Arbitrary writer selection shall not be added to the public v1 surface.
- LOG-035 `sc-observability` shall expose a public retained-sink fault-injection surface for live validation runs, gated behind `#[cfg(test)]` or a dedicated `fault-injection` feature, with at least `degraded` and `unavailable` states.
- LOG-036 Retained-sink fault injection shall live in the retained-sink layer and exercise the same health-state transitions consumers observe in production, without filesystem sabotage or internal-only hooks.
- LOG-037 `sc-observability` shall be the concrete downstream integration
  surface for logging-only consumers that need logger construction, sink
  toggles, custom sink registration, `Logger::health()`, or `Logger::shutdown()`.
- LOG-038 Logging-only downstream consumers may keep their own local event or
  observer abstractions and adapt them into `Logger`; `sc-observability` shall
  not require those consumers to adopt `sc-observability-types` as their
  application-facing observer API.
- LOG-039 `sc-observability` shall own retained-log lifecycle management as an
  additive logging-layer capability rather than leaving rotation-pruning
  maintenance to downstream applications.
- LOG-040 The retained-log policy surface shall expose additive configuration
  for `rotation_max_bytes`, `rotation_max_files`, `retention_max_age`,
  `maintenance_cadence`, `writer_shutdown_timeout`, and
  `maintenance_max_work_per_pass`, using strong public newtypes for bytes and
  maintenance timing fields. `retention_max_age` supersedes the prior
  `retention.max_age_days` field.
- LOG-041 Retained-log maintenance shall run on the writer thread during idle or post-batch windows, stay off the producer hot path, and shall not require an async runtime dependency.
- LOG-042 Downstream applications shall configure retained-log policy through
  `sc-observability` config only and shall not need to spawn, join, or manage
  a separate prune or rotation worker.
- LOG-043 The public retained-log policy surface shall have documented
  defaults, and any public config type added for that surface shall support the
  same serialization conventions already used by the surrounding logging
  configuration.
- LOG-044 Logging health shall expose retained-log maintenance status including
  the last maintenance pass timestamp, rotated/pruned totals, last maintenance
  error, and worker state. The normative worker states are `Running`,
  `Degraded`, and `Stopped`.
- LOG-045 Retained-log maintenance failures shall be fail-open, shall not crash
  the logger, and shall not block or interfere with the emit path.
- LOG-046 `Logger::shutdown()` shall drain queued events, record timeout/degraded
  state in health or error reporting when shutdown exceeds the configured
  timeout threshold, and return `Logger<Stopped>` only after the writer thread
  has definitively stopped.
- LOG-047 `LogError` shall be the blocking queue-admission error surface for
  `Logger::log(...)` and shall include `WriterDegraded` and
  `ShutdownTimedOut` variants in addition to invalid-event rejection.
- LOG-048 `TryLogError` shall be the non-blocking queue-admission error surface
  for `Logger::try_log(...)` and shall include `QueueFull`,
  `WriterDegraded`, and `ShutdownTimedOut` variants in addition to
  invalid-event rejection.

### 4.2 Consumer Documentation Requirements

- DOC-001 `README.md` shall be a real consumer entrypoint that includes a workspace crate summary, a "which crate do I need?" decision table, one minimal logging-only snippet, and "start here" links into deeper docs.
- DOC-002 A root-level `CONSUMING.md` shall exist and cover logging-only setup, the default log root/path, `SC_LOG_ROOT` behavior, enable/disable controls for file and console sinks, custom sink registration, `Logger::health()` usage, and links to deeper docs.
- DOC-003 A runnable `examples/custom-sink-example/` shall exist and demonstrate a public-only `LogSink` implementation, `LoggerBuilder`, `SinkRegistration`, optional `LogFilter`, builder-time sink registration, and `logger.health()`.
- DOC-004 Default sink behavior, path layout, and environment override behavior shall be documented in a consumer-facing section, which may live in `CONSUMING.md`.
- DOC-005 The normative docs shall include an explicit downstream
  `sc-compose` logging-only integration contract that states the exact split between
  `sc-observability-types` and `sc-observability`.
- DOC-006 The `sc-compose` integration contract shall state that
  `sc-observability-types` provides shared `LogEvent`, health, diagnostics, and
  identifier contracts, while `sc-observability` provides `Logger`,
  `LoggerConfig`, `LoggerBuilder`, `LogSink`, `SinkRegistration`, built-in
  sinks, sink toggles, `Logger::health()`, and `Logger::shutdown()`.
- DOC-007 The same contract shall document the approved logging-only wiring for
  `sc-compose`: `sc-composer` keeps a local observer layer with no direct
  dependency on `sc-observability-types`, and the CLI adapts that local layer
  into `sc-observability::Logger`.
- DOC-008 The same contract shall define the adapter-owned mapping from
  `sc-compose` local observer events to `LogEvent` fields, including stable
  guidance for `target`, `action`, `outcome`, `diagnostic`, `message`, and
  structured `fields`.
- DOC-009 The same contract shall explicitly define the local `sc-composer`
  observer injection model or cross-reference the controlling `sc-compose`
  normative section that defines it, including trait shape, callback/event
  source, and `dyn`-compatible injection semantics.
- DOC-010 OTel integration for `sc-compose` is out of scope for this
  logging-only contract and is deferred to a future sprint if `sc-compose`
  later adopts `sc-observe` or `sc-observability-otlp`.

### 4.3 Consumer Usability Issue Traceability

| GitHub issue | Scope | Requirement IDs |
| --- | --- | --- |
| #20 | consumer onboarding docs and custom sink example | DOC-001, DOC-002, DOC-003, DOC-004 |
| #21 | default file sink path simplification | LOG-008 |
| #55 | public console writer parity | LOG-034 |
| #57 | public retained-sink fault injection | LOG-035, LOG-036 |
| #70 | retained-log rotation, pruning, and maintenance | LOG-039, LOG-040, LOG-041, LOG-042, LOG-043, LOG-044, LOG-045, LOG-046 |

### 4.4 Query/Follow Issue Traceability

| GitHub issue | Scope | Requirement IDs |
| --- | --- | --- |
| #24 | `LogQuery`, `LogOrder`, `LogFieldMatch` contract | TYP-032, TYP-033 |
| #25 | `QueryError` surface and stable error codes | TYP-035, TYP-036 |
| #26 | historical query API and `LogSnapshot` result | TYP-034, LOG-025, LOG-029 |
| #27 | follow/tail API and synchronous polling | LOG-026, LOG-027, LOG-030 |
| #28 | query health signal on logging health | TYP-037, LOG-031 |
| #29 | independent `JsonlLogReader` surface | LOG-028, LOG-029, LOG-032 |

## 5. `sc-observe` Requirements

This crate is the observation routing layer built on top of logging.

- OBS-001 `sc-observe` shall expose `Observability` as the producer-facing routing service.
- OBS-002 `Observability` shall emit `Observation<T>` values rather than raw payloads.
- OBS-003 `Observation<T>` shall carry shared envelope metadata: version, timestamp, service, process identity, optional trace context, and payload.
- OBS-004 `sc-observe` shall own subscriber registration, projector registration, routing, filtering, and fan-out for typed observations.
- OBS-005 Registrations shall be construction-time only and shall close when `Observability` is built.
- OBS-006 No runtime registration after `Observability::new(...)` or `ObservabilityBuilder::build()` shall be part of v1.
- OBS-007 Subscriber and projection registrations shall be `Send + Sync`.
- OBS-008 Matching registrations shall execute in deterministic registration order.
- OBS-009 Failure in one subscriber or projector shall not prevent later matching registrations from running.
- OBS-010 If no active or eligible subscriber/projector path remains for an observation, emission shall return `ObservationError::RoutingFailure`.
- OBS-011 Calling `Observability::emit()` after `shutdown()` shall return `ObservationError::Shutdown`.
- OBS-012 `ObservationError` shall provide named runtime-guard variants for at least `Shutdown`, `QueueFull`, and `RoutingFailure`.
- OBS-013 `ObservabilityBuilder` shall support construction-time registration of subscribers and projections.
- OBS-014 `sc-observe` shall depend on `sc-observability` for logging integration.
- OBS-015 `sc-observe` shall not depend on `sc-observability-otlp`.
- OBS-016 `sc-observe` shall route typed observations into logging and generic downstream extension points without taking ownership of OpenTelemetry transport concerns.
- OBS-017 `ObservabilityConfig` shall be the top-level configuration for the routing runtime and logging integration.
- OBS-018 `ObservabilityConfig` shall not own OTLP transport configuration.
- OBS-019 `sc-observe` shall expose `ObservabilityHealthReport` as the top-level runtime health view.
- OBS-020 `ObservabilityHealthReport` shall summarize dropped observations, subscriber failures, projection failures, and downstream attached service health where available.
- OBS-021 `sc-observe` shall not own application-specific observation payloads or ATM compatibility behavior.
- OBS-022 Boot-phase observability shall initialize before plugin or adapter registration so early lifecycle events can be recorded without ATM-specific context.
- OBS-023 `ObservabilityConfig` shall define documented defaults for v1:
  - `env_prefix` derived from `ToolName`
  - `queue_capacity = 1024`
  - logging defaults inherited from `LoggerConfig`
  - no HTTP health endpoint
- OBS-024 `Observability` lifecycle behavior shall be explicit:
  - `emit()` after `shutdown()` returns `ObservationError::Shutdown`
  - `flush()` delegates to logging and active routing/projector state
  - repeated `shutdown()` calls after a successful shutdown are idempotent and return `Ok(())`
  - repeated `shutdown()` calls after a failed shutdown replay the retained terminal failure to callers

## 6. `sc-observability-otlp` Requirements

This crate is the OTel/OTLP layer built on `sc-observability-types`.

- OTLP-001 `sc-observability-otlp` shall provide the OTLP-backed telemetry surface.
- OTLP-002 `sc-observability-otlp` shall expose `v2::Telemetry` (`RuntimeTelemetry`) and `v2::TelemetryConfig`.
- OTLP-003 `sc-observability-otlp` shall own all OpenTelemetry and OTLP transport concerns.
- OTLP-004 `v2::OtelConfig.protocol` shall be a typed `v2::OtlpProtocol` enum rather than a free-form string.
- OTLP-005 Invalid OTLP transport configuration shall fail at `RuntimeTelemetry::new(...)` with `sc_observability_types::v2::InitError`.
- OTLP-006 `RuntimeTelemetry` emit methods shall return `sc_observability_types::v2::TelemetryError`.
- OTLP-007 Calling an emit method after `shutdown()` shall return `v2::TelemetryError::Shutdown`.
- OTLP-008 The canonical runtime `V2SpanAssembler` shall buffer a started span, attach events, and emit `sc_observability_types::otlp::OtlpCompleteSpan` only when the span ends.
- OTLP-009 In-flight started spans without a matching end shall be dropped at flush/shutdown and counted as dropped exports.
- OTLP-010 The internal trace-export path shall export `sc_observability_types::otlp::OtlpCompleteSpan`, not a raw span signal.
- OTLP-011 crate-local `LogExporter`, `TraceExporter`, and `MetricExporter` contracts may remain object-safe for `Arc<dyn ...>`, but they are implementation details rather than public extension points.
- OTLP-012 Exporter failures after validation shall be fail-open and shall update health and dropped-export counters.
- OTLP-013 Telemetry health shall expose `TelemetryHealthReport`,
  `ExporterHealth`, and typed `ExporterHealthState` (defined in
  `sc-observability-types` and re-exported by `sc-observability-otlp`).
- OTLP-014 `sc-observability-otlp`'s only normal sc-* dependency is `sc-observability-types`; `sc-observe` is a dev-dependency of its tests.
- OTLP-015 `sc-observability-otlp` shall attach OTel behavior using lower-level routing and logging infrastructure from the crates beneath it.
- OTLP-016 `sc-observability-otlp` shall not push OTLP-specific requirements into `sc-observability`.
- OTLP-017 `sc-observability-otlp` shall attach to the routing layer by registering `LogProjector`, `SpanProjector`, and `MetricProjector` implementations with `ObservabilityBuilder`, not through direct internal access to `sc-observe` internals.
- OTLP-018 `v2::TelemetryConfig` shall be constructed independently of `ObservabilityConfig` and passed directly to `sc-observability-otlp` at setup time.
- OTLP-019 Zero-configuration OTLP behavior shall be disabled by default until the application explicitly enables telemetry or provides a valid endpoint.
- OTLP-020 `v2::TelemetryConfig` and `v2::OtelConfig` shall define documented defaults:
  - `enabled = false`
  - `protocol = HttpBinary`
  - `timeout_ms = 3000`
  - `max_retries = 3`
  - `initial_backoff_ms = 250`
  - `max_backoff_ms = 5000`
  - logs, traces, and metrics disabled unless explicitly configured
  This list remains the released 1.x baseline under ADR-020. The accepted
  D21 canonical backend contract is retained. D22 now owns the exact
  compatibility/namespace contract and D26 implements its adapter without
  redefining backend defaults or validation; D18 verifies both public paths.
- OTLP-021 Retired in Phase F. The former `Telemetry` lifecycle
  behavior shall be explicit as follows; until that acceptance these bullets
  are the gated Phase D candidate contract rather than an active requirement:
  - synchronous emit admission remains available for both backends; emit methods
    after async shutdown begins return `TelemetryError::Shutdown`
  - `flush_async_typed().await` completes only after every export admitted
    before its ordered barrier has a terminal outcome
  - `shutdown_async_typed().await` closes admission, drains every prior
    admission, performs provider/exporter shutdown exactly once, and surfaces
    any final export failure as `ShutdownFailure`
  - concurrent or repeated async shutdown callers share one completion; calls
    made after terminal completion are idempotent and return `Ok(())`, so only
    the first/in-flight caller set observes a terminal failure
  - the synchronous HTTP backend retains final-result
    `flush_typed()`/`shutdown_typed()` compatibility on plain threads; it
    returns `BlockingBackendInAsyncContext` before buffer/state mutation when
    called from an entered Tokio runtime
  - the SDK backend returns a typed `AsyncLifecycleRequired` from synchronous
    lifecycle before buffer/state mutation because it cannot block a Tokio
    worker for async completion
  - callers of the SDK backend keep the host runtime alive through awaited
    shutdown; premature runtime termination yields `RuntimeTerminated`, never
    false success, and accounts admitted-but-incomplete records as dropped
  - incomplete spans are dropped only at shutdown/final flush
- OTLP-022 `sc-observability-otlp` shall own crate-local sealed signal-emitter traits for direct telemetry injection where needed.
- OTLP-023 The synchronous HTTP/JSON backend (feature `sync-http`) is a
  first-class supported backend for callers without an async runtime. Its code,
  tests, dashboards, and operational recipes shall be maintained in this
  repository and shall not ship scratch paths, ATM dependencies/labels, or
  stale source-repository names.
- OTLP-024 The supported Grafana/LogQL operational recipes shall be translated
  to the current neutral resource/attribute schema, tested against the same
  hermetic collector corpus as both exporters, and stored under
  `docs/observability/otlp/`; legacy phase documents are evidence, not a public
  labeling contract.

## 6.1 ATM Out-Of-The-Box Baseline

The shared workspace shall document the ATM-shaped out-of-the-box baseline in
[`atm-quickstart.md`](./atm-quickstart.md).

- ATM-BASE-001 Zero-configuration shared-crate behavior for an ATM-shaped workload shall be documented explicitly, including log format, built-in sink behavior, redaction defaults, rotation defaults, queue defaults, and the absence of a built-in health endpoint.
- ATM-BASE-002 The minimal ATM production configuration surface shall be documented explicitly, covering logging, routing, OTLP attachment, and ATM-owned adapter responsibilities.
- ATM-BASE-003 Any ATM day-one gap between the shared design and ATM production needs shall be resolved in the shared docs or explicitly assigned to the ATM adapter boundary.

## 7. Non-Functional Requirements

- NFR-001 The workspace shall not require a daemon, broker, or external runtime for correctness.
- NFR-002 The logging-only crate shall remain lightweight enough for basic CLI use.
- NFR-003 Routing complexity shall remain isolated to `sc-observe`.
- NFR-004 OTLP transport complexity shall remain isolated to `sc-observability-otlp`.
- NFR-005 The design shall preserve object-safe trait boundaries for dynamic registration and fan-out.
- NFR-006 The workspace shall not mandate global mutable state for basic operation.
- NFR-007 Backend sink/export failures shall be fail-open.
- NFR-008 Each crate section in this document shall remain readable in isolation without requiring upward-layer concepts to understand lower-layer behavior.
- NFR-009 The workspace shall enforce layering and repo-boundary rules in CI, including dependency bans against `agent-team-mail-*` and banned crate edges that violate the approved stack.
- NFR-010 Changes to approved crate layering shall be reviewed for consistency across requirements, architecture, and API design documents. CI shall enforce missing-doc checks for the public Rust API; literal documentation phrases are not a CI contract.
- NFR-011 The workspace shall enforce version-literal consistency in CI for the
  maintained files covered by the validation script: Cargo package tables,
  internal workspace dependency version pins that reference local crate paths,
  and `RELEASE-NOTES*.md` documents. Within that tracked scope, every release
  version literal shall match `workspace.package.version`.
- NFR-012 A public item is removed only in a release after one that shipped it
  `#[deprecated]` behind `v1` under PHF-002. The merge gate is the stock check
  against committed `schema/api/rust-stock/<crate>/1.5.0.txt` baselines:
  additions pass, while a removed or changed line fails unless that sprint
  re-captures and commits its affected baseline in the same change.

## 8. Source Organization Requirements

- SRC-001 Each crate shall define its stable error codes in one dedicated source file or module owned by that crate.
- SRC-002 Each crate shall expose its public error codes from that single registry location so they can be reviewed, reported, and documented consistently.
- SRC-003 Shared non-trivial constants for a crate shall be defined in one dedicated constants file or module owned by that crate.
- SRC-004 Error-code registries and constants modules shall remain separate concerns; error-code definitions shall not be mixed into the general constants module.
- SRC-005 Non-trivial magic numbers shall not appear outside dedicated constants definitions, except for trivial language literals such as `0` and `1` where their meaning is self-evident.
- SRC-006 Policy values, limits, thresholds, retry counts, timeouts, and similar operational numbers shall be named constants rather than inline numeric literals.

## 9. Out Of Scope

- OOS-001 daemon-owned canonical file writing
- OOS-002 producer-to-daemon socket contracts
- OOS-003 spool-write and merge semantics
- OOS-004 runtime-home path derivation
- OOS-005 ATM-specific fields in the core schema
- OOS-006 ATM mailbox, plugin, and session contracts
- OOS-007 application-specific event taxonomies in the shared crates
- OOS-008 CLI success envelopes and exit-code conventions


## 10. Phase B Contracts — Retained 1.x Requirements

These requirements govern the released 1.x baseline and the current compatible
Phase D adoption under ADR-020. Architectural acceptance is recorded in the
ADRs below; acceptance does not itself assert implementation closure.
The [Phase B index](plans/phase-b/plan-phase-b.md) routes the authoritative sprint
contracts; [ADR-011 through ADR-015](architecture.md#adr-011-companion-boundaries-and-pre-copy-contract)
record the accepted architecture. No item below asserts implementation closure.

- PHB-001 sc-observability shall own and review the target bridge contract before
  BTIT completes its initial implementation. All foreseeable bridge changes,
  including runtime-level integration, shall be implemented and accepted in BTIT
  before B.1 mechanically copies the working reference. Contract approval, source
  SHA, accepted critical review and provenance are separate required evidence.
- PHB-002 The existing core dependency graph shall remain intact. The new bridge
  may own companion-specific lifecycle errors, health and facade constants;
  macros remain independent of the bridge. The DTO crate may own wire projections
  only. These are scoped TYP-030 exceptions, not permission to duplicate core
  diagnostics, move published definitions, or introduce Tauri/PyO3/log/runtime
  dependencies into neutral types. Runtime adapters convert companion failures
  to neutral DTOs without a reverse dependency from DTOs to the bridge. A shared
  native binding-runtime crate owns core/bridge backends and conversions for both
  Tauri and Python; language adapters do not repeat those runtime mappings.
- PHB-003 Superseded by PHF-002 (Phase F): deprecate before removing.
- PHB-004 For Phase B and the 1.x release line only, Issue #92 shall provide
  improved typed error implementations and usable
  improved operation/extension entry points, with total typed classification and
  mandatory diagnostic/remediation preservation. Unknown/custom legacy codes
  remain explicit unclassified failures. Existing source/backtrace data shall
  survive adapters; new code shall construct typed failures at their origin.
  Conversion shall not claim recovery of data already discarded by the original
  operation. DiagnosticSummary remains its existing optional-code/message/time
  shape; new OperationDiagnostic carries required code/message/remediation/time.
- PHB-005 For Phase B and the 1.x release line only, legacy interfaces shall
  remain functional with actionable compiler
  deprecation warnings only after working replacements exist. Removal and a
  breaking representation conversion remain unscheduled. Default-lint legacy
  consumer fixtures shall still work; migrated fixtures shall deny deprecated
  usage. Strict consumer warning policies may reject deprecations and shall be
  explained in upgrade guidance rather than suppressed globally.
- PHB-006 Existing adoption guidance shall cover incremental upgrades, exact
  symbol mappings, custom extension adapters, typed matching, diagnostic
  preservation and verification. A downstream fixture shall execute that guide.
  Qualification follows the [ADR-013 preflight amendment](architecture.md#adr-013-owner-controlled-shared-runtime-level).

The B.1e implementation record supplies the exact migration routing, warning
inventory, downstream Cargo fixtures and JSON diagnostic validator for
PHB-005/PHB-006. The typed methods and failures remain additive, and the
warning rollout was qualified for the selected `1.4.0` next-minor candidate. B.2
qualified the B.1e result for that release; B.7 alone owned publication. No removal schedule or
major-release claim is introduced.
- PHB-007 Issue #97 shall add one core-owned effective level shared by every
  producer path, with immutable LoggerConfig baseline and owner-only temporary
  elevate/reset operations. Attached handles have read access only. Existing
  constructors retain behavior; additional construction paths convey the sole
  mutation capability. Public core configuration/health structs stay unchanged;
  expose new state through additive accessors.
- PHB-008 Runtime changes shall return typed results, serialize against admission
  and shutdown, expose coherent baseline/effective/revision state, preserve
  admitted records, and reject below-baseline, unsupported, lifecycle and revision
  overflow requests without mutation. Repeated effective values are unchanged.
  No configuration persistence, timer or lease stack is implied.
- PHB-009 A committed change shall attempt bounded nonrecursive diagnostic
  admission and separately report its outcome. Diagnostic failure shall not
  roll back the change or become fatal. Redaction, sink policy and queue limits
  remain enforced. Runtime filtering shall not promise recovery of compiled-out
  sites; supported release builds and typed ceiling failures require evidence.
- PHB-010 New operational APIs shall use Rust Results or language discriminated
  unions for creation, validation, submission, observation and lifecycle.
  Expected failures shall not use deliberate panic/throw/raise/rejection control
  flow. Foreign failures shall be converted at boundaries. Existing infallible
  Rust accessors retain their signatures; standard log/Python logging protocols
  may keep required unit returns with inspectable best-effort failure accounting.
- PHB-011 Default emission shall be nonblocking and nonfatal; callers may ignore
  results. Dispatch, accepted admission, filtered admission, flush and persistence
  shall remain distinct. No failure shall be converted into success, recursively
  logged or allowed to create unbounded retries, helper threads or queues.
- PHB-012 TypeScript shall support the Tauri frontend first. The host owns policy,
  initialization, level-change authorization and shutdown; all front/backend
  records share its writer. Command operational failures resolve tagged envelopes,
  with explicit schema, integer, path and unknown-variant conversion rules.
- PHB-013 Python shall be first-class in owned and Rust-host-attached modes, using
  shared DTO/error contracts and one writer per owner. Attachment shall not gain
  lifecycle/level ownership. Standard logging/context integration and optional
  optional receipt/flush awaits shall preserve nonfatal typed outcomes and
  bounded work. Admission receipts are resolved on submit return; async flush
  observer timeout/cancellation does not cancel native work. Retained shutdown
  results are observable; timed-out flush has no prior-result retrieval API,
  and bridge-native timeout follows the documented separate adapter/native slots.
- PHB-014 Release closure shall require downloadable immutable artifacts and
  registry-only consumer evidence. For the 1.4.x release, B.P2 qualified immutable
  staged runtime-level artifacts before B.P3 BTIT integration; B.7 alone owned
  publication and registry-only consumer proof. Migrated Rust companions
  and subsequent language artifacts have their own release gates. Go, Node.js
  and sc-runtime process/interpreter topology remain deferred. #96 configuration
  loading is independent.
  Qualification follows the [ADR-013 preflight amendment](architecture.md#adr-013-owner-controlled-shared-runtime-level).

## 11. Phase C Contracts — Retained Publishing Constraints

These requirements retain the shared publishing and preflight constraints.
Architectural acceptance is recorded in ADR-016; this text grants no new
publication authority or assertion of implementation closure.
The [Phase C index](plans/phase-c/plan-phase-c.md) routes the authoritative
sprint contracts; [ADR-016](architecture.md#adr-016-shared-publishing-pipeline-adoption)
records the accepted architecture. Phase C shall not publish, tag, or execute
BTIT integration tests; current Phase D work also has no publication authority.

- PHC-001 The repository-specific publishing implementation (release
  workflows, the publisher agent, the manifest/gate scripts and the legacy
  manifest schema) shall be replaced by the pinned `../sc-publish` shared
  package, installed only through its `install.py` caller-owned JSON
  contract. Phase C shall pin and document the exact reviewed shared-package
  revision; an unpinned or silently-updated shared revision does not satisfy
  this requirement.
- PHC-002 The shared package's channel set (`github_release`, `crates_io`,
  `pypi`, `homebrew`, `scoop`, `winget`) does not include an npm channel.
  Phase C shall not silently drop npm publication for the TypeScript client,
  fabricate npm support that does not exist in the shared package, or
  substitute a repository-local npm publish workflow as if it were
  equivalent shared-package adoption. Phase C shall treat an npm channel in
  `../sc-publish`, owned upstream and consumed at a reviewed pin, as a named
  execution prerequisite for the sprint that installs the shared package. If
  that upstream capability cannot land before Phase C needs to execute,
  Phase C shall stop and obtain an explicit owner decision (delay execution,
  or accept a documented, owner-signed-off temporary gap) rather than
  closing on a local workaround or a silent omission.
- PHC-003 Phase C sprints preflight only. They shall not authorize or
  execute publication to any channel, create or push a release tag, or run
  BTIT repository integration tests. Preflight commands shall be
  idempotent, non-mutating with respect to any external registry, and
  independently re-runnable.
- PHC-004 The release-surface preflight shall enumerate, without silent
  omission, every Phase B-added publishable Rust crate, the Python
  wheel/sdist artifacts, the npm client package, and applicable native/Tauri
  artifacts, cross-checked against a deterministic manifest inventory. A
  crate or package present in the merged Phase B tree but absent from the
  preflight inventory is a defect, not an accepted gap.
- PHC-005 Phase C shall document the exact credential/authentication model
  required by each adopted channel at its actual scope — for example, the
  shared PyPI workflow's `PYPI_API_TOKEN`/`TEST_PYPI_API_TOKEN` secrets are
  GitHub Environment-scoped (`pypi`/`testpypi`), distinct from crates.io's
  repository-scoped `CARGO_REGISTRY_TOKEN` — without assuming unimplemented
  mechanisms such as trusted publishing, without collapsing environment
  scope into repository scope, and without inspecting, printing, or
  otherwise exposing secret values. Verification shall check secret
  presence at the correct scope (`gh secret list --env <name>` for
  environment-scoped secrets), not merely at repository scope.
- PHC-006 Installing shared-package workflows shall not silently regress
  CI runtime/action versions already adopted elsewhere in this repository.
  Phase C shall treat a `../sc-publish` revision with current, compatible
  action-runtime pins as a named execution prerequisite for the sprint that
  installs the shared package, verified when adopting the reviewed upstream
  revision — not recorded as an accepted regression closed out by a follow-up
  ticket.

## 12. Phase D — Compatible 1.x Adoption

ADR-020 records the Phase F deprecate-before-remove decision. The next release
keeps version 1.5.0 for every published crate; there is no major version bump.

- PHD-001 Superseded by PHF-002 (Phase F): deprecate before removing.
- PHD-002 Superseded by PHF-002 (Phase F): deprecate before removing.
- PHD-003 OTLP shall provide both an official SDK/Tokio backend requiring a caller-owned runtime and a bounded plain-thread synchronous HTTP/JSON backend (feature `sync-http`) for callers without an async runtime. They shall share crate-private contracts, ordered admission/lifecycle barriers, deadlines and health/drop accounting. Backend/protocol/runtime combinations shall be validated at construction, and enabled transports shall never silently fall back to no-op.
- PHD-004 Preserve the accepted Phase D canonical OTLP config/default/validation behavior: queue bounds limit record count and aggregate bytes, explicit validated config is not overridden by ambient OTEL_* values, and both backends satisfy the retired OTLP-021 lifecycle design. D22 specifies compatibility with released config literals/defaults; D26 supplies minimal adapters without changing backend contracts. Incompatible new configuration owners use the canonical namespace while the released root configuration retains its behavior.

## 13. Phase F — Purpose test and migration path

- PHF-001 Every published crate exposes only its 2.0 logging, observation-routing and OTel export primitives and their configuration. Application code and examples live in exactly one designated place: `examples/` or the consumer's own repository.
- PHF-002 Deprecate before removing. Every public 1.x item in every published crate ships behind its crate's default-on `v1` Cargo feature with `#[deprecated(note = "<2.0 replacement>")]` (an item with no replacement: `"removed; see docs/migration/phase-f.md"`), re-exported from its released path; the next release deletes the `v1` modules and features. A deprecated item need not keep working: it needs no compatibility adapters, tests or baseline comparison. Deleted in Phase F: public items already `#[deprecated]` in published 1.4.1, items never released, and internal plumbing. Test seams follow PHF-003. Every 2.0 item stays (Product Bar, CLAUDE.md). Canonical code never uses a `v1` item.
- PHF-003 A test seam that the instrumented Python wheel needs (`sc-observability-types` `test-double`, `sc-observability-binding-runtime` `test-hooks`, `sc-observability` `fault-injection` for revision exhaustion) stays behind its off-by-default feature, `#[doc(hidden)]`, and out of the published API snapshot. Every other test seam is `#[cfg(test)]`.
- PHF-004 `queue_capacity` is standard logger configuration with a recommended default that the logger enforces; no other layer carries a copy.

### Phase D wave 5 — Customer telemetry submission

These additive requirements extend the compatible 1.x baseline. Customer field
mapping remains outside the transport. These requirements cover logs, traces,
metrics and profiles, including the pinned development-version profile protocol.

- PHD-005 A shared Rust submission API shall accept logs, completed spans,
  metrics and profiles independently or together, preserving original UTC timestamps,
  structured attributes, resource/scope and supplied valid correlation. Paired
  logs/spans without IDs receive shared IDs; conflicting supplied IDs or timing
  return typed errors. Historical records without actual start time are not
  fabricated spans.
- PHD-006 Metric submission shall preserve gauge, sum, explicit histogram,
  exponential histogram and imported summary representations; numeric type,
  temporality, monotonicity, timestamps, distribution data and supported
  exemplars shall not be flattened or silently dropped. Profile samples, stacks,
  dictionaries, units, timestamps and links shall be preserved and references
  validated against the pinned profile schema. Span events/links/status
  and structured log bodies shall survive export. New contracts remain additive.
- PHD-007 Successful durable admission shall mean a versioned record is committed
  locally before the receipt returns. Pending data shall survive process restart.
  Capacity exhaustion, invalid payloads and persistence failure shall be explicit
  errors; pending data shall not be silently evicted. The store shall support
  bounded retention and coordinated access from Python and CLI processes.
- PHD-008 Delivery shall track each signal independently and support bounded flush,
  timeout, shutdown and retry after collector recovery. Admission and remote
  delivery shall be distinguishable. Document at-least-once delivery and its crash
  duplication window; stable import keys shall prevent duplicate local admission.
- PHD-009 Installed Python bindings shall expose shared configuration, submission,
  receipts, status and flush/shutdown with matching type stubs and typed errors.
  Existing logging APIs shall retain compatibility. `open`, `emit` (durable
  commit), `flush`, `shutdown` and `status` shall release the GIL; context exit
  shall not hide delivery errors. Release wheels shall enable the telemetry
  feature.
- PHD-010 An installable Rust CLI, the `sc-otel` binary from crate `sc-otel-cli`,
  shall expose the same submission contract through structured stdin and
  log/span/metric/profile options, plus validate, flush and status. Python and
  CLI shall use the same validation/config precedence. Exit status and machine
  output shall distinguish invalid input, admission failure and delivery failure.
- PHD-011 The sanity consumer shall map historical and live LLM/JEV records with
  configurable team/service/phase and a repository-specific PR URL template.
  Preserve reviewer identity, tested commit and original UTC timing. Persist
  source progress only after admission and handle partial lines, rotation,
  truncation and repeated imports without silent loss. Local time is display only.
- PHD-012 End-to-end tests shall submit through the installed Python package and
  actual CLI, then read back records from the pinned `otel-desktop-viewer`
  (v0.5.0) for every signal it supports, asserting values, timestamps and correlation. Collector capture
  shall verify remaining supported wire forms explicitly; it does not substitute
  for viewer readback or justify claiming unsupported viewer capabilities.
  Exercise offline recovery, process restart and partial signal delivery.
  Proof is automated query assertions; manual UI inspection is not proof.
- PHD-013 Full telemetry support shall be measured against the pinned upstream
  opentelemetry-proto v1.10.0 (Rust `opentelemetry-proto =0.33.0`): all four signal families, every payload variant
  and their fields must be represented through Rust, Python, CLI and the durable
  format. Serialization/export tests shall exercise the variants and nested
  non-default fields. Protocol maturity and viewer limitations shall be explicit,
  not used to silently remove product support. Any exclusion needs a user decision.
  Payload fields include the profile string-index forms and non-finite doubles
  (proto-JSON `"NaN"`, `"Infinity"`, `"-Infinity"`). `tracez.proto` (zPages) is
  not an OTLP payload and is outside this scope.


## Phase H — Native OpenTelemetry Simplification (proposed implementation)

Operator decisions 2026-10-07: remove the unused 1.5.0 OTel additions without a
deprecation release, preserve accepted v2 logging, use the official blocking
HTTP exporter for a thin shared sync client, and authorize minimal existing
LogSink integration for file/OTel/both. Implementation awaits plan approval.
For Phase H these requirements supersede custom OTel facade, dual custom
transport, durable admission, and full mirror-model requirements in OTLP-002,
OTLP-005..013, OTLP-017, OTLP-020..024 and PHD-005..013. They do not retire
logging requirements or erase historical release contracts.

- H-001 Existing synchronous file logging and accepted canonical v2 logging,
  macros, levels, redaction, query/follow, nonblocking admission, retention and
  binding lifecycle shall remain intact. OTel/Tokio dependencies stay above the
  core logging boundary. Current sc-compose and planned ATM logging consumers
  must remain supported; absence of current callers is not removal permission.
- H-002 The OTel path shall use official opentelemetry, opentelemetry_sdk and
  opentelemetry-otlp APIs and types. The Tokio path shall expose native exporters,
  providers, instruments and standard customization; no replacement signal model,
  provider facade, exporter trait, retry queue or transport implementation.
  Native synchronous recording remains synchronous; genuine exporter futures
  remain awaitable. Reuse existing LoggerConfig and native SDK configuration/builders; a shared
  application configuration supplies their values. No parallel config hierarchy
  or model layer. Minimal setup helpers are allowed only to remove duplication.
- H-003 A thin synchronous client shall use the official OTLP HTTP/protobuf
  exporter with its blocking reqwest client, sharing configuration with the
  Tokio path. It shall require no caller-owned Tokio runtime. Transport feature
  unification must not accidentally select an async HTTP client. Export methods
  shall return actual native exporter outcomes, not durable admission receipts
  or promises of remote persistence. No application database or extra queue.
- H-004 Minimal mapping through the existing LogSink extension point shall
  support file-only, OTel-only and both. The existing logger supplies one-event
  fanout, level filtering and redaction before either destination. Mapping uses
  native SDK log records. Preserve structured values and trace correlation or
  report unsupported values explicitly. File-only creates no OTel provider;
  OTel-only creates no file. Do not install a global subscriber implicitly,
  duplicate events through tracing/log bridges, block sink writes on network
  export, or shut down a caller-owned SDK provider. Macro and tracing input
  composition must be tested without replacing the existing file logger.
- H-005 sc-otel and Python PyO3 shall expose equivalent log/span/metric send
  operations over that one sync client and configuration. CLI is a thin Clap
  frontend and Python a thin language binding; neither owns a transport,
  durable store, retry worker or alternate payload model. Python blocking I/O
  releases the GIL. Native Rust APIs retain the full supported SDK surface;
  these frontends need not reimplement every native SDK instrumentation API.
- H-006 Remove custom SQLite storage, durable receipt/status/lease/replay APIs,
  custom SDK and sync_http implementations, superseded signal mirrors, and
  obsolete dependencies and tests of removed contracts. No v1 compatibility
  stubs for rejected 1.5.0 OTel additions. Preserve reused wire/behavior fixtures
  and shared logging types/errors. Never delete an existing user database
  automatically. No OTel migration notes, deprecation gates, compatibility tests or historical
  baseline-equivalence work. Profiles and
  private historical-envelope import APIs introduced with the rejected mirror
  contract are removed, not reimplemented around SDK 0.33.0. Delete other unused
  code identified in the affected boundaries as well. Before deleting any core,
  -log or -types symbol, obtain Solar (ATM BD replan owner) item-by-item
  confirmation against current and planned consumers; copy ATM team-lead.
  Preserve any symbol without that confirmation. Align official 0.33.0 features,
  shared configuration and resource conventions with Solar before finalizing
  the native-library sprint contract.
- H-007 Return native network timeout/rejection errors at the synchronous
  export boundary and document upstream partial-success handling accurately. SDK log admission is not a delivery
  receipt. Use standard exporter timeouts and lifecycle; do not claim cancelling
  a wait kills a blocking task or that every processor honors an overall timeout.
  The host retains its process-exit policy. Optional off-the-shelf Collector
  forwarding is deployment guidance, not an application-owned durable subsystem
  or a required new ATM dependency.
- H-008 File plus OTel simultaneous logging is the only new capability. Each
  sprint shall list any added wrapper and its necessity; default is none.
  Phase H shall finish with net deletion of source code, report actual
  added/deleted source and test lines separately, and preserve relevant tests.
  Reuse existing local collectors/capture fixtures to prove native sync and Tokio
  export, equivalent installed CLI/Python payloads, and file/OTel/both. Update
  generated Clap manual/web/installer documentation through the existing release
  pipeline. CI remains the publication gate, not a prerequisite for sanity/QA
  dispatch. No unrelated cleanup, new process framework, or release publication
  is included in this planning authorization.
