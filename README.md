# sc-observability

Shared structured logging, routing, and OTLP observability crates.

## Workspace Crates

| Crate | Purpose |
| --- | --- |
| [`sc-observability-types`](./crates/sc-observability-types/) | Shared contracts: identifiers, timestamps, diagnostics, health reports, query/follow value types, and error surfaces. |
| [`sc-observability`](./crates/sc-observability/) | Logging-only runtime: `Logger`, built-in file/console sinks, custom sink registration, redaction, health, query, and follow. |
| [`sc-observe`](./crates/sc-observe/) | Observation routing layer on top of logging for subscribers and projectors. |
| [`sc-observability-otlp`](./crates/sc-observability-otlp/) | OTLP/OTel export layer for logs, spans, and metrics. |

## Which Crate Do I Need?

| If you need... | Start with... |
| --- | --- |
| Logging only | `sc-observability` |
| Query/follow on JSONL logs | `sc-observability` |
| Routing one observation to logs and subscribers | `sc-observe` + `sc-observability` + `sc-observability-types` |
| OTLP export | `sc-observability-otlp` + lower layers |
| Shared value types only | `sc-observability-types` |

## Minimal Logging-Only Snippet

```rust
use std::path::PathBuf;

use sc_observability::{
    ActionName, Level, LogEvent, ProcessIdentity, ServiceName, TargetCategory, Timestamp,
    Logger, LoggerConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ServiceName::new("example-service")?;
    let logger = Logger::new(LoggerConfig::default_for(
        service.clone(),
        PathBuf::from("./observability"),
    ))?;

    logger.emit(LogEvent {
        version: "1".to_string(),
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
        outcome: Some("ok".to_string()),
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    })?;

    let health = logger.health();
    println!("active log path: {}", health.active_log_path.display());
    Ok(())
}
```

Default output goes to `<log_root>/logs/<service>.log.jsonl`.

## Fault Injection For Retained Sinks

The `fault-injection` feature exposes a `RetainedSinkFaultInjector` for live
validation. It wraps one retained sink and forces that sink to report degraded
or unavailable health through the normal `LoggingHealthReport` path without
filesystem sabotage.

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
- ATM-shaped proving example: [`examples/atm-adapter-example/`](./examples/atm-adapter-example/)

## Release / Publishing

- Publish procedure: [PUBLISHING.md](./PUBLISHING.md)
