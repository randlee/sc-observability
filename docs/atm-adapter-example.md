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
4. OTLP attaches from `sc-observability-otlp` by registering projectors with
   `ObservabilityBuilder`

## What The Example Must Prove

- the shared repo boundaries are sufficient for ATM integration
- OTLP attachment uses projector registration, not a special internal OTLP hook
- the shared repo remains free of `agent-team-mail-*` dependencies

## Follow-On Ownership

Once the shared boundaries are accepted, the real adapter implementation should
be built in an ATM-owned repository or module, not in this repo.
