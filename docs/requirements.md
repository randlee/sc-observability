# SC-Observability Requirements

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Source of truth**: [`api-design.md`](./api-design.md)
**Related ATM adapter docs**:
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)

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
- LAY-005 `sc-observability-otlp` shall sit at the top of the stack and may depend on `sc-observability-types`, `sc-observability`, and `sc-observe`.
- LAY-006 Higher-layer concerns shall not be required to understand or use lower-layer crates.
- LAY-007 `sc-observability` requirements shall remain fully self-contained and shall not include routing or OTLP concerns.

## 3. `sc-observability-types` Requirements

This crate owns shared neutral contracts only.

- TYP-001 `sc-observability-types` shall own shared value types, identifiers, diagnostics, health contracts, and trait definitions used across the workspace.
- TYP-002 `sc-observability-types` shall not own sinks, routing runtimes, exporters, OTLP transports, ATM helpers, or application-specific event types.
- TYP-003 `ErrorCode` shall be a stable string-like type using namespace prefixes and `SCREAMING_SNAKE_CASE`.
- TYP-004 `Diagnostic` shall carry code, message, optional cause, mandatory remediation, optional docs reference, and structured details.
- TYP-005 One `Diagnostic` shall be reusable across CLI rendering, JSON error rendering, log attachment, span attachment, and health summaries.
- TYP-006 `DiagnosticInfo` shall be sealed in `sc-observability-types`.
- TYP-007 `ErrorContext` shall not be directly constructible without remediation.
- TYP-008 Canonical timestamps shall be UTC-only and stably serializable.
- TYP-009 `TraceContext` shall be limited to generic W3C-style trace correlation only.
- TYP-010 `TraceContext` shall use `TraceId` and `SpanId` newtypes rather than raw strings.
- TYP-011 `TraceId` shall validate 32-character lowercase hex W3C trace IDs.
- TYP-012 `SpanId` shall validate 16-character lowercase hex W3C span IDs.
- TYP-013 Request, session, runtime, and application metadata shall not be part of `TraceContext`.
- TYP-014 Span lifecycle shall be encoded through typestate at the producer-facing API using `SpanRecord<SpanStarted>` and `SpanRecord<SpanEnded>`.
- TYP-015 `SpanRecord<SpanStarted>` shall have the only public constructor.
- TYP-016 `SpanRecord<SpanEnded>` shall be reachable only through `SpanRecord<SpanStarted>::end(...)`.
- TYP-017 Producer-facing `SpanRecord<S>` fields shall be private, with read access through accessors.
- TYP-018 Final span duration shall be exposed only on `SpanRecord<SpanEnded>`.
- TYP-019 `SpanState` serialization shall be derived from typestate at export/serialization time and shall not be a producer-facing mutable field.
- TYP-020 `Observable` shall remain an open trait for consumer-owned payload types.
- TYP-021 `ObservationEmitter<T>`, `LogEmitter`, `SpanEmitter`, and `MetricEmitter` shall be sealed traits.
- TYP-022 `ObservationSubscriber<T>`, `ObservationFilter<T>`, `LogProjector<T>`, `SpanProjector<T>`, and `MetricProjector<T>` shall remain open extension points.
- TYP-023 Traits used behind `Arc<dyn ...>` shall remain object-safe, with `T` fixed at each usage site.
- TYP-024 Traits used in concurrent routing or injection contexts shall be `Send + Sync`.

## 4. `sc-observability` Requirements

This crate is the lightweight logging layer.

- LOG-001 `sc-observability` shall provide a lightweight logging surface usable without `sc-observe` or `sc-observability-otlp`.
- LOG-002 `sc-observability` shall expose `Logger` and `LoggerConfig`.
- LOG-003 `sc-observability` shall own local structured logging concerns only.
- LOG-004 `sc-observability` shall expose `LogSink` as an open extension point and preserve object-safety for `Arc<dyn LogSink>`.
- LOG-005 `sc-observability` shall provide built-in JSONL file sink support.
- LOG-006 `sc-observability` shall provide a built-in human-readable console sink.
- LOG-007 `sc-observability` shall support multi-sink fan-out.
- LOG-008 The built-in file sink shall use the default layout `<log_root>/<service_name>/logs/<service_name>.log.jsonl`.
- LOG-009 The log root shall be redirectable via environment helper, with explicit config taking precedence.
- LOG-010 Redaction shall run before sink fan-out.
- LOG-011 `RedactionPolicy` shall support built-in denylist and bearer-token redaction.
- LOG-012 `RedactionPolicy` shall support consumer-provided `Redactor` implementations.
- LOG-013 Sink filtering shall be sink-local policy, not producer burden.
- LOG-014 Invalid log events shall fail fast with `EventError`.
- LOG-015 Sink failures after validation shall be fail-open and shall not block the caller’s core flow.
- LOG-016 Logging health shall include `LoggingHealthReport`, `SinkHealth`, and typed `SinkHealthState`.
- LOG-017 `sc-observability` shall not own typed observation routing.
- LOG-018 `sc-observability` shall not own OTLP transport or any OpenTelemetry dependency.
- LOG-019 `sc-observability` shall not own ATM-specific metadata rules, path conventions, or compatibility behavior.

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

## 6. `sc-observability-otlp` Requirements

This crate is the OTel/OTLP layer built on top of `sc-observe`.

- OTLP-001 `sc-observability-otlp` shall provide the OTLP-backed telemetry surface.
- OTLP-002 `sc-observability-otlp` shall expose `Telemetry` and `TelemetryConfig`.
- OTLP-003 `sc-observability-otlp` shall own all OpenTelemetry and OTLP transport concerns.
- OTLP-004 `OtelConfig.protocol` shall be a typed `OtlpProtocol` enum rather than a free-form string.
- OTLP-005 Invalid OTLP transport configuration shall fail at `Telemetry::new(...)` with `InitError`.
- OTLP-006 `Telemetry` emit methods shall return `TelemetryError`.
- OTLP-007 Calling `emit_log()`, `emit_span()`, or `emit_metric()` after `shutdown()` shall return `TelemetryError::Shutdown`.
- OTLP-008 `SpanAssembler` shall buffer `SpanSignal::Started`, attach `SpanSignal::Event`, and emit `CompleteSpan` only on `SpanSignal::Ended`.
- OTLP-009 In-flight started spans without a matching end shall be dropped at flush/shutdown and counted as dropped exports.
- OTLP-010 `TraceExporter` shall export `CompleteSpan`, not raw `SpanSignal`.
- OTLP-011 `LogExporter`, `TraceExporter`, and `MetricExporter` shall remain open extension points and object-safe for `Arc<dyn ...>`.
- OTLP-012 Exporter failures after validation shall be fail-open and shall update health and dropped-export counters.
- OTLP-013 Telemetry health shall include `TelemetryHealthReport`, `ExporterHealth`, and typed `ExporterHealthState`.
- OTLP-014 `sc-observability-otlp` shall depend on `sc-observe` rather than the other way around.
- OTLP-015 `sc-observability-otlp` shall attach OTel behavior using lower-level routing and logging infrastructure from the crates beneath it.
- OTLP-016 `sc-observability-otlp` shall not push OTLP-specific requirements into `sc-observability`.
- OTLP-017 `sc-observability-otlp` shall attach to the routing layer by registering `LogProjector`, `SpanProjector`, and `MetricProjector` implementations with `ObservabilityBuilder`, not through direct internal access to `sc-observe` internals.
- OTLP-018 `TelemetryConfig` shall be constructed independently of `ObservabilityConfig` and passed directly to `sc-observability-otlp` at setup time.

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
- NFR-010 The workspace shall enforce basic docs consistency checks in CI so the approved crate layering does not drift out of sync across requirements, architecture, and API design documents.

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
