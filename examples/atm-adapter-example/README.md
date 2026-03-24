# ATM Adapter Example

This unpublished crate demonstrates the intended ATM adapter pattern on top of:

- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

This is a boundary-correct starter pattern, not a near-production ATM adapter.
ATM still needs to supply the real production adapter layer for env/config
translation, health projection, and durability/fan-in behavior described in
`docs/atm-adapter-mapping-spec.md`.

## What It Shows

- ATM-shaped payload types defined locally in the example
- projector-based OTLP attachment through `ObservabilityBuilder`
- `ATM_OTEL_*` environment translation into `TelemetryConfig`
- health projection from shared health reports into an ATM-shaped snapshot
- normal shutdown and fail-open shutdown paths

## Run

Normal path:

```bash
cargo run --manifest-path examples/atm-adapter-example/Cargo.toml
```

Fail-open shutdown path:

```bash
cargo run --manifest-path examples/atm-adapter-example/Cargo.toml -- fail-open
```

The `-- fail-open` mode intentionally leaves one span incomplete by design so
shutdown drops it and records the loss per `OTLP-009`.

## ATM_OTEL_* Environment Variables

- `ATM_OTEL_ENDPOINT`
- `ATM_OTEL_PROTOCOL`
  values: `http-binary`, `http-json`, `grpc`
- `ATM_OTEL_AUTH_HEADER`
- `ATM_OTEL_CA_FILE`
- `ATM_OTEL_INSECURE_SKIP_VERIFY`
  values: `true/false`, `1/0`, `yes/no`
- `ATM_OTEL_DEBUG_LOCAL_EXPORT`
  values: `true/false`, `1/0`, `yes/no`

If `ATM_OTEL_ENDPOINT` is unset, telemetry stays disabled and the example still
runs end to end.

## ATM Team Adoption

- Replace `AgentContext`, `HookEventKind`, and `AgentInfoEvent` with ATM-owned
  payload types and keep them local to the ATM adapter crate.
- Minimum Cargo.toml dependencies:
  `sc-observability-types`, `sc-observability`, `sc-observe`,
  `sc-observability-otlp`, plus `serde`/`serde_json` if ATM-owned payloads need
  them.
- Wire projectors through `ObservabilityBuilder` by registering ATM-owned
  `LogProjector`, `SpanProjector`, and `MetricProjector` implementations, then
  attach OTLP by wrapping those projectors with the local telemetry-aware
  adapter pattern shown in `main.rs`.
- Keep `ATM_OTEL_*` translation in the ATM adapter layer. This example’s
  `telemetry_config_from_env()` function is the copyable pattern for mapping
  environment input into `TelemetryConfig`.
