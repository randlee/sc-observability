---
id: D.7b
status: proposed
branch: feature/phase-d-7b-otlp-sdk-tokio
base: develop
release_train: '2.0'
recommended_agent: rust-developer
recommended_model: deep-reasoning
---

# D.7b — Official SDK/Tokio exporter

## Goal and dependency

Add the production official-SDK exporter for Tokio-hosted Rust consumers while
retaining the existing synchronous `Telemetry` facade. D.7b `must_follow`s
D.7a. This repository owns a neutral Tokio fixture; no `atm-core` code or PR is
part of the sprint.

## Public contract

```rust
pub enum ExporterBackend {
    OpenTelemetrySdk,
    LegacyHttpJson, // reserved; D.7c makes this backend operational
}

pub struct OtelConfig {
    pub backend: ExporterBackend,
    pub protocol: OtlpProtocol,
    // existing endpoint/auth/TLS/timeout/retry fields remain explicit
}
```

Selecting `LegacyHttpJson` before D.7c returns a stable typed unsupported-
backend error. An enabled configuration never silently installs `Noop*Exporter`.

Both backends use the existing crate-private `LogExporter`, `TraceExporter`,
and `MetricExporter` traits. `Telemetry` stores only those trait objects; the
backend enum is consumed by construction/injection code and is never branched
on by `emit_*`, flush, or shutdown.

```rust
pub(crate) trait ExporterLifecycle: Send + Sync {
    fn flush(&self) -> Result<(), ExportFailure>;
    fn shutdown(&self) -> Result<(), ExportFailure>;
}

pub(crate) trait LogExporter: ExporterLifecycle {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportFailure>;
}

pub(crate) trait TraceExporter: ExporterLifecycle {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportFailure>;
}

pub(crate) trait MetricExporter: ExporterLifecycle {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportFailure>;
}
```

The three SDK adapters share one reference-counted backend state. That state
deduplicates provider lifecycle commands and owns a completion outcome sink,
so async task results update the same health counters read by `Telemetry`.

The traits remain synchronous **admission** boundaries. The SDK adapters clone
owned batches and hand them to the caller's captured Tokio `Handle` with
`spawn`; they never call `block_on`, construct another runtime, or wait for
network I/O on a Tokio worker. Immediate admission failures are returned by
the trait method. Task completion records success/failure through the existing
shared health and dropped-export accounting. Flush and shutdown send ordered
barriers through the same adapter state; shutdown closes admission
synchronously (so a later `emit_*` returns `TelemetryError::Shutdown`) and
dispatches SDK provider shutdown exactly once. This shape preserves the
synchronous public facade without pretending that the SDK transport itself is
synchronous.

## Deliverables

1. Pin reviewed compatible versions/features of `opentelemetry`,
   `opentelemetry_sdk`, and `opentelemetry-otlp`; record dependency, license,
   Rust-version, feature, and protocol impact.
2. Implement the typed backend selector and SDK adapters for logs, traces, and
   metrics behind the same crate-private per-signal exporter traits used by the
   synchronous backend. Selection constructs and injects trait objects;
   `Telemetry` contains no per-backend emit/flush/shutdown branch. Convert D.7a
   neutral signals to SDK-native values without losing resource/scope metadata,
   kind, flags, links, events, status, or histogram content.
3. Implement the synchronous-admission/async-execution bridge defined above:
   capture a caller-hosted Tokio `Handle`, use non-blocking task hand-off, and
   create no hidden process-global or secondary runtime. Ordered flush and
   shutdown barriers reach exactly-once SDK provider/processor lifecycle calls.
   No `block_on`, network wait, or completion wait may occur while holding the
   telemetry state mutex or on an executor critical path.
4. Inject the shared completion outcome sink and preserve existing fail-open
   export health and dropped-count behavior for both immediate admission and
   late task failures.
   Disabled telemetry remains no-network; invalid/unsupported combinations
   fail construction with stable typed errors.
5. Add an in-repository Tokio-hosted consumer and loopback collector fixture
   covering all three signals, auth redaction, timeout/failure, flush, and
   idempotent shutdown through public APIs.

## Acceptance criteria

- The public `Telemetry` facade exports logs, traces, and metrics through the
  official SDK to a real loopback collector from an existing Tokio runtime.
- The public facade calls only the common crate-private exporter traits; a
  construction test proves that changing `ExporterBackend` changes injected
  implementations without changing facade control flow.
- No hidden runtime or process-global provider is created; two telemetry
  instances remain isolated and each shuts down exactly once.
- The host runtime stays responsive while export/flush occurs; mutex and
  shutdown tests detect deadlock, `block_on`, or Tokio-worker blocking, and a
  late async failure updates health/dropped accounting.
- Shutdown rejects subsequent emission synchronously, while its ordered SDK
  lifecycle command is dispatched exactly once.
- Enabled SDK configuration cannot resolve to a no-op; disabled configuration
  makes no request; unsupported backend/protocol selection fails eagerly.
- SDK errors update the existing health/dropped contracts without exposing
  credentials.

## Required validation

- Focused common-trait injection, SDK adapter, ordered-barrier,
  runtime-ownership, loopback, late-failure, and shutdown tests under
  current-thread and multi-thread Tokio runtimes.
- `cargo test -p sc-observability-otlp --features otlp-sdk --locked`.
- Workspace tests/clippy/rustdoc, dependency/license policy, and API approval.

## Non-closure

`LegacyHttpJson` is not operational until D.7c. No downstream `atm-core` work,
Python binding, dashboard restoration, or publication.
