# Typed sink registration

This guide covers the current typed registration APIs.
`v2::LoggerBuilder::register_typed_sink` accepts an `Arc<dyn TypedLogSink>`
without requiring a consumer to invoke `legacy_sink`; it is not part of the
v1.4.1 released surface. It uses the D13 adapter internally, so canonical v2
`LogSinkError` failures retain their structured diagnostic code and original
source at the retained `LogSink` boundary.

```rust,no_run
use std::sync::Arc;
use sc_observability::{LoggerConfig, SinkHealth};
use sc_observability::v2::LoggerBuilder;
use sc_observability::typed::TypedLogSink;
use sc_observability_types::{LogEvent, SinkHealthState, SinkName};
use sc_observability_types::v2::LogSinkError;

struct CustomSink;

impl TypedLogSink for CustomSink {
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

The builder only records healthy, unique typed registrations. Re-registering
the same `Arc` returns a canonical `v2::InitError::Configuration` with code
`SC_LOG_SINK_REGISTRATION_DUPLICATE`; a degraded sink uses
`SC_LOG_SINK_REGISTRATION_INVALID`, and an unavailable sink uses
`SC_LOG_SINK_REGISTRATION_CLOSED`. The logger writer owns each write and
flush, so registering a typed sink does not add a writer, a flush path, or
level-owner authority. Existing `LogSink` registrations remain supported; D18
owns retirement of transitional compatibility adapters.
