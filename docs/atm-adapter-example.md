# ATM Adapter Example

> **Retired in Phase F.** This document preserves historical context for the
> former ATM proving artifact. It is not a shared-repository contract artifact,
> does not establish interface sufficiency, and does not replace the ATM-owned
> production adapter boundary. See [architecture section 10](./architecture.md#10-atm-proving-artifact-retired-in-phase-f).

## Purpose

This document records the former proving-artifact pattern that was used to
describe ATM integration without placing ATM production code in the shared
`sc-observability` workspace.

The artifact was intended to show that:

- ATM-shaped event types could remain outside the shared crates
- ATM could attach logging, routing, and OTLP behavior using shared extension
  points only
- no `agent-team-mail-*` dependency was required in this repo to demonstrate the
  integration path
- the shared repo could provide boundary evidence without pretending to be the
  full ATM migration specification

Its former references to ATM production configuration and shared defaults are
historical; the retirement notice in [architecture section 10](./architecture.md#10-atm-proving-artifact-retired-in-phase-f)
governs this document.

## Boundary

The former pattern did not make this repository the owner of the real ATM
adapter implementation.

The historical artifact described:

- documentation of the adapter boundary;
- an unpublished ATM-shaped proving crate; and
- tests/examples that illustrated the shared interfaces.

It did not make this repository the owner of:

- `LogEventV1` production definitions
- daemon fan-in or spool compatibility logic
- ATM env parsing
- ATM health snapshot production contracts
- ATM-specific projector behavior used in production

Those belong in an ATM-owned adapter crate or module, referred to in the
architecture as `atm-observability-adapter`.

## Boundary evidence

The historical artifact documented the intended integration pattern without
owning an ATM implementation:

1. ATM-shaped payload types are defined locally in the example crate
2. logging uses the lower-level `sc-observability` crate
3. routing uses `sc-observe`
4. OTLP attaches from `sc-observability-otlp` through the shipped
   `TelemetryProjectors<T>` registration path on `ObservabilityBuilder`
5. top-level routing health includes the attached telemetry health snapshot via
   `ObservabilityBuilder::with_observability_health_provider(...)`

## What The Historical Evidence Was Intended To Show

- the shared repo boundaries appeared sufficient for ATM integration
- OTLP attachment used the shipped `TelemetryProjectors<T>` registration path,
  not a special internal OTLP hook
- the shared repo remained free of `agent-team-mail-*` dependencies

## What The Historical Example Did Not Prove

This documentation was intentionally boundary-focused and was not sufficient
evidence that ATM migration was fully specified.

It did not prove:

- complete `EventFields -> LogEventV1` compatibility semantics
- ATM direct-spool or daemon fan-in durability behavior
- ATM health JSON parity
- ATM-prefixed env/config translation and launch inheritance behavior
- full ATM migration readiness without the ATM-owned adapter docs and code

## Follow-On Ownership

The real adapter implementation belongs in an ATM-owned repository or module,
not in this repository. The retired proving artifact does not provide current
integration evidence.
