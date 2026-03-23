# SC-Observability Architecture

**Status**: Draft for review
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`api-design.md`](./api-design.md)

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
- `TraceContext`, `TraceId`, `SpanId`
- `SpanRecord<S>`, `SpanSignal`, `MetricRecord`, `LogEvent`
- health report contracts
- shared emitter, subscriber, filter, and projector traits

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
- logging health

Runtime role:

- validate and redact `LogEvent`
- fan out to local sinks
- record sink-local health and drop behavior

Must not own:

- observation routing
- subscriber registries
- OTLP transport
- OpenTelemetry dependencies

This crate must remain usable on its own by a basic CLI.

### 3.3 `sc-observe`

This crate is the observation runtime layered on top of logging.

Owns:

- `Observability`
- `ObservabilityBuilder`
- `ObservabilityConfig`
- subscriber registration
- projector registration
- observation routing and fan-out
- top-level routing health

Runtime role:

- accept `Observation<T>`
- route to typed subscribers
- project to `LogEvent`, `SpanSignal`, and `MetricRecord`
- send logs into the logging layer
- expose generic downstream extension points for higher-layer integrations

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
- exporter health

Runtime role:

- consume lower-layer projected logs, spans, and metrics
- assemble span lifecycle signals into completed exportable spans
- invoke actual OpenTelemetry/OTLP services and transports

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
| `sc-observability-types` | shared support crates only | `sc-observability`, `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | shared contracts, identifiers, diagnostics, traits, health types |
| `sc-observability` | `sc-observability-types` | `sc-observe`, `sc-observability-otlp`, `agent-team-mail-*` | lightweight logging, sinks, redaction, rotation, logging health |
| `sc-observe` | `sc-observability-types`, `sc-observability` | `sc-observability-otlp`, `agent-team-mail-*` | observation routing, subscribers, projectors, top-level health |
| `sc-observability-otlp` | `sc-observability-types`, `sc-observability`, `sc-observe` | `agent-team-mail-*` | OTel/OTLP transport, telemetry services, exporters, exporter health |

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

## 8. API-Design Consistency

In this docs-v2 branch, `api-design.md` is updated to match the corrected
layering:

- `sc-observe` depends on `sc-observability-types` and `sc-observability` only
- `ObservabilityConfig` no longer owns OTLP configuration
- `TelemetryConfig` is application-constructed and passed directly to
  `sc-observability-otlp`
- OTLP attachment is expressed through projector registration with
  `ObservabilityBuilder`

## 9. Pre-Implementation Cleanup

- remove any requirement or architecture text that places OTLP concerns in `sc-observability`
- remove any requirement or architecture text that requires `sc-observe -> sc-observability-otlp`
- make OTLP integration attach from the top of the stack rather than being constructed inside `sc-observe`
