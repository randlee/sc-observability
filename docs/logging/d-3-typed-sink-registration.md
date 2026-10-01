# Typed sink registration

This guide covers the current canonical sink registration APIs.
`v2::LoggerBuilder::register_typed_sink` accepts an `Arc<dyn v2::LogSink>`
and stores that exact `Arc` in the runtime, with no adapter and without
requiring a consumer to invoke `legacy_sink`; it is not part of the v1.4.1
released surface. `v2::LoggerBuilder` is its canonical owner, and the retained
`LoggerBuilder` facade forwards to it. `v2::LogSink` reports canonical v2
`LogSinkError` failures, so they retain their structured diagnostic code and
original source all the way to the runtime. The released `LogSink` and
`typed::TypedLogSink` traits keep their released error types and adapt once, at
registration.

```rust,no_run
use std::sync::Arc;
use sc_observability::{LoggerConfig, SinkHealth};
use sc_observability::v2::{LogSink, LoggerBuilder};
use sc_observability_types::{LogEvent, SinkHealthState, SinkName};
use sc_observability_types::v2::LogSinkError;

struct CustomSink;

impl LogSink for CustomSink {
    fn write(&self, _: &LogEvent) -> Result<(), LogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("custom").expect("static sink name"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

let mut builder = LoggerBuilder::new(LoggerConfig::default_for(
    sc_observability_types::ServiceName::new("example").expect("static service name"),
    "logs".into(),
))?;
builder.register_typed_sink(Arc::new(CustomSink))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The builder only records healthy, unique canonical registrations, checked
against every sink already registered (`register_sink` is the released
infallible path and stores its registration without this validation). Re-registering
the same `Arc` returns a canonical `v2::InitError::Configuration` with code
`SC_LOG_SINK_REGISTRATION_DUPLICATE`; a degraded sink uses
`SC_LOG_SINK_REGISTRATION_INVALID`, and an unavailable sink uses
`SC_LOG_SINK_REGISTRATION_CLOSED`. The logger writer owns each write and
flush, so registering a canonical sink does not add a writer, a flush path, or
level-owner authority. Existing `LogSink` registrations remain supported; D18
owns retirement of transitional compatibility adapters.
