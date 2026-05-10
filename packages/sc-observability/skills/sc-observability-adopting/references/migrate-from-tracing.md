# Migrate From `tracing`

Use this reference when the existing project already uses `tracing` and
`tracing-subscriber`.

## Inventory

- existing subscriber stack
- existing file or console sinks
- whether traces and logs are already exported remotely
- whether structured fields already exist and how stable they are

## Migration Strategy

- decide whether the project needs only logging or also needs `sc-observe` and
  `sc-observability-otlp`
- preserve lifecycle and error coverage first
- map tracing metadata into stable `target` and `action` names
- avoid trying to replace logging, routing, and OTLP in one uncontrolled step

## Watch Items

- tracing setups often mix local logging and telemetry concerns
- operator expectations may depend on subscriber-specific formatting
- if OTLP already exists, confirm whether the migration should stop at
  `sc-observability` first or move straight to the full stack
