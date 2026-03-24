# ATM Adapter Example

This unpublished crate demonstrates the intended ATM adapter pattern on top of:

- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

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
