# d-7: Official SDK/Tokio adapter

## Plan metadata

- Wave: 10
- Branch: `sprint/d-7-otlp-sdk-tokio`
- PR target: `sprint/d-6-otlp-lifecycle-core`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-otlp/src/assembly.rs`
  - `examples/otlp-sdk/**`

## Goal and dependency

After D.6, wire the reviewed official OpenTelemetry SDK adapter into the shared
lifecycle core. This sprint owns the Tokio-hosted adapter and its public
consumer fixture, not lifecycle state, error definitions, or a downstream
`atm-core` integration.


## Deliverables

1. Pin reviewed `opentelemetry`, `opentelemetry_sdk`, and
   `opentelemetry-otlp` versions/features; document the feature-gated
   dependency boundary in architecture §6.
2. Implement adapters from D.5 neutral logs, traces, and metrics to the SDK.
   Preserve resource/scope metadata, trace kind/flags/links/events/status, and
   complete histogram data.
3. Use D.6's factory, lifecycle commands, bounds, health, and canonical
   failure types. `OpenTelemetrySdk` with `Grpc` is the default enabled
   backend/protocol pair; unsupported pairs fail construction before admission.
4. Add a Tokio-hosted public consumer and loopback collector fixture covering
   all signals, redaction, queue pressure, timeout, flush, shutdown,
   cancellation, and host teardown after awaited completion.


## Non-closure

No legacy HTTP/JSON implementation, operational dashboard work, Python OTEL
surface, or `atm-core` code.


## Design

## Implementation targets

- `crates/sc-observability-otlp/src/assembly.rs`: implement the SDK/Tokio adapter constructor that satisfies the D.12 exporter trait (deliverable 1).
- `examples/otlp-sdk/**`: add an SDK adapter configuration and lifecycle example (deliverable 2).
- Adapter tests: exercise export, cancellation, retry, and shutdown through the D.6 lifecycle core (deliverables 3–4).


## Acceptance criteria

## Acceptance criteria

- The adapter adds no dispatcher, hidden runtime, process-global provider, or
  second lifecycle/error contract.
- Awaited shutdown reports terminal export failure and permits immediate host
  runtime teardown after success.
- SDK-only feature tests prove no legacy HTTP/JSON dependency is enabled.


## Required validation

- Focused SDK adapter/collector tests and the Tokio consumer fixture.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc, and
  the existing dependency-boundary checks.


