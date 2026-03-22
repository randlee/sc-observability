# SC-Observability Architecture

## Overview

This repo is a layered standalone observability stack:

```text
sc-observability-types <- sc-observability <- consumers
sc-observability-types <- sc-observability-otlp <- consumers
```

The types crate is the stable foundation. Higher layers may depend on it; it
must not depend on higher layers or on ATM.

## Crate Roles

### `sc-observability-types`

Owns:
- trace, metric, and OTLP-neutral record types
- health/config data structures
- prefix-parameterized environment config loading

Must not own:
- ATM-specific path helpers
- spool path defaults
- daemon/socket contracts
- transport implementation details

### `sc-observability`

Owns:
- generic observability facade APIs
- non-transport runtime behavior that is still generic
- optional local/file-oriented helpers when they are not ATM-specific

Must not own:
- ATM log fan-in semantics
- ATM-specific error-code contracts
- ATM runtime-home assumptions

### `sc-observability-otlp`

Owns:
- OTLP request shaping
- OTLP transport/export logic
- protocol-specific adapters

Must not own:
- ATM config discovery
- ATM daemon interactions

## Architectural Constraints

1. No `agent-team-mail-*` dependency is allowed anywhere in this repo.
2. All ATM-specific integration must be expressed at the consumer edge.
3. Shared types must flow outward from `sc-observability-types`.
4. If a type or helper is ATM-specific, it does not belong in this repo.

## Integration Boundary

ATM may consume these crates as external dependencies.

If ATM needs:
- spool-path conventions
- daemon-specific log fan-in
- ATM socket error code contracts
- ATM-prefixed compatibility helpers

those belong in ATM-owned adapter code, not in the generic crates here.

## Migration Rule

When moving code from ATM into this repo, move only code that satisfies the
constraints above. If the code still carries ATM assumptions, split the ATM
adapter first and move only the neutral portion.
