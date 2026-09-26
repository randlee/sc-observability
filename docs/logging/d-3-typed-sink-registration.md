# Typed sink registration

`SinkRegistration::typed` and `LoggerBuilder::register_typed_sink` accept an
`Arc<dyn TypedLogSink>` without requiring a consumer to invoke `legacy_sink`.
Both paths use the D13 adapter internally, so typed failures retain their
structured diagnostic code and original source at the retained `LogSink`
boundary.

```rust,no_run
use std::sync::Arc;
use sc_observability::{LoggerBuilder, LoggerConfig, SinkHealth};
use sc_observability::typed::TypedLogSink;
use sc_observability_types::{LogEvent, SinkHealthState, SinkName};
use sc_observability_types::typed::LogSinkFailure;

struct CustomSink;

impl TypedLogSink for CustomSink {
    fn write(&self, _: &LogEvent) -> Result<(), LogSinkFailure> {
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

let mut builder = LoggerBuilder::new_typed(LoggerConfig::default_for(
    sc_observability_types::ServiceName::new("example").expect("static service name"),
    "logs".into(),
))?;
builder.register_typed_sink(Arc::new(CustomSink));
# Ok::<(), sc_observability_types::typed::InitFailure>(())
```

The builder only records registrations. The logger writer owns each write and
flush, so registering a typed sink does not add a writer, a flush path, or
level-owner authority. Existing `LogSink` registrations remain supported;
D18 owns retirement of transitional compatibility adapters.
