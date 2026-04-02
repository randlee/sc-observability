# ATM Adapter Example

## Purpose

This document describes the proving-artifact pattern for ATM integration without
placing ATM production code in the shared `sc-observability` workspace.

The goal is to verify that:

- ATM-shaped event types can remain outside the shared crates
- ATM can attach logging, routing, and OTLP behavior using shared extension
  points only
- no `agent-team-mail-*` dependency is required in this repo to prove the
  integration path
- the shared repo can provide boundary evidence without pretending to be the
  full ATM migration specification

For the minimal ATM production configuration and shared out-of-the-box
defaults, see [`atm-quickstart.md`](./atm-quickstart.md).

## Boundary

This repo does **not** own the real ATM adapter implementation.

This repo may own:

- documentation of the adapter boundary
- an unpublished ATM-shaped proving crate
- tests/examples that validate the shared interfaces are sufficient

This repo must not own:

- `LogEventV1` production definitions
- daemon fan-in or spool compatibility logic
- ATM env parsing
- ATM health snapshot production contracts
- ATM-specific projector behavior used in production

Those belong in an ATM-owned adapter crate or module, referred to in the
architecture as `atm-observability-adapter`.

## Proving Artifact

The proving artifact for this repo is the unpublished crate:

- `examples/atm-adapter-example`

It demonstrates the intended integration pattern:

1. ATM-shaped payload types are defined locally in the example crate
2. logging uses the lower-level `sc-observability` crate
3. routing uses `sc-observe`
4. OTLP attaches from `sc-observability-otlp` through the shipped
   `TelemetryProjectors<T>` registration path on `ObservabilityBuilder`
5. top-level routing health includes the attached telemetry health snapshot via
   `ObservabilityBuilder::with_telemetry_health_provider(...)`

## What The Example Must Prove

- the shared repo boundaries are sufficient for ATM integration
- OTLP attachment uses the shipped `TelemetryProjectors<T>` registration path,
  not a special internal OTLP hook
- the shared repo remains free of `agent-team-mail-*` dependencies

## What The Example Does Not Prove

This example is intentionally boundary-focused and is not sufficient evidence
that ATM migration is fully specified.

It does not prove:

- complete `EventFields -> LogEventV1` compatibility semantics
- ATM direct-spool or daemon fan-in durability behavior
- ATM health JSON parity
- ATM-prefixed env/config translation and launch inheritance behavior
- full ATM migration readiness without the ATM-owned adapter docs and code

## Follow-On Ownership

Once the shared boundaries are accepted, the real adapter implementation should
be built in an ATM-owned repository or module, not in this repo.
