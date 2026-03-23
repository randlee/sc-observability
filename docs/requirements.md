# SC-Observability Requirements

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Source of truth**: [`api-design.md`](/Users/randlee/Documents/github/sc-observability-req/docs/api-design.md)

## 1. Purpose And Scope

This document defines the enforceable requirements for the standalone
`sc-observability` workspace.

The workspace exists to provide reusable Rust observability infrastructure for:

- structured logging
- typed observation routing
- OpenTelemetry export

The workspace is explicitly not:

- a daemon-aware logging system
- an ATM-specific library
- a socket/spool/merge transport
- a runtime-home discovery mechanism

## 2. Stakeholders And Consumers

Primary stakeholders:

- application and CLI maintainers who need lightweight structured logging
- application and service maintainers who need typed observation routing
- operators and developers who need OTLP export and health visibility
- downstream crate authors who need stable, generic shared contracts

Primary consumer expectations by crate:

- `sc-observability-types`: stable shared types, traits, diagnostics, and
  health contracts
- `sc-observability`: lightweight logging for basic CLIs with minimal runtime
  cost
- `sc-observe`: observation routing, fan-out, filtering, subscribers, and
  projectors
- `sc-observability-otlp`: OTLP-backed telemetry export and exporter health

## 3. Functional Requirements

### 3.1 Observation Emission

- REQ-001 [§2, §7.1] The workspace shall use an observation-first architecture in which producers emit one canonical observation and the system fans it out to downstream outputs.
- REQ-002 [§7.1] `sc-observe` shall expose `Observability` as the producer-facing routing service.
- REQ-003 [§7.1, §7.6] `Observability` shall emit `Observation<T>` values rather than raw payloads.
- REQ-004 [§7.5] `Observable` shall be an open trait for consumer-owned payload types.
- REQ-005 [§7.6] `Observation<T>` shall carry shared envelope metadata: version, timestamp, service, process identity, optional trace context, and payload.
- REQ-006 [§7.1] Calling `Observability::emit()` after `shutdown()` shall be invalid and shall return `ObservationError::Shutdown`.
- REQ-007 [§7.1] `ObservationError` shall provide named runtime guard variants for at least `Shutdown`, `QueueFull`, and `RoutingFailure`.
- REQ-008 [§7.4] `ObservationEmitter<T>`, `LogEmitter`, `SpanEmitter`, and `MetricEmitter` shall be sealed traits and shall not be intended for external implementation.

### 3.2 Observation Routing

- REQ-009 [§6.3, §10] `sc-observe` shall own subscriber registration, projector registration, routing, filtering, and fan-out for typed observations.
- REQ-010 [§10.1] `ObservationSubscriber<T>` shall be an open extension point for external consumer implementations.
- REQ-011 [§10.2-§10.4] `LogProjector<T>`, `SpanProjector<T>`, and `MetricProjector<T>` shall be open extension points for external consumer implementations.
- REQ-012 [§10.5] `ObservationFilter<T>` shall be an open extension point for external consumer implementations.
- REQ-013 [§10.5] Registrations shall be construction-time only and shall close when `Observability` is built.
- REQ-014 [§7.1, §10.5] No runtime registration after `Observability::new(...)` or `ObservabilityBuilder::build()` shall be part of v1.
- REQ-015 [§10.5] Subscriber and projection registrations shall be `Send + Sync`.
- REQ-016 [§10.5] Matching registrations shall execute in deterministic registration order.
- REQ-017 [§10.5] Failure in one subscriber or projector shall not prevent later matching registrations from running.
- REQ-018 [§7.1, §10.5] If no active or eligible subscriber/projector path remains for an observation, emission shall return `ObservationError::RoutingFailure`.
- REQ-019 [§10.1-§10.5] All routing traits used behind `Arc<dyn ...<T>>` shall remain object-safe.
- REQ-020 [§10.1] At each `Arc<dyn ...<T>>` site, `T` shall be fixed; object erasure shall apply to the implementation, not to the observation type.

### 3.3 Logging Surface

- REQ-021 [§6.2] `sc-observability` shall provide a lightweight logging surface usable without `sc-observe` or OTLP dependencies.
- REQ-022 [§11.1] `sc-observability` shall expose `Logger` and `LoggerConfig`.
- REQ-023 [§11.1, §7.2] Within `sc-observe`, `LoggerConfig` shall be derived from `ObservabilityConfig`; direct standalone `LoggerConfig` construction shall remain available in `sc-observability`.
- REQ-024 [§11.2] The built-in file sink shall use the default layout `<log_root>/<service_name>/logs/<service_name>.log.jsonl`.
- REQ-025 [§11.2, §16.1] The log root shall be redirectable via environment helper, with explicit config taking precedence.
- REQ-026 [§11.5] Redaction shall run before sink fan-out.
- REQ-027 [§11.5] `RedactionPolicy` shall support built-in denylist and bearer-token redaction.
- REQ-028 [§11.5] `RedactionPolicy` shall support consumer-provided `Redactor` implementations.
- REQ-029 [§11.7] `LogSink` shall be an open extension point and shall remain object-safe for `Arc<dyn LogSink>`.
- REQ-030 [§11.7] V1 built-in sink scope shall include JSONL file sink, human-readable console sink, and multi-sink fan-out.
- REQ-031 [§11.8] Sink filtering shall be sink-local policy, not producer burden.
- REQ-032 [§11.9] Invalid log events shall fail fast with `EventError`.
- REQ-033 [§11.9] Sink failures after validation shall be fail-open and shall not block the caller’s core flow.
- REQ-034 [§11.10] Logging health shall include `LoggingHealthReport`, `SinkHealth`, and `SinkHealthState`.

### 3.4 Telemetry Surface

- REQ-035 [§6.4, §12] `sc-observability-otlp` shall provide the OTLP-backed telemetry surface.
- REQ-036 [§12.1] `sc-observability-otlp` shall expose `Telemetry` and `TelemetryConfig`.
- REQ-037 [§12.1, §7.2] Within `sc-observe`, `TelemetryConfig` shall be derived from `ObservabilityConfig.otel`; direct standalone `TelemetryConfig` construction shall remain available in `sc-observability-otlp`.
- REQ-038 [§12.1.1] `OtelConfig.protocol` shall be a typed `OtlpProtocol` enum rather than a free-form string.
- REQ-039 [§12.1.1] Invalid OTLP transport configuration shall fail at `Telemetry::new(...)` with `InitError`.
- REQ-040 [§12.3] `Telemetry` emit methods shall return `TelemetryError`.
- REQ-041 [§12.3] Calling `emit_log()`, `emit_span()`, or `emit_metric()` after `shutdown()` shall return `TelemetryError::Shutdown`.
- REQ-042 [§12.4] `SpanAssembler` shall buffer `SpanSignal::Started`, attach `SpanSignal::Event`, and emit `CompleteSpan` only on `SpanSignal::Ended`.
- REQ-043 [§12.4] In-flight started spans without a matching end shall be dropped at flush/shutdown and counted as dropped exports.
- REQ-044 [§12.4] `LogExporter`, `TraceExporter`, and `MetricExporter` shall be open extension points and shall remain object-safe for `Arc<dyn ...>`.
- REQ-045 [§12.4] `TraceExporter` shall export `CompleteSpan`, not raw `SpanSignal`.
- REQ-046 [§12.5] Exporter failures after validation shall be fail-open and shall update health and dropped-export counters.
- REQ-047 [§12.6] Telemetry health shall include `TelemetryHealthReport`, `ExporterHealth`, and `ExporterHealthState`.

### 3.5 Error Model

- REQ-048 [§8.1] Error codes shall be stable string-like values with namespace prefixes and `SCREAMING_SNAKE_CASE` formatting.
- REQ-049 [§8.2] Remediation metadata shall be mandatory in structured diagnostics.
- REQ-050 [§8.2] Recoverable remediation shall be constructible only through `Remediation::recoverable(...)` and shall require at least one recovery step.
- REQ-051 [§8.3] `Diagnostic` shall carry code, message, optional cause, remediation, optional docs reference, and structured details.
- REQ-052 [§9.10] `DiagnosticInfo` shall be sealed.
- REQ-053 [§9.10] `ErrorContext` shall not be directly constructible without remediation.
- REQ-054 [§9.10] Public error types shall implement `Display` and `std::error::Error`.
- REQ-055 [§9.10] Error types that always carry diagnostics shall implement `DiagnosticInfo`.
- REQ-056 [§9.10] `ObservationError` and `TelemetryError` shall use enums for named runtime-guard variants while exposing contextual diagnostics on contextual variants.
- REQ-057 [§15] One `Diagnostic` shall be reusable across CLI rendering, JSON error rendering, log attachment, span attachment, and health summaries.

### 3.6 Configuration Model

- REQ-058 [§7.2] `ObservabilityConfig` shall be the top-level configuration for `Observability`.
- REQ-059 [§7.2] `ObservabilityConfig` shall include at least `tool_name`, `log_root`, `env_prefix`, `queue_capacity`, `rotation`, and optional `otel`.
- REQ-060 [§7.2] `ObservabilityBuilder` shall support construction-time registration of subscribers and projections.
- REQ-061 [§16] The configuration model shall be explicit-first.
- REQ-062 [§16] Explicit config shall override environment-derived config.
- REQ-063 [§16.2] Telemetry environment loading shall support both standard OTel names and custom prefixes.
- REQ-064 [§7.2, §12.1] `sc-observe` shall derive `LoggerConfig` and `TelemetryConfig` from `ObservabilityConfig` without removing standalone construction paths from the lower-level crates.

### 3.7 Span Lifecycle And Trace Correlation

- REQ-065 [§9.3-§9.6] Span lifecycle shall be encoded through typestate at the producer-facing API using `SpanRecord<SpanStarted>` and `SpanRecord<SpanEnded>`.
- REQ-066 [§9.4] `SpanRecord<SpanStarted>` shall have the only public constructor.
- REQ-067 [§9.4] `SpanRecord<SpanEnded>` shall be reachable only via `SpanRecord<SpanStarted>::end(...)`.
- REQ-068 [§9.4] Producer-facing `SpanRecord<S>` fields shall be private, with read access through accessors.
- REQ-069 [§9.4] Final duration shall be exposed only on `SpanRecord<SpanEnded>`.
- REQ-070 [§9.5-§9.6] Trace output shall support started spans, in-span events, and ended spans through `SpanSignal`.
- REQ-071 [§12.4] OTLP export shall assemble `SpanSignal` into `CompleteSpan`.
- REQ-072 [§8.8] `TraceContext` shall be limited to generic W3C-style trace correlation only.
- REQ-073 [§8.8] `TraceContext` shall use `TraceId` and `SpanId` newtypes rather than raw strings.
- REQ-074 [§8.8] `TraceId` shall validate 32-character lowercase hex W3C trace IDs.
- REQ-075 [§8.8] `SpanId` shall validate 16-character lowercase hex W3C span IDs.
- REQ-076 [§8.8] Request, session, runtime, and application metadata shall not be part of `TraceContext`.

### 3.8 Health Reporting

- REQ-077 [§7.3] `sc-observe` shall expose `ObservabilityHealthReport` as the top-level runtime health view.
- REQ-078 [§7.3] `ObservabilityHealthReport` shall summarize dropped observations, subscriber failures, projection failures, and downstream logging/telemetry health.
- REQ-079 [§11.10] Logging health shall distinguish healthy, degraded-dropping, and unavailable states.
- REQ-080 [§12.6] Telemetry health shall distinguish disabled, healthy, degraded, and unavailable states.
- REQ-081 [§7.3, §11.10, §12.6] Health reports shall expose the last structured diagnostic summary where available.

## 4. Non-Functional Requirements

- NFR-001 [§3, §4] The workspace shall not require a daemon, broker, or external runtime for correctness.
- NFR-002 [§6.2] The logging-only crate shall remain lightweight enough for basic CLI use.
- NFR-003 [§6.3] Observation routing complexity shall be isolated to `sc-observe`.
- NFR-004 [§6.4] OTLP transport complexity shall be isolated to `sc-observability-otlp`.
- NFR-005 [§7.4, §10, §11, §12] Traits used in concurrent routing/export contexts shall be `Send + Sync` where required by the design.
- NFR-006 [§16] The workspace shall not mandate global mutable state for basic operation.
- NFR-007 [§10, §11, §12] The design shall preserve object-safe trait boundaries for dynamic registration and fan-out.
- NFR-008 [§11.9, §12.5] Backend sink/export failures shall be fail-open.
- NFR-009 [§8.5] Canonical timestamps shall be UTC-only and stably serializable.

## 5. Crate Boundary Requirements

- BND-001 [§6.1] `sc-observability-types` shall own only neutral shared contracts, traits, diagnostics, health types, and shared value types.
- BND-002 [§6.1] `sc-observability-types` shall not own sinks, background workers, transport implementations, ATM helpers, or application-specific event types.
- BND-003 [§6.2] `sc-observability` shall own local structured logging and sink infrastructure only.
- BND-004 [§6.2] `sc-observability` shall not own typed observation routing, OTLP transport, or ATM-specific metadata behavior.
- BND-005 [§6.3] `sc-observe` shall own observation routing, filtering, fan-out, projectors, subscribers, and top-level routing health.
- BND-006 [§6.3] `sc-observe` shall not own application-specific observation payloads or ATM compatibility behavior.
- BND-007 [§6.4] `sc-observability-otlp` shall own OTLP exporters, transport concerns, batching, retry, timeout, flush, shutdown, and exporter health.
- BND-008 [§6.4] `sc-observability-otlp` shall not own local file logging or ATM-specific metadata behavior.
- BND-009 [§3.1, §6.5] The workspace dependency direction shall preserve `sc-observe -> sc-observability`, `sc-observe -> sc-observability-otlp`, and `sc-observability-types` as the shared base.
- BND-010 [§3.1] `sc-observe` shall be a workspace member before implementation begins and shall not introduce `agent-team-mail-*` dependencies.

## 6. Out Of Scope

- OOS-001 [§5] daemon-owned canonical file writing
- OOS-002 [§5] producer-to-daemon socket contracts
- OOS-003 [§5] spool-write and merge semantics
- OOS-004 [§5] runtime-home path derivation
- OOS-005 [§3, §5] ATM-specific fields in the core schema
- OOS-006 [§5] ATM mailbox, plugin, and session contracts
- OOS-007 [§5] application-specific event taxonomies in the shared crates
- OOS-008 [§5] CLI success envelopes and exit-code conventions
