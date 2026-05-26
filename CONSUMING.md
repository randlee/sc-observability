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

use sc_observability::{Logger, LoggerConfig, Running, ServiceName};

let service = ServiceName::new("my-service")?;
let logger: Logger<Running> = Logger::new(LoggerConfig::default_for(
    service,
    PathBuf::from("./observability"),
))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Customize retained-log rotation and maintenance through
`LoggerConfig.retained_log_policy`:

```rust
use std::path::PathBuf;
use std::time::Duration;

use sc_observability::{
    ByteCount, FileCount, LoggerConfig, MaintenanceCadence, MaintenanceJoinTimeout,
    RetentionMaxAge, ServiceName,
};

let mut config = LoggerConfig::default_for(
    ServiceName::new("my-service")?,
    PathBuf::from("./observability"),
);
config.retained_log_policy.rotation_max_bytes = ByteCount::from_mib(8);
config.retained_log_policy.rotation_max_files = FileCount::from_usize(5);
config.retained_log_policy.retention_max_age =
    RetentionMaxAge::from_duration(Duration::from_secs(3 * 86_400));
config.retained_log_policy.maintenance_cadence =
    MaintenanceCadence::new(Duration::from_secs(30));
config.retained_log_policy.maintenance_join_timeout =
    MaintenanceJoinTimeout::new(Duration::from_secs(2));
config.retained_log_policy.maintenance_max_work_per_pass = None; // default: unbounded work per pass
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 2. `log()`, `try_log()`, `flush()`, And Deprecated `emit()`

Use the queue-backed APIs directly in new code:

- `Logger::log(event)` validates and redacts the event, then blocks until the
  writer runtime admits it into the bounded queue.
- `Logger::try_log(event)` performs the same validation path but returns
  `TryLogError::QueueFull` immediately instead of waiting for queue space.
- successful `log()` or `try_log()` means queue admission, not durability on
  disk or console
- `Logger::flush()` is the durability barrier; call it when the caller must
  wait until queued events have been written and sinks flushed
- `Logger::emit()` remains available only as a deprecated compatibility path;
  prefer `log()` or `try_log()` in new code and examples

Blocking queue admission:

```rust
# use sc_observability::{
#     ActionName, Level, LogEvent, Logger, LoggerConfig, OutcomeLabel, ProcessIdentity,
#     SchemaVersion, ServiceName, TargetCategory, Timestamp, OBSERVATION_ENVELOPE_VERSION,
# };
# use std::path::PathBuf;
# let service = ServiceName::new("my-service")?;
# let logger = Logger::new(LoggerConfig::default_for(service.clone(), PathBuf::from("./observability")))?;
logger.log(LogEvent {
    version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)?,
    timestamp: Timestamp::now_utc(),
    level: Level::Info,
    service: service.clone(),
    target: TargetCategory::new("app.core")?,
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
# Ok::<(), Box<dyn std::error::Error>>(())
```

Non-blocking queue admission:

```rust
# use sc_observability::{
#     ActionName, Level, LogEvent, Logger, LoggerConfig, OutcomeLabel, ProcessIdentity,
#     SchemaVersion, ServiceName, TargetCategory, Timestamp, TryLogError, OBSERVATION_ENVELOPE_VERSION,
# };
# use std::path::PathBuf;
# let service = ServiceName::new("my-service")?;
# let logger = Logger::new(LoggerConfig::default_for(service.clone(), PathBuf::from("./observability")))?;
let event = LogEvent {
    version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)?,
    timestamp: Timestamp::now_utc(),
    level: Level::Info,
    service,
    target: TargetCategory::new("app.cli")?,
    action: ActionName::new("completed")?,
    message: Some("command succeeded".to_string()),
    identity: ProcessIdentity::default(),
    trace: None,
    request_id: None,
    correlation_id: None,
    outcome: Some(OutcomeLabel::new("ok")?),
    diagnostic: None,
    state_transition: None,
    fields: serde_json::Map::new(),
};

match logger.try_log(event) {
    Ok(()) => {}
    Err(TryLogError::QueueFull(_)) => {
        eprintln!("writer queue is full; inspect logger.health()");
    }
    Err(err) => return Err(Box::new(err)),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Logger::shutdown()` consumes `Logger<Running>` and returns `Logger<Stopped>`.
That typestate transition makes post-shutdown `log()`, `try_log()`, `query()`,
and `follow()` invalid by construction while still allowing health inspection on
the stopped logger value.

## 3. Default Log Root And Path

- The built-in file sink writes to `<log_root>/logs/<service>.log.jsonl`.
- The service name is part of the filename, not a parent directory.
- `Logger::health().active_log_path` is the consumer-facing way to inspect the
  resolved active path at runtime.

## 4. `SC_LOG_ROOT` Behavior

- `LoggerConfig::default_for(service, log_root)` treats a non-empty `log_root`
  as explicit configuration.
- If `log_root` is empty and `SC_LOG_ROOT` is set, `SC_LOG_ROOT` becomes the
  effective log root.
- If both are present, explicit `log_root` wins.

## 5. Enabling And Disabling Built-In Sinks

- `LoggerConfig::default_for(...)` enables the built-in file sink by default.
- `LoggerConfig::default_for(...)` disables the built-in console sink by
  default.
- Set `config.enable_file_sink = false` to disable the built-in JSONL sink.
- Set `config.enable_console_sink = true` to enable the built-in console sink.
- The built-in console sink supports `ConsoleSink::stdout()` and
  `ConsoleSink::stderr()` as the public writer-selection surface.

## 6. Registering A Custom Sink

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

## 7. Using `Logger::health()`

`Logger::health()` is the consumer-facing status snapshot for:

- aggregate logging state
- active log path
- queue depth, queue capacity, and queue high-water mark
- explicit queue-full drop count from `try_log()`
- writer-thread state and last writer error
- per-sink status
- query/follow availability
- retained-log maintenance status
- last observed logging error summary

Typical usage:

```rust
use sc_observability::WriterState;

let health = logger.health();
println!("state: {:?}", health.state);
println!("active log path: {}", health.active_log_path.display());
println!(
    "queue depth: {} / {} (high-water {})",
    health.queue_depth,
    health.queue_capacity,
    health.queue_high_water_mark
);
println!("writer state: {:?}", health.writer_state);

if health.queue_full_drops_total != 0 {
    eprintln!(
        "serious issue: {} non-blocking log events were dropped",
        health.queue_full_drops_total
    );
}

if health.writer_state != WriterState::Running {
    eprintln!("serious issue: writer runtime is {:?}", health.writer_state);
}

if let Some(error) = &health.last_writer_error {
    eprintln!(
        "last writer error: {} {}",
        error.code.as_ref().map(|code| code.as_str()).unwrap_or("<no-code>"),
        error.message
    );
}

for sink in &health.sink_statuses {
    println!("sink {} => {:?}", sink.name, sink.state);
}
```

Treat these as serious operator findings in `doctor`-style health commands:

- any non-zero `queue_full_drops_total`
- `writer_state != WriterState::Running`
- a persistent `last_writer_error`
- sustained `queue_depth` close to `queue_capacity`, especially with an elevated
  `queue_high_water_mark`

Those conditions mean the logger is degraded, pressure is outrunning the writer,
or non-blocking log calls are losing data. They should be surfaced clearly to
operators rather than treated as background implementation detail.

## 8. Deeper Docs

- Public architecture: [docs/architecture.md](./docs/architecture.md)
- Contract details: [docs/requirements.md](./docs/requirements.md)
- Query/follow surface: [docs/api-design.md](./docs/api-design.md)
- ATM-shaped defaults: [docs/atm-quickstart.md](./docs/atm-quickstart.md)

## 9. Fault Injection For Retained Sinks

The optional `fault-injection` feature adds `RetainedSinkFaultInjector`, which
forces one retained sink to report `SinkHealthState::DegradedDropping` or
`SinkHealthState::Unavailable` through the same `LoggingHealthReport`
transitions consumers see during normal operation.

Enable it only for validation runs:

```bash
cargo test --features fault-injection
```

Never enable `fault-injection` in production builds.
