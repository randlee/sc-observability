# Migration Guide

## Purpose

This guide covers migration for existing ATM logging and observability
consumers that currently use the in-workspace shared crates from the
`agent-team-mail` repository.

## Target Workspace

The standalone workspace now owns these crates:

- `sc-observability-types`
- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

## Shared-Crate Migration

1. Replace ATM workspace path dependencies with dependencies sourced from this
   repo or crates.io once the standalone release is published.
2. Keep logging-only consumers on `sc-observability` plus
   `sc-observability-types`.
3. Add `sc-observe` only for typed observation routing and projector-based
   fan-out.
4. Add `sc-observability-otlp` only when OTLP export is required.

## ATM Adapter Boundary

ATM-specific integration stays outside the shared crates.

ATM-owned code should continue to own:

- ATM-shaped payload types
- ATM environment-variable translation
- ATM-specific projector logic
- ATM health projection and compatibility formatting
- any daemon/spool/fan-in behavior

Use these repo artifacts as the implementation baseline:

- [`docs/atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`docs/atm-adapter-architecture.md`](./atm-adapter-architecture.md)
- [`docs/atm-adapter-mapping-spec.md`](./atm-adapter-mapping-spec.md)
- [`docs/atm-adapter-example.md`](./atm-adapter-example.md)
- `examples/atm-adapter-example/`

## Logging-Only Consumers

For consumers that only need structured logging:

1. Depend on `sc-observability-types` and `sc-observability`.
2. Construct `LoggerConfig::default_for(service_name, log_root)`.
3. Register additional sinks only if required.
4. Do not pull in `sc-observe` or `sc-observability-otlp`.

## Observation Routing Consumers

For consumers that emit typed observations:

1. Define the domain payload type outside the shared crates.
2. Construct `ObservabilityConfig`.
3. Register subscribers and projectors through `Observability::builder(...)`.
4. Keep projector registration construction-time only.

## OTLP Consumers

For consumers that export to OTLP:

1. Construct `TelemetryConfig` directly in the adapter/application layer.
2. Keep OTLP env/config parsing outside the shared crates.
3. Attach OTLP by wrapping projector implementations locally, following the
   pattern used by `examples/atm-adapter-example`.

## Breaking API Renames

The production-readiness review approved these source-breaking API updates:

1. The sealed telemetry-health provider trait now uses the
   `ObservabilityHealthProvider` name.
2. `ObservabilityBuilder` now exposes
   `with_observability_health_provider(...)`.
3. `ObservationSubscriber<T>` implementations now provide
   `observe(...)`.

## ATM Adoption Sequence

1. Move shared crate usage in ATM to the published standalone crates.
2. Keep ATM-specific adapter code in ATM-owned code.
3. Copy the example adapter pattern and replace the sample ATM-shaped structs
   with ATM-owned structs.
4. Verify health projection and fail-open behavior against ATM requirements.
5. Remove obsolete ATM-local copies of the shared crate implementations after
   parity is confirmed.
