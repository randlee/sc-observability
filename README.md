# sc-observability

Structured logging and OpenTelemetry observability infrastructure crates for use across projects.

## Crates

| Crate | Description |
|-------|-------------|
| [`sc-observability-types`](crates/sc-observability-types/) | Shared log type contracts — payload types, severity levels, structured field definitions |
| [`sc-observability`](crates/sc-observability/) | Core structured logging backend — `AppLogger` initialization, sink routing, and session lifecycle |
| [`sc-observe`](crates/sc-observe/) | Lightweight observer façade — emit structured log events without owning the logger lifecycle |
| [`sc-observability-otlp`](crates/sc-observability-otlp/) | OpenTelemetry export adapter — bridges `sc-observability` events to OTLP collectors |

## Usage

Add the crates you need to your `Cargo.toml`:

```toml
# For applications that own the logger lifecycle
sc-observability = "1"
sc-observability-types = "1"

# For libraries that emit log events but don't own the logger
sc-observe = "1"

# Optional: export to an OpenTelemetry collector
sc-observability-otlp = "1"
```

### Quick start

```rust
use sc_observability::{AppLogger, LoggerConfig};
use sc_observability_types::Severity;

fn main() {
    let logger = AppLogger::init(LoggerConfig::default()).expect("logger init failed");

    logger.emit(sc_observe::event!(
        severity: Severity::Info,
        message: "application started",
        fields: { "version" => env!("CARGO_PKG_VERSION") },
    ));

    // logger shuts down cleanly on drop
}
```

See [`examples/`](examples/) for complete usage patterns including OTLP export and adapter integration.

## Design

- **`sc-observability-types`** is the only shared contract crate — consumers and producers depend on it, never on each other.
- **`sc-observability`** owns logger initialization and sink configuration; only the application binary or top-level integration crate should initialize it.
- **`sc-observe`** provides a zero-lifecycle façade for library crates that need to emit events without coupling to the backend.
- **`sc-observability-otlp`** is an optional adapter; include it only when exporting to an OTLP-compatible collector.

## Publishing

See [PUBLISHING.md](PUBLISHING.md) for release and publish procedures.
