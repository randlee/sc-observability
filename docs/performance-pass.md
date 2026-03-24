# Performance Pass

## Scope

Sprint 6 review of hot-path allocations and fan-out behavior in:

- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

## Findings

### Logging fan-out

- `Logger::emit(...)` validates once, redacts once, and then fan-outs the same
  redacted event to sinks.
- Sink fan-out does not clone the event per sink in the logger itself.
- Significant additional allocation was not found in the logging hot path
  beyond JSON serialization and sink-specific write behavior.

### Observation routing

- `Observability::emit(...)` routes a typed observation through construction-time
  registrations in deterministic order.
- Routing itself does not maintain a background queue in v1.
- Projectors naturally allocate their own output vectors (`Vec<LogEvent>`,
  `Vec<SpanSignal>`, `Vec<MetricRecord>`), which is part of the documented API
  contract rather than accidental overhead.

### OTLP span assembly

- `SpanAssembler` currently constructs a string key from trace/span ids for its
  internal hash maps.
- This introduces one small allocation per signal path.
- For v1 this is acceptable because the assembler remains simple, correct, and
  isolated within the OTLP crate.

## Outcome

- No significant hot-path allocation issue was found that blocks release.
- No mandatory pre-release performance refactor is required.
- The main deferred optimization opportunity is replacing the current span-key
  string assembly with a dedicated structured key type if future profiling
  shows it to be material.
