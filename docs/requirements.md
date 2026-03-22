# SC-Observability Requirements

## Purpose

`sc-observability` is a standalone observability toolkit for structured logging,
traces, metrics, and OTLP export.

This repo contains:
- `sc-observability-types`
- `sc-observability`
- `sc-observability-otlp`

It must be reusable outside ATM.

## Product Requirements

1. No crate in this repo may depend on any `agent-team-mail-*` crate.
2. Shared record/config/type definitions must live in
   `sc-observability-types`.
3. Generic observability logic must live in `sc-observability`.
4. OTLP transport/export logic must live in `sc-observability-otlp`.
5. Environment-based configuration must support configurable prefixes rather
   than hard-coding ATM-specific names into the generic API.
6. The repo must not encode ATM daemon/socket/spool assumptions into the shared
   crates.
7. ATM-specific logging or fan-in behavior, if still needed, must live in ATM
   or in a dedicated ATM adapter crate outside the generic core.

## Boundary Rules

1. `sc-observability-types` may contain only neutral data/config/types.
2. `sc-observability` may depend on `sc-observability-types`, but not on ATM.
3. `sc-observability-otlp` may depend on `sc-observability-types` and generic
   transport libraries, but not on ATM.
4. Generic crates must not expose ATM-specific constants, spool helpers, socket
   error contracts, or runtime-home conventions.
5. Backward-compatibility shims for ATM belong in ATM, not in this repo.

## Configuration Rules

1. Generic APIs must support prefix-parameterized environment loading.
2. ATM-prefixed env support may exist only as a thin compatibility wrapper or in
   ATM-owned integration code.
3. Local mirror/file export paths must be explicit inputs, not derived from ATM
   home helpers.

## Non-Goals

This repo does not own:
- ATM daemon log fan-in
- ATM spool layout
- ATM runtime-home discovery
- ATM mailbox/event schemas
- ATM plugin contracts
