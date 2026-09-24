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
preserving synchronous emit admission and giving asynchronous transport
lifecycle an honest awaitable completion surface. D.7b `must_follow`s D.7a.
This repository owns a neutral Tokio fixture; no `atm-core` code or PR is part
of the sprint.

## Backend and trait contract

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

type LifecycleFuture = Pin<
    Box<dyn Future<Output = Result<(), ExportFailure>> + Send + 'static>
>;

pub(crate) trait ExporterLifecycle: Send + Sync {
    fn flush_async(&self) -> LifecycleFuture;
    fn shutdown_async(&self) -> LifecycleFuture;
    fn flush_blocking(&self) -> Result<(), ExportFailure>;
    fn shutdown_blocking(&self) -> Result<(), ExportFailure>;
}

pub(crate) trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportFailure>;
}

pub(crate) trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportFailure>;
}

pub(crate) trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportFailure>;
}

pub(crate) struct ExporterSet {
    logs: Arc<dyn LogExporter>,
    traces: Arc<dyn TraceExporter>,
    metrics: Arc<dyn MetricExporter>,
    lifecycle: Arc<dyn ExporterLifecycle>,
}
```

Both backends construct the same `ExporterSet`; `Telemetry` stores only these
trait objects. `ExporterBackend` is consumed by construction/injection and is
never branched on by emit, flush, or shutdown. Selecting `LegacyHttpJson`
before D.7c returns a stable typed unsupported-backend error. An enabled
configuration never silently installs a no-op exporter.

## 2.0 lifecycle decision

Synchronous `emit_*` remains admission-only for the SDK backend. One bounded
SDK dispatcher is spawned on the caller's current Tokio runtime and holds the
SDK providers/processors. Trait calls clone owned batches and synchronously
enqueue ordered commands; they never call `block_on`, create another runtime,
or wait for network I/O.

The canonical 2.0 completion surface is:

```rust
impl Telemetry {
    pub async fn flush_async_typed(&self) -> Result<(), FlushFailure>;
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownFailure>;
}
```

The existing synchronous `flush_typed`/`shutdown_typed` compatibility methods
call `ExporterLifecycle::*_blocking` without inspecting the backend enum. The
legacy implementation completes there synchronously. The SDK implementation
returns the stable typed `AsyncLifecycleRequired` failure **before** enqueueing
a barrier or changing lifecycle state; callers then use the async method. This
avoids returning success before a future collector failure is known and avoids
blocking a Tokio worker.

The dispatcher linearizes every export and lifecycle command under one short
admission lock with a monotonic sequence. A flush barrier completes only after
all commands sequenced before it have terminal outcomes. Concurrent emission
is either sequenced before that barrier or after it for the next flush. Async
shutdown atomically changes `Open -> Closing` under the same lock, rejects all
new emit calls synchronously with `TelemetryError::Shutdown`, drains all prior
admissions, performs provider shutdown exactly once, stores the terminal
result, and changes `Closing -> Shutdown`. Concurrent/repeated shutdown awaits
the shared completion while it is in flight; later calls after terminal
completion are idempotent and return `Ok(())`, preserving the existing
first-caller failure rule.

Dropping an awaiter does not cancel the queued lifecycle command. The host must
keep its Tokio runtime alive until `shutdown_async_typed().await` completes;
after completion it may tear the runtime down immediately. If the host runtime
terminates first, dispatcher/task drop guards resolve waiters with a typed
`RuntimeTerminated` failure and account every uncompleted admitted record as
dropped/degraded. The plan records this decision in the architecture/ADR and
amends OTLP-021 and the 1.x-to-2.0 migration guide accordingly.

## Deliverables

1. Pin reviewed compatible versions/features of `opentelemetry`,
   `opentelemetry_sdk`, and `opentelemetry-otlp`; record dependency, license,
   Rust-version, feature, and protocol impact.
2. Implement the backend factory, common `ExporterSet`, official SDK signal
   adapters, shared dispatcher, and completion outcome sink. Convert D.7a
   neutral signals without losing resource/scope metadata, kind, flags, links,
   events, status, or histogram content.
3. Implement the exact ordering/state/cancellation contract above without
   `block_on`, a hidden runtime, a process-global provider, mutex-held network
   waits, or executor-worker blocking.
4. Preserve fail-open health/dropped behavior for immediate admission,
   terminal export, runtime cancellation, and lifecycle failures. Invalid or
   unsupported combinations fail construction with stable typed errors.
5. Add an in-repository Tokio-hosted public consumer and loopback collector
   fixture covering all signals, redaction, bounded channel pressure, timeout,
   late failure, flush barriers, concurrent admission, shutdown, cancellation,
   and immediate post-completion host teardown.
6. Record the lifecycle ADR, OTLP-021 revision, API approval, and migration from
   synchronous 1.x lifecycle to the backend-neutral async 2.0 completion API.

## Acceptance criteria

- The public facade exports all three signals through the official SDK from an
  existing Tokio runtime and calls only common exporter/lifecycle traits.
- SDK synchronous lifecycle methods return `AsyncLifecycleRequired` without
  changing state; the async lifecycle returns the actual final collector/
  provider failure and rejects emit synchronously once shutdown begins.
- Barrier-order tests prove the disposition of emission racing flush/shutdown;
  provider shutdown occurs exactly once for concurrent/repeated callers.
- Current-thread and multi-thread Tokio tests complete without deadlock,
  `block_on`, worker blocking, a second runtime, or global-provider leakage.
- An awaited final-export failure is surfaced as `ShutdownFailure`, and the
  host can immediately tear down its runtime after the await without loss.
- Premature runtime teardown produces `RuntimeTerminated`, accounts pending
  records as dropped, and never reports successful completion.
- Two telemetry instances remain isolated; enabled SDK config cannot resolve
  to no-op; disabled config makes no request; credentials never enter errors.

## Required validation

- Focused common-trait/factory, dispatcher-ordering, current-thread,
  multi-thread, cancellation, late-failure, idempotency, and teardown tests.
- `cargo test -p sc-observability-otlp --features otlp-sdk --locked`.
- Workspace tests/clippy/rustdoc, dependency/license, public API/semver,
  requirements/ADR, and migration-doc consistency gates.

## Non-closure

`LegacyHttpJson` is not operational until D.7c. No downstream `atm-core` work,
Python binding, dashboard restoration, or publication.
