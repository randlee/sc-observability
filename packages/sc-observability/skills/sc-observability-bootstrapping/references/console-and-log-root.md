# Console And Log Root

## In-Code Log Root Override

The standard setup passes an explicit `log_root` into
`LoggerConfig::default_for(service, log_root)`.

If the user wants a different default than `~/.<app>`, change that path in the
app's configuration layer rather than changing the house style itself.

## `SC_LOG_ROOT` Precedence

`LoggerConfig::default_for(...)` uses `SC_LOG_ROOT` only when the provided
`log_root` is empty.

Practical rule:

- explicit non-empty `log_root` wins
- empty `log_root` allows `SC_LOG_ROOT` to take effect

## Easiest Console Logging

For simple stdout console logging:

```rust
config.enable_console_sink = true;
```

This uses the built-in `ConsoleSink::stdout()` path.

## Stderr Console Logging

If the user wants stderr console output, use `LoggerBuilder` and register
`ConsoleSink::stderr()` explicitly:

```rust
use std::sync::Arc;

use sc_observability::{ConsoleSink, LoggerBuilder, SinkRegistration};

config.enable_console_sink = false;
let mut builder = LoggerBuilder::new(config)?;
builder.register_sink(SinkRegistration::new(Arc::new(ConsoleSink::stderr())));
let logger = builder.build();
```

## Usage Modes

- local development: file + console is often useful
- long-running production service: file-only is the default baseline
- CLI: file-only or file + stderr depending on operator preference

Avoid enabling multiple console sinks accidentally.
