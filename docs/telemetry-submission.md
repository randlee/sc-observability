# Telemetry submission

**Superseded by H-003/H-005/H-006 (Phase H).** The durable local store,
submission envelope, admission receipts, `flush`/`status` commands and
`telemetry.yaml` store configuration described here in earlier releases are
removed. Existing store files are not deleted automatically.

The `sc-otel` CLI and the Python `sc_observability.telemetry.Telemetry` class
send one log, completed span or metric through `sync::Client` in
`sc-observability-otlp`, which exports with the official blocking OTLP/HTTP
protobuf exporter and returns the exporter's outcome. Nothing is stored or
retried after the call returns. Python returns `Ok` or `TelemetryErr` tagged
results (ADR-014).

See the [Clap-generated CLI manual](manual/sc-otel/cli-reference.md) for
commands, options and exit codes, and
[ADR-023](architecture.md#adr-023-native-opentelemetry-and-thin-synchronous-frontends)
for the design.
