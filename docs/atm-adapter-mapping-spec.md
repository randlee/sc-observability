# ATM Adapter Mapping Spec

**Status**: Draft for review
**Applies to**: ATM-owned adapter implementation and proving work
**Related documents**:
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)
- [`atm-adapter-example.md`](./atm-adapter-example.md)
- [`atm-quickstart.md`](./atm-quickstart.md)

## 1. Purpose

This document captures the ATM-specific mapping work that must be clear before
ATM implementation starts. It is intentionally outside the shared workspace
API docs because it governs ATM-owned semantics rather than generic shared
behavior.

## 2. Mapping Layers

ATM integration has three mapping layers:

1. ATM payloads -> `Observation<T>`
2. ATM payloads / observations -> `LogEvent`, `SpanSignal`, `MetricRecord`
3. shared health and durability state -> ATM CLI/daemon-facing outputs

The ATM adapter owns all three layers.

## 3. ATM Payload To Observation Mapping

The ATM adapter shall define ATM-owned observation payloads for at least:

- agent lifecycle events
- subagent lifecycle events
- tool use events
- task lifecycle events
- daemon lifecycle events
- error and recovery events

Required mapping rules:

- `Observation.service` comes from ATM service/tool identity, not ad hoc call
  sites
- `Observation.identity` uses shared process identity, optionally enriched by
  ATM-owned process-resolution rules
- `Observation.trace` is populated when ATM has a valid trace/span context
- ATM-specific fields such as team, agent, subagent, session, and runtime stay
  inside the ATM payload type or ATM-owned projection attributes

## 4. Projection Mapping Rules

### 4.1 Log Projection

The ATM adapter shall define:

- which ATM fields are promoted into `LogEvent.target`, `LogEvent.action`,
  `LogEvent.outcome`, `LogEvent.diagnostic`, and `LogEvent.state_transition`
- which ATM fields remain in `LogEvent.fields`
- whether ATM preview fields are omitted, truncated, or redacted
- which ATM-sensitive values must never be persisted

### 4.2 Span Projection

The ATM adapter shall define:

- which ATM events create `SpanSignal::Started`
- which ATM events create `SpanSignal::Event`
- which ATM events create `SpanSignal::Ended`
- how parent/child span relationships are assigned
- how ATM-generated trace/span IDs are handled when upstream IDs are absent

### 4.3 Metric Projection

The ATM adapter shall define:

- which ATM actions become counters, gauges, or histograms
- naming conventions for ATM metrics
- which ATM fields become metric attributes
- which tool-use and timing data become histograms

## 5. Health Projection Rules

The ATM adapter shall define a field-by-field mapping from:

- `LoggingHealthReport`
- `ObservabilityHealthReport`
- `TelemetryHealthReport`

into ATM-facing health outputs for:

- `atm status`
- `atm doctor`
- `atm daemon status`

This spec must call out:

- dropped counters
- last-error projection
- collector/export state
- spool/fan-in state if ATM continues to ship it

## 6. Env And Config Translation Rules

The ATM adapter shall own translation for at least:

- `ATM_OTEL_ENDPOINT`
- `ATM_OTEL_PROTOCOL`
- `ATM_OTEL_AUTH_HEADER`
- `ATM_OTEL_CA_FILE`
- `ATM_OTEL_INSECURE_SKIP_VERIFY`
- `ATM_OTEL_DEBUG_LOCAL_EXPORT`

The ATM adapter shall also define:

- precedence between ATM env, config file, and CLI overrides
- redaction rules for auth/TLS material
- which values propagate to subprocesses

## 7. Durability And Shutdown Rules

The ATM adapter shall explicitly define:

- when normal shared fail-open logging is acceptable
- when ATM requires synchronous direct-spool fallback
- who owns replay and merge
- what happens on pre-exit and crash-adjacent paths

The shared workspace does not define those semantics.

## 8. Required ATM Proving Cases

Before ATM implementation is declared design-complete, the ATM adapter plan
shall prove:

- one ATM observation can reach logging, routing, and OTLP through the shared
  extension points
- ATM-owned projection rules preserve required fields and redaction behavior
- ATM health projection can be generated from shared health models
- ATM shutdown behavior is defined for both normal and fallback paths

## 9. Open ATM-Owned Decisions

The following remain ATM-owned and must be decided in ATM planning or code
review, not in shared-crate implementation:

- exact `LogEventV1` compatibility envelope
- exact preview truncation policy
- exact generated trace/span ID policy when upstream IDs are absent
- whether GH observability ledger integration remains in ATM or moves later
