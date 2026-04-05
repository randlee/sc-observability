# Consuming sc-observability

This document is the consumer-facing starting point for logging-only adoption.

## 1. Logging-Only Setup

Add the logging crate and shared types:

```toml
[dependencies]
sc-observability = "1"
sc-observability-types = "1"
serde_json = "1"
```

Create a logger with the documented defaults:

```rust
use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::ServiceName;

let service = ServiceName::new("my-service")?;
let logger = Logger::new(LoggerConfig::default_for(
    service,
    PathBuf::from("./observability"),
))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 2. Default Log Root And Path

- The built-in file sink writes to `<log_root>/logs/<service>.log.jsonl`.
- The service name is part of the filename, not a parent directory.
- `Logger::health().active_log_path` is the consumer-facing way to inspect the
  resolved active path at runtime.

## 3. `SC_LOG_ROOT` Behavior

- `LoggerConfig::default_for(service, log_root)` treats a non-empty `log_root`
  as explicit configuration.
- If `log_root` is empty and `SC_LOG_ROOT` is set, `SC_LOG_ROOT` becomes the
  effective log root.
- If both are present, explicit `log_root` wins.

## 4. Enabling And Disabling Built-In Sinks

- `LoggerConfig::default_for(...)` enables the built-in file sink by default.
- `LoggerConfig::default_for(...)` disables the built-in console sink by
  default.
- Set `config.enable_file_sink = false` to disable the built-in JSONL sink.
- Set `config.enable_console_sink = true` to enable the built-in console sink.
- The built-in console sink supports `ConsoleSink::stdout()` and
  `ConsoleSink::stderr()` as the public writer-selection surface.

## 5. Registering A Custom Sink

Consumers register custom sinks through `SinkRegistration`:

```rust
use std::sync::Arc;

use sc_observability::{LogSink, SinkRegistration};

fn register_sink(logger: &mut sc_observability::Logger, sink: Arc<dyn LogSink>) {
    logger.register_sink(SinkRegistration::new(sink));
}
```

Optional sink-local filtering stays on the registration:

```rust
use std::sync::Arc;

use sc_observability::{LogFilter, SinkRegistration};

fn register_filtered(
    logger: &mut sc_observability::Logger,
    sink: Arc<dyn sc_observability::LogSink>,
    filter: Arc<dyn LogFilter>,
) {
    logger.register_sink(SinkRegistration::new(sink).with_filter(filter));
}
```

See [`examples/custom-sink-example/`](./examples/custom-sink-example/) for a
runnable public-only example.

## 6. Using `Logger::health()`

`Logger::health()` is the consumer-facing status snapshot for:

- aggregate logging state
- active log path
- per-sink status
- query/follow availability
- last observed logging error summary

Typical usage:

```rust
let health = logger.health();
println!("state: {:?}", health.state);
println!("active log path: {}", health.active_log_path.display());
for sink in &health.sink_statuses {
    println!("sink {} => {:?}", sink.name, sink.state);
}
```

## 7. Deeper Docs

- Public architecture: [docs/architecture.md](./docs/architecture.md)
- Contract details: [docs/requirements.md](./docs/requirements.md)
- Query/follow surface: [docs/api-design.md](./docs/api-design.md)
- ATM-shaped defaults: [docs/atm-quickstart.md](./docs/atm-quickstart.md)
