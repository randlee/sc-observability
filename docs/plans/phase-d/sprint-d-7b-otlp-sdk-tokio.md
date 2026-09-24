---
id: D.7b
status: complete
branch: feature/phase-d-7b-otlp-sdk-tokio
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-7b-otlp-sdk-tokio
depends_on: [D.7a]
relation: must_follow
owned_docs: [docs/requirements.md, docs/architecture.md, docs/migrate-error-api.md]
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
    fn blocking_preflight(&self) -> Result<(), ExportFailure>;
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

The factory validates this closed matrix before allocating providers/workers:

| Backend | Valid protocol | Required feature/runtime | Invalid result |
| --- | --- | --- | --- |
| disabled (transport disabled) | none | none | the sole no-network disabled implementation |
| `OpenTelemetrySdk` | SDK-supported gRPC or HTTP/protobuf | `otlp-sdk`; entered caller Tokio runtime | stable unsupported-protocol/runtime error |
| `LegacyHttpJson` | `HttpJson` only | `legacy-http-json`; plain-thread construction | reserved typed error until D.7c |

Delete public/production `Noop*Exporter` fallbacks; disabled construction is an
explicit private disabled set and an enabled selection can never reach it.
Every existing `OtelConfig` field receives one disposition: endpoint,
headers/auth, CA/TLS and `timeout_ms` map to the SDK/legacy builders;
`debug_local_export` is a separate diagnostic mirror outside exporter
selection; `insecure_skip_verify` is either implemented by the backend with an
explicit security warning or rejected at construction—never ignored.

### D.7b-L-owned validated transport contract

D.7b-L is the sole owner of every transport-bound field, default, validation,
and configuration error. The flat 2.0 wire surface is:

| Field | Applicability | Default when absent |
| --- | --- | --- |
| `timeout_ms` | both backends; maps to request/export timeout | `3_000` |
| `lifecycle_flush_timeout_ms` | both backends | `30_000` |
| `lifecycle_shutdown_timeout_ms` | both backends | `30_000` |
| `max_retries` | legacy only, optional on wire | `3` |
| `initial_backoff_ms` | legacy only, optional on wire | `250` |
| `max_backoff_ms` | legacy only, optional on wire | `5_000` |
| `retry_sequence_timeout_ms` | legacy only, optional on wire | `30_000` |
| `retry_after_cap_ms` | legacy only, optional on wire | `5_000` |
| `retry_jitter_percent` | legacy only, optional on wire | `20` |

For `OpenTelemetrySdk`, the three shared timeout fields map to SDK lifecycle /
export construction. Any explicit legacy-only field—including the pre-existing
`max_retries`, `initial_backoff_ms`, and `max_backoff_ms`—returns
`ConfigFieldNotApplicable`. Nothing is ignored. This 2.0 optional-field change
and its migration from the former unconditional retry defaults are documented.

Defaults are resolved **before** validation. Each resolved value retains
`ValueOrigin::{Default, Explicit}` so an error identifies both the offending
field and whether a conflicting peer was defaulted. Partial overrides are
therefore deterministic and reviewable.

All raw serialized millisecond/percent fields are converted exactly once:

```rust
pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    fn try_from_millis(field: &'static str, value: u64)
        -> Result<Self, ConfigFailure>;
}

pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

pub(crate) struct BoundedPercent(u8); // checked 0..=100

pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: PositiveDuration,
    max_backoff: PositiveDuration,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

pub(crate) struct ValidatedTransportBounds {
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

pub(crate) enum BackendTransportBounds {
    Sdk,
    Legacy(RetryPolicy),
}

impl ValidatedTransportBounds {
    fn try_from_config(config: &OtelConfig) -> Result<Self, ConfigFailure>;
}
```

The constructor derives the backend only from `config.backend`.
`LifecycleBounds` holds checked positive flush/shutdown durations;
`RetryPolicy` holds `max_retries`, checked initial/max/sequence/Retry-After
durations, and `BoundedPercent(0..=100)`. `BackendTransportBounds` makes legacy
retry state unrepresentable for SDK. Validation, using checked arithmetic, is:

- every millisecond duration is positive and convertible to `Duration`;
- `lifecycle_shutdown_timeout_ms >= timeout_ms`;
- for legacy, `max_backoff_ms >= initial_backoff_ms`;
- for legacy, `retry_sequence_timeout_ms >= timeout_ms`;
- for legacy, `0 < retry_after_cap_ms <= retry_sequence_timeout_ms`;
- for legacy, `retry_jitter_percent <= 100`.

Both factories receive only `ValidatedTransportBounds` and cannot inspect or
reparse raw fields. Deadlines use a monotonic injectable clock and start when
the public operation is admitted.

### D.7b-L stable failure inventory

This table owns common/config failures; the complete Phase D OTLP inventory is
this table union D.7c's legacy-runtime table.

| Variant | Stable code | Owning error type | Cause | Recovery |
| --- | --- | --- | --- | --- |
| `ZeroDuration` | `OTLP_CONFIG_ZERO_DURATION` | `ConfigFailure` | required duration is zero | provide a positive value |
| `DurationOverflow` | `OTLP_CONFIG_DURATION_OVERFLOW` | `ConfigFailure` | milliseconds cannot convert safely | reduce the field |
| `InvalidBoundOrdering` | `OTLP_CONFIG_BOUND_ORDER` | `ConfigFailure` | resolved ordering rule fails | correct the named explicit/defaulted fields |
| `InvalidJitterPercent` | `OTLP_CONFIG_JITTER_PERCENT` | `ConfigFailure` | jitter exceeds 100 | use `0..=100` |
| `ConfigFieldNotApplicable` | `OTLP_CONFIG_FIELD_NOT_APPLICABLE` | `ConfigFailure` | legacy-only field supplied for SDK | omit it or select legacy |
| `UnsupportedBackend` | `OTLP_UNSUPPORTED_BACKEND` | `ConfigFailure` | feature/backend unavailable | enable/select a supported backend |
| `UnsupportedProtocol` | `OTLP_UNSUPPORTED_PROTOCOL` | `ConfigFailure` | protocol invalid for backend | select a matrix-supported protocol |
| `AsyncLifecycleRequired` | `OTLP_ASYNC_LIFECYCLE_REQUIRED` | `FlushFailure` / `ShutdownFailure` | SDK sync completion requested | await the typed async operation |
| `RuntimeTerminated` | `OTLP_RUNTIME_TERMINATED` | `ShutdownFailure` | host runtime ended before completion | keep runtime alive through awaited shutdown |
| `LifecycleTimeout` | `OTLP_LIFECYCLE_TIMEOUT` | `FlushFailure` / `ShutdownFailure` | monotonic lifecycle deadline elapsed | inspect terminal health and transport/provider |
| `QueueFull` | `OTLP_QUEUE_FULL` | `TelemetryError` | bounded admission queue saturated | preserve fail-open behavior and inspect health |

## 2.0 lifecycle decision

Synchronous `emit_*` remains admission-only for the SDK backend. Construction
requires an entered Tokio handle and fails before mutation outside a runtime.
One bounded SDK dispatcher is spawned on that runtime and holds the SDK
providers/processors. Use the SDK batch processor (not a second simple/blocking
processor). Trait calls clone owned batches and use nonblocking bounded
admission; they never call `block_on`, create another runtime, wait for queue
capacity, or perform network I/O. Capacity is a validated configuration value;
full/closed queues fail open, increment existing per-signal dropped counters,
set degraded/unavailable health, and return stable `QueueFull`/worker failures.

The canonical 2.0 completion surface is:

```rust
impl Telemetry {
    pub async fn flush_async_typed(&self) -> Result<(), FlushFailure>;
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownFailure>;
}
```

The existing synchronous `flush_typed`/`shutdown_typed` compatibility methods
call `ExporterLifecycle::blocking_preflight` before removing buffers, changing
lifecycle state, or calling any signal exporter, then call `*_blocking`, all
without inspecting the backend enum. The legacy implementation completes there
synchronously from supported plain threads. The SDK preflight returns the
stable typed `AsyncLifecycleRequired` failure before any mutation; callers then
use the async method. This avoids returning success before a future collector
failure is known and avoids blocking a Tokio worker.

Every public lifecycle entry point is dispositioned together: untyped
`flush()`/`shutdown()` delegate once to the typed synchronous methods;
`flush_typed()`/`shutdown_typed()` perform backend-neutral preflight; and
`flush_async_typed()`/`shutdown_async_typed()` use the same shared barriers.
No entry point branches on `ExporterBackend`, bypasses ordering, or starts a
second completion path.

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

Every async flush/shutdown derives its finite deadline from those validated
fields; there is no unbounded or caller-implicit default.
Timeout resolves all waiters with `LifecycleTimeout`, leaves a truthful
degraded terminal state, and never reports success while work is pending.
Synchronous SDK lifecycle always returns `AsyncLifecycleRequired`, including
from a plain thread; it never uses `spawn_blocking` or blocks a current-thread
runtime. Typestate was considered and rejected because existing `Telemetry`
must support runtime-selected backends; the explicit shared state machine plus
typed preflight is the reviewable 2.0 contract.

Dropping an awaiter does not cancel the queued lifecycle command. The host must
keep its Tokio runtime alive until `shutdown_async_typed().await` completes;
after completion it may tear the runtime down immediately. If the host runtime
terminates first, dispatcher/task drop guards resolve waiters with a typed
`RuntimeTerminated` failure and account every uncompleted admitted record as
dropped/degraded. Accepted ADR-018 activates and verifies the conditional
OTLP-021 contract; the sprint also updates the 1.x-to-2.0 migration guide.

## Mandatory two-stage implementation

D.7b is one sprint but must be reviewed as two sequential implementation PRs.
**D.7b-L** first lands the backend-neutral lifecycle core, state machine,
barriers, deadlines, health/accounting, error inventory, matrix validation,
and traits with fake exporters only. **D.7b-S** must follow it and adds the
official SDK adapter and Tokio fixture. D.7c consumes D.7b-L; it must not build
a second dispatcher or lifecycle state machine. Neither sub-PR may be folded
into an unreviewable single change.

## Deliverables

1. Pin reviewed compatible versions/features of `opentelemetry`,
   `opentelemetry_sdk`, and `opentelemetry-otlp`; record dependency, license,
   Rust-version, feature, and protocol impact.
   Update `validate_repo_boundaries.sh`, `validate_dependency_bans.sh`, and
   architecture §6; automated no-exporter/SDK-only graph fixtures enforce the
   exact allowlist.
2. Implement D.7b-L's one shared lifecycle core and factory, then D.7b-S's
   common `ExporterSet`, official SDK signal adapters and outcome sink. Convert D.7a
   neutral signals without losing resource/scope metadata, kind, flags, links,
   events, status, or histogram content.
   D.7b-L owns `ValidatedTransportBounds`, `LifecycleBounds`, `RetryPolicy`,
   `BoundedPercent`, and the sole backend-aware validation constructor.
3. Implement the exact ordering/state/cancellation contract above without
   `block_on`, a hidden runtime, a process-global provider, mutex-held network
   waits, or executor-worker blocking.
4. Preserve fail-open health/dropped behavior for immediate admission,
   terminal export, runtime cancellation, and lifecycle failures. Invalid or
   unsupported combinations fail construction with stable typed errors.
   Health exposes bounded queue depth/capacity, worker/provider state,
   `last_terminal_failure`, per-signal overflow counts, and
   `retry_attempt_failures` without credentials. Transient attempts never overwrite the terminal
   field; the next successful export while `Open` clears it and records
   recovery, while `Closing`/`Shutdown` retains it. D.7c uses the same model.
5. Add an in-repository Tokio-hosted public consumer and loopback collector
   fixture covering all signals, redaction, bounded channel pressure, timeout,
   late failure, flush barriers, concurrent admission, shutdown, cancellation,
   and immediate post-completion host teardown.
   Its fixture crate is `publish = false` and excluded from publish rosters.
6. Record the lifecycle ADR, OTLP-021 revision, API approval, and migration from
   synchronous 1.x lifecycle to the backend-neutral async 2.0 completion API.
   The technical lead must accept ADR-018 before D.7b-L production code.

## Acceptance criteria

- The public facade exports all three signals through the official SDK from an
  existing Tokio runtime and calls only common exporter/lifecycle traits.
- SDK synchronous lifecycle methods return `AsyncLifecycleRequired` without
  draining buffers or changing state; the async lifecycle returns the actual
  final collector/provider failure and rejects emit synchronously once shutdown
  begins.
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
- Capacity-one/full/closed queue tests account each record exactly once; finite
  deadlines, worker/provider death, construction outside Tokio, all valid and
  invalid backend/protocol/config combinations, queue-depth health, and
  `debug_local_export`/`insecure_skip_verify` dispositions are asserted.
- Boundary fixtures cover zero/overflowing lifecycle values, shutdown shorter
  than transport timeout, exact 30-second defaults, and monotonic expiry.

## Required validation

- Focused common-trait/factory, dispatcher-ordering, current-thread,
  multi-thread, cancellation, late-failure, idempotency, and teardown tests.
- `cargo test -p sc-observability-otlp --features otlp-sdk --locked`.
- Workspace tests/clippy/rustdoc, dependency/license, public API/semver,
  requirements/ADR, and migration-doc consistency gates.
- Automated feature graph gates for no-exporter and SDK-only builds, including
  the updated repository-boundary/dependency-ban allowlists.

## Non-closure

`LegacyHttpJson` is not operational until D.7c. No downstream `atm-core` work,
Python binding, dashboard restoration, or publication.
