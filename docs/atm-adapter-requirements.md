# ATM Adapter Requirements

**Status**: Draft for review
**Applies to**: ATM-owned adapter layer only
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`architecture.md`](./architecture.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)
- [`extraction-inventory.md`](./extraction-inventory.md)

## 1. Purpose

This document defines the normative requirements for the ATM-owned adapter
boundary referred to in the shared workspace as `atm-observability-adapter`.

The shared `sc-observability` workspace does not own ATM compatibility
behavior. This adapter layer owns the ATM-specific transforms, compatibility
rules, and projected surfaces needed to preserve ATM observability behavior
while using the standalone shared crates underneath.

## 2. Ownership Boundary

- ADP-001 The ATM adapter shall own `LogEventV1` and all ATM-specific payload,
  envelope, and compatibility mapping behavior.
- ADP-002 The ATM adapter shall own the transform from ATM-owned event fields
  into ATM-owned event/log envelopes, including promotion rules and passthrough
  rules for extra fields.
- ADP-003 The ATM adapter shall own all ATM-prefixed env/config translation for
  logging and OTEL setup.
- ADP-004 The ATM adapter shall own ATM-specific daemon fan-in, direct-spool
  fallback, and shutdown-safe durability behavior.
- ADP-005 The ATM adapter shall own ATM health JSON projection from shared
  health models.
- ADP-006 The ATM adapter shall preserve parity across `atm status`,
  `atm doctor`, and `atm daemon status` health surfaces until an explicit
  breaking change is approved.
- ADP-007 The ATM adapter shall preserve compatibility for currently shipped ATM
  observability schemas until an explicit migration or breaking change is
  approved.

## 3. Mapping Semantics

- ADP-008 The ATM adapter shall define the full `EventFields -> LogEventV1`
  mapping contract as a source-of-truth transform owned by ATM.
- ADP-009 The mapping contract shall define generated trace and span identifier
  behavior when upstream ATM payloads omit those identifiers.
- ADP-010 The mapping contract shall define propagation rules for ATM runtime,
  session, team, and agent metadata.
- ADP-011 The mapping contract shall define which ATM fields are promoted to
  first-class envelope fields and which remain passthrough extra fields.
- ADP-012 The mapping contract shall define ATM message preview behavior and any
  feature/env flag that controls preview emission.
- ADP-013 The mapping contract shall define redaction-sensitive exclusions,
  including any sensitive text that must not be written to persistent logs.

## 4. Durability And Shutdown

- ADP-014 The ATM adapter shall define when normal producer-path fan-in is
  acceptable and when synchronous direct-spool fallback is required.
- ADP-015 The ATM adapter shall define shutdown and crash-adjacent durability
  expectations for ATM-owned emission paths.
- ADP-016 The ATM adapter shall define replay and merge ownership for any
  persisted spool path it continues to support.

## 5. Health Projection

- ADP-017 The ATM adapter shall define the field-level mapping from shared
  health models to ATM health JSON outputs.
- ADP-018 The health projection contract shall define compatibility behavior for
  ATM-specific fields such as collector state, local mirror state, spool path,
  dropped counters, and last-error projection where ATM surfaces them.
- ADP-019 The ATM adapter shall treat shared health objects as the generic input
  model and ATM JSON as an ATM-owned projection, not a shared-repo contract.

## 6. Env And Launch Translation

- ADP-020 The ATM adapter shall define precedence rules for ATM-prefixed OTEL
  env/config translation.
- ADP-021 The ATM adapter shall define validation and redaction expectations for
  OTEL auth, TLS, and endpoint-related ATM config.
- ADP-022 The ATM adapter shall define launch inheritance rules for OTEL/logging
  configuration passed to subprocesses or child runtimes where ATM relies on
  inheritance.

## 7. Proving Obligations

- ADP-023 ATM migration sufficiency shall not be claimed from the shared repo
  docs alone.
- ADP-024 ATM migration confidence shall require an ATM-owned proving plan or
  implementation that exercises the adapter contract against the shared crates.
- ADP-025 The unpublished proving artifact in this repo may be used as boundary
  evidence only; it shall not be used as the sole evidence that ATM migration is
  fully specified.
