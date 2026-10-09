# sc-observability

A general-purpose Rust logging library: any new project gets structured logging
and OpenTelemetry (OTLP) export immediately. Consistent, clean, easy to use, and
high-performance.

## Workspace Crates

| Crate | Purpose |
| --- | --- |
| [`sc-observability-types`](./crates/sc-observability-types/) | Shared contracts: identifiers, timestamps, trace context, logs, profiles, diagnostics, health reports, query/follow values, and error surfaces. |
| [`sc-observability`](./crates/sc-observability/) | Logging-only runtime: `Logger`, built-in file/console sinks, custom sink registration, redaction, health, query, and follow. |
| [`sc-observe`](./crates/sc-observe/) | Typed routing of observations to log projectors and subscribers, including combined health. |
| [`sc-observability-otlp`](./crates/sc-observability-otlp/) | Native OpenTelemetry: `log-sink` (`OtelLogSink`) and `synchronous-client` (`sync::Client`). |
| [`sc-observability-log`](./crates/sc-observability-log/) | Additive bridge/logging API and typed error surface. |
| [`sc-observability-log-macros`](./crates/sc-observability-log-macros/) | Procedural macros used by the bridge/logging API. |
| [`sc-observability-dto`](./crates/sc-observability-dto/) | Language-neutral wire DTOs and checked conversions. |
| [`sc-observability-binding-runtime`](./crates/sc-observability-binding-runtime/) | Shared native binding runtime and bounded coordination. |

## Which Crate Do I Need?

| If you need... | Start with... |
| --- | --- |
| Logging only | `sc-observability` |
| Query/follow on JSONL logs | `sc-observability` |
| Routing one observation to logs and subscribers | `sc-observe` + `sc-observability` + `sc-observability-types` |
| OTLP export | `sc-observability-otlp` + lower layers |
| Shared value types only | `sc-observability-types` |

The three consumer entry points are:

- `sc-observability` for structured logging;
- `sc-observability-otlp` for OpenTelemetry: the `log-sink` feature maps core
  logs to the official SDK, and `synchronous-client` exports over OTLP/HTTP
  without a Tokio runtime; and
- `sc-observe` for typed routing of observations and their combined health.

The PyO3 extension is a root-workspace crate and is published as a Rust
support artifact; its Python wheel/sdist, the standalone Tauri host, and the
generated TypeScript client are separate binding artifacts. Their intended
publish channels and deliberate post-Phase-B authorization are recorded in
[`release/bindings-artifacts.toml`](./release/bindings-artifacts.toml).

## 1.5.0 API and binding updates

The 1.5.0 candidate keeps released 1.x APIs behind each library crate's
default-on `v1` feature and deprecates them; canonical APIs are under `v2`.
Build with `default-features = false` to check that a consumer uses no `v1`
surface. See the [Phase F migration guide](./docs/migration/phase-f.md).

Tokio hosts use the official `opentelemetry_sdk` and `opentelemetry-otlp`
crates directly; the OTLP crate adds the `OtelLogSink` bridge and a blocking
`sync::Client` used by the `sc-otel` CLI and Python `Telemetry`.
Python bindings release the GIL around flush, shutdown, and health;
the six-platform wheel matrix includes Windows ARM64 (`win_arm64`). See the
[Python platform policy](./release/python-platform-policy.json).

## Minimal Logging-Only Snippet

```rust
use std::path::PathBuf;

use sc_observability::{
    ActionName, Level, LogEvent, LoggerConfig, OutcomeLabel, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp, OBSERVATION_ENVELOPE_VERSION,
};
use sc_observability::v2::Logger;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ServiceName::new("example-service")?;
    let logger = Logger::new(LoggerConfig::default_for(
        service.clone(),
        PathBuf::from("./observability"),
    ))?;

    logger.log(LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)?,
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service,
        target: TargetCategory::new("example.app")?,
        action: ActionName::new("startup")?,
        message: Some("service booted".to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some(OutcomeLabel::new("ok")?),
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    })?;

    logger.flush()?;

    let health = logger.health();
    println!("active log path: {}", health.active_log_path.display());
    println!("writer state: {:?}", health.writer_state);
    println!(
        "queue depth: {} / {}",
        health.queue_depth, health.queue_capacity
    );
    Ok(())
}
```

Default output goes to `<log_root>/logs/<service>.log.jsonl`.

Use `log()` for the default blocking queue-admission path, `try_log()` for the
non-blocking best-effort path, and `flush()` when the caller needs a durability
barrier after successful queue admission. `emit()` remains available only as a
deprecated compatibility path.

`LoggerConfig::queue_capacity` is the logger's bounded buffering
configuration. Leave it at `DEFAULT_LOG_QUEUE_CAPACITY` unless the deployment
has measured a reason to choose another admission bound; do not copy that
capacity into another layer.

## Fault Injection For Retained Sinks

The `fault-injection` feature exposes a `RetainedSinkFaultInjector` for live
validation. It wraps one retained sink and forces that sink to report
`SinkHealthState::DegradedDropping` or `SinkHealthState::Unavailable` through
the normal `LoggingHealthReport` path without filesystem sabotage.

Enable it only for validation runs:

```bash
cargo test --features fault-injection
```

Never enable `fault-injection` in production builds.

## Start Here

- Consumer onboarding: [CONSUMING.md](./CONSUMING.md)
- Public architecture: [docs/architecture.md](./docs/architecture.md)
- Requirements and contract decisions: [docs/requirements.md](./docs/requirements.md)
- Custom sink example: [`examples/custom-sink-example/`](./examples/custom-sink-example/)
- Phase F consumer migration: [docs/migration/phase-f.md](./docs/migration/phase-f.md)

## Release / Publishing

- Publish procedure: [PUBLISHING.md](./PUBLISHING.md)

### CLI reference

The [sc-otel manual](docs/manual/sc-otel/cli-reference.md) is generated from Clap and ships with CLI release archives under `share/doc/sc-otel/`. The same reference is [published on the website](https://randlee.github.io/sc-observability/manual/sc-otel/). Run `sc-otel --help` or `sc-otel <command> --help` for the built-in reference.
