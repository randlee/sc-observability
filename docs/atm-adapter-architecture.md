# ATM Adapter Architecture

**Status**: Draft for review
**Applies to**: ATM-owned adapter layer only
**Related documents**:
- [`architecture.md`](./architecture.md)
- [`requirements.md`](./requirements.md)
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-example.md`](./atm-adapter-example.md)

## 1. Purpose

This document describes the architecture of the ATM-owned adapter boundary
referred to as `atm-observability-adapter`.

Its purpose is to define the ATM-side integration that sits above the shared
`sc-observability` workspace without pushing ATM semantics back into the shared
crates.

## 1.1 Layered Diagram

```text
ATM producers / daemon paths
          |
          v
atm-observability-adapter
  - ATM payload mapping
  - ATM env/config translation
  - ATM health projection
  - ATM spool / fan-in ownership
          |
          v
sc-observability-otlp
  - OTLP attachment
  - exporter runtime
          |
          v
sc-observe
  - observation routing
  - subscriber / projector registration
          |
          v
sc-observability
  - logging
  - sinks
          |
          v
sc-observability-types
  - shared contracts
```

## 2. Ownership Split

### Shared Workspace Owns

- neutral contracts and diagnostics
- lightweight logging infrastructure
- generic observation routing
- OTLP attachment and exporter infrastructure

### ATM Adapter Owns

- ATM payload and envelope shapes such as `LogEventV1`
- ATM event-to-envelope transforms
- ATM env/config translation
- ATM daemon fan-in and direct-spool compatibility behavior
- ATM health JSON projection and schema parity
- ATM-specific projector and subscriber behavior

## 3. Producer Path

ATM producers emit ATM-owned payloads into the ATM adapter layer.

The ATM adapter:

1. validates ATM-owned fields and env/config inputs
2. applies ATM-owned field promotion, passthrough, and generated-ID rules
3. emits shared `Observation<T>` and related lower-level shared structures into
   the standalone workspace

Important boundary:

- ATM producers do not emit directly into shared ATM-specific contracts because
  those contracts do not exist in the shared repo
- the adapter is the single source of truth for ATM-owned mapping semantics

## 4. Routing And Logging Path

The ATM adapter attaches to `sc-observe` using the generic registration points
approved in the shared architecture:

- typed ATM-owned observations remain ATM-owned
- ATM-owned mapping/projector code translates those observations into generic
  `LogEvent`, `SpanSignal`, and `MetricRecord` projections
- `sc-observe` routes those projections through the shared logging and
  subscriber infrastructure

This preserves the intended split:

- generic runtime behavior in shared crates
- ATM-specific projection behavior in the ATM adapter

## 5. OTLP Attachment Path

ATM OTLP export uses the shared top-of-stack OTLP layer:

1. ATM constructs `TelemetryConfig` through ATM-owned env/config translation
2. ATM adapter composes `sc-observability-otlp` with `sc-observe`
3. OTLP behavior attaches through projector registration with
   `ObservabilityBuilder`

The adapter owns:

- translation from ATM launch/config surfaces into generic OTLP config
- any ATM-specific defaults, validation, redaction, and inheritance rules

The shared repo owns:

- OTLP transport/runtime behavior after configuration reaches the generic layer

## 6. Direct-Spool And Fan-In Path

Any ATM-owned direct-spool or daemon fan-in path remains outside the shared
repo.

The ATM adapter architecture must define:

- when normal fail-open routing is sufficient
- when synchronous direct-spool emission is required
- who owns replay and merge semantics
- what durability is expected during shutdown or crash-adjacent paths

The shared repo does not define those semantics and must not silently imply
them.

## 7. Health Projection Path

The shared repo provides generic health reports.

The ATM adapter projects those generic health reports into ATM-shipped JSON
surfaces for:

- `atm status`
- `atm doctor`
- `atm daemon status`

The adapter architecture must define:

- field-level mappings
- compatibility and parity expectations
- projection ownership for ATM-only fields not present in generic health models

## 8. Boot Sequencing

Boot sequencing follows the shared architecture:

1. generic observability initializes first
2. ATM adapter registration and augmentation happen after generic startup is
   available
3. early lifecycle events may be recorded before ATM-specific context is fully
   available

The ATM adapter owns the rules for enriching early events once ATM-specific
context becomes available.

## 9. Shutdown And Flush Sequencing

The adapter architecture must define:

- when shared logger/observability/telemetry flush is sufficient
- when ATM-specific synchronous durability paths must run before process exit
- how ATM daemon-managed fan-in paths are shut down safely
- what parity guarantees are expected across normal exit vs pre-exit fallback

## 10. Proving Scope

The proving artifact in this repo is intentionally limited.

It may prove:

- ATM-shaped payloads can stay outside the shared repo
- shared extension points are sufficient for ATM-owned mapping and projector
  code
- OTLP can attach from the top layer without ATM dependencies in shared crates

It does not prove:

- complete ATM compatibility behavior
- daemon fan-in and direct-spool durability semantics
- full ATM health JSON parity
- complete ATM env/config and launch inheritance behavior

Those remain ATM-adapter architecture obligations outside the shared repo.
