# Performance Pass

## Status

Historical baseline plus phase-A follow-on note.

## Scope

Sprint 6 review of hot-path allocations and fan-out behavior in:

- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

## Findings

### Logging fan-out baseline

- Before phase A, `Logger::emit(...)` validated once, redacted once, and then
  fanned out the same redacted event to sinks on the caller thread.
- Sink fan-out did not clone the event per sink in the logger itself.
- Significant additional allocation was not found in that pre-phase-A hot path
  beyond JSON serialization and sink-specific write behavior.

### Phase-A follow-on

- Phase A supersedes the caller-thread sink-write baseline with a queue-backed
  writer-thread model.
- The approved follow-on optimization is structural rather than
  micro-allocational: producer calls validate, redact, and enqueue, while one
  writer thread owns batching, sink writes, rotation, pruning, flush, and
  shutdown drain completion with timeout-threshold degradation reporting.
- This document is not a veto on that redesign; it is the historical
  measurement note that motivated the phase-A architectural follow-on.

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

- No significant pre-phase-A hot-path allocation issue was found that blocked
  the `v1.1.0` release.
- Phase A intentionally takes a larger runtime-architecture step for lock and
  producer-path decoupling reasons, not because this baseline found a release
  blocker.
- The main deferred optimization opportunity is replacing the current span-key
  string assembly with a dedicated structured key type if future profiling
  shows it to be material.
