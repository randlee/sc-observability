# OTLP smoke test

This is the current, repository-local smoke path (H-008). It supersedes only the
historical plan's external operational instructions.

`tests/telemetry-e2e` is a pytest suite (not a Cargo package). It starts the
pinned official OpenTelemetry Collector recorded in
`tests/telemetry-e2e/collector-release.json` with an OTLP HTTP receiver and a
file exporter, then reads the exported logs, spans and metrics back:

```sh
TELEMETRY_E2E_COLLECTOR_BINARY=/path/to/otelcol-contrib \
  python3 -m pytest -q tests/telemetry-e2e
```

It covers synchronous and Tokio export through `examples/otlp-native`,
file-only, OTel-only and both logging modes with sc macro and tracing inputs,
and equivalent operations from the installed `sc-otel` CLI and Python wheel.
It does not configure or query any external observability service. Without the
Collector binary the suite skips locally and fails under CI.
