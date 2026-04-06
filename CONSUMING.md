# Consuming sc-observability

This document is the consumer-facing starting point for logging-only adoption.

## 1. Logging-Only Setup

Add the logging crate:

```toml
[dependencies]
sc-observability = "1"
serde_json = "1"
```

`sc-observability` re-exports the shared contract types from
`sc-observability-types`, so consumers can import the common surface directly
from `sc_observability`. That re-export set includes:

- event and value contracts such as `LogEvent`, `Level`, `ErrorCode`,
  `ServiceName`, `TargetCategory`, `ActionName`, `Timestamp`, and
  `ProcessIdentity`
- runtime error and health types such as `EventError`, `LoggingHealthReport`,
  `LoggingHealthState`, `SinkHealth`, and `SinkHealthState`
- historical access helpers such as `LogQuery`, `LogSnapshot`,
  `LogFollowSession`, and `JsonlLogReader`

Consumers only need to depend on `sc-observability` for that surface. Add
`sc-observability-types` as a direct dependency only if you need the types
crate independently, such as when implementing custom sinks or extending the
shared types layer directly.

Create a logger with the documented defaults:

```rust
use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig, ServiceName};

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

Consumers register custom sinks through `LoggerBuilder` and
`SinkRegistration`:

```rust
use std::sync::Arc;

use sc_observability::{LogSink, LoggerBuilder, LoggerConfig, ServiceName, SinkRegistration};

fn register_sink(builder: &mut LoggerBuilder, sink: Arc<dyn LogSink>) {
    builder.register_sink(SinkRegistration::new(sink));
}
# let service = ServiceName::new("consumer-app")?;
# let config = LoggerConfig::default_for(service, std::env::temp_dir());
# let mut builder = LoggerBuilder::new(config)?;
# register_sink(&mut builder, Arc::new(sc_observability::ConsoleSink::stderr()));
# let _logger = builder.build();
# Ok::<(), Box<dyn std::error::Error>>(())
```

Optional sink-local filtering stays on the registration:

```rust
use std::sync::Arc;

use sc_observability::{LogFilter, LoggerBuilder, LoggerConfig, ServiceName, SinkRegistration};

fn register_filtered(
    builder: &mut LoggerBuilder,
    sink: Arc<dyn sc_observability::LogSink>,
    filter: Arc<dyn LogFilter>,
) {
    builder.register_sink(SinkRegistration::new(sink).with_filter(filter));
}
# let service = ServiceName::new("consumer-app")?;
# let config = LoggerConfig::default_for(service, std::env::temp_dir());
# let mut builder = LoggerBuilder::new(config)?;
# struct AcceptAll;
# impl LogFilter for AcceptAll { fn accepts(&self, _event: &sc_observability::LogEvent) -> bool { true } }
# register_filtered(
#     &mut builder,
#     Arc::new(sc_observability::ConsoleSink::stderr()),
#     Arc::new(AcceptAll),
# );
# let _logger = builder.build();
# Ok::<(), Box<dyn std::error::Error>>(())
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

## 8. Fault Injection For Retained Sinks

The optional `fault-injection` feature adds `RetainedSinkFaultInjector`, which
forces one retained sink to report `SinkHealthState::DegradedDropping` or
`SinkHealthState::Unavailable` through the same `LoggingHealthReport`
transitions consumers see during normal operation.

Enable it only for validation runs:

```bash
cargo test --features fault-injection
```

Never enable `fault-injection` in production builds.
