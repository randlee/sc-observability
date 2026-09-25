---
id: D.6
status: planned
branch: feature/phase-d-6-otlp-sdk-tokio
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-6-otlp-sdk-tokio
depends_on: [D.5]
relation: must_follow
assignee: aobs
model_class: astra
owned_docs: [docs/requirements.md, docs/architecture.md, docs/api-design.md, docs/migrate-error-api.md]
release_train: '2.0'
---

# D.6 — Official SDK/Tokio exporter

## Goal and dependency

Add the production official-SDK exporter for Tokio-hosted Rust consumers while
preserving synchronous emit admission and giving asynchronous transport
lifecycle an honest awaitable completion surface. D.6 `must_follow`s D.5.
This repository owns a neutral Tokio fixture; no `atm-core` code or PR is part
of the sprint.

## Backend and trait contract

```rust
pub enum ExporterBackend {
    OpenTelemetrySdk,
    LegacyHttpJson, // reserved; D.7 makes this backend operational
}

pub struct OtelConfig {
    pub backend: ExporterBackend,
    pub protocol: OtlpProtocol,
    // endpoint/auth/TLS and timeout fields remain explicit;
    // legacy-only retry fields are optional (authoritative D.6-L table below)
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
before D.7 returns a stable typed unsupported-backend error. An enabled
configuration never silently installs a no-op exporter.

The factory validates this closed matrix before allocating providers/workers:

| Backend | Valid protocol | Required feature/runtime | Invalid result |
| --- | --- | --- | --- |
| disabled (transport disabled) | none | none | the sole no-network disabled implementation |
| `OpenTelemetrySdk` | SDK-supported gRPC or HTTP/protobuf | `otlp-sdk`; entered caller Tokio runtime | stable unsupported-protocol/runtime error |
| `LegacyHttpJson` | `HttpJson` only | `legacy-http-json`; plain-thread construction | reserved typed error until D.7 |

Delete public/production `Noop*Exporter` fallbacks; disabled construction is an
explicit private disabled set and an enabled selection can never reach it.
Every existing `OtelConfig` field receives one disposition: endpoint,
headers/auth, CA/TLS and `timeout_ms` map to the SDK/legacy builders;
`debug_local_export` is a separate diagnostic mirror outside exporter
selection; `insecure_skip_verify` is either implemented by the backend with an
explicit security warning or rejected at construction—never ignored.

### D.6-L-owned validated transport contract

D.6-L is the sole owner of every transport-bound field, default, validation,
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
pub enum OtlpConfigField {
    Timeout,
    LifecycleFlushTimeout,
    LifecycleShutdownTimeout,
    MaxRetries,
    InitialBackoff,
    MaxBackoff,
    RetrySequenceTimeout,
    RetryAfterCap,
    RetryJitterPercent,
}

pub enum ValueOrigin { Default, Explicit }

pub struct ResolvedField<T> {
    pub field: OtlpConfigField,
    pub value: T,
    pub origin: ValueOrigin,
}

pub enum OtlpConfigTarget { Disabled, Backend(ExporterBackend) }

pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    fn try_from_millis(field: OtlpConfigField, value: u64)
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
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

impl ValidatedTransportBounds {
    fn try_from_config(config: &OtelConfig) -> Result<Self, ConfigFailure>;
}
```

The constructor derives `Disabled` from `config.enabled == false`; otherwise
it derives the backend only from `config.backend`.
`LifecycleBounds` holds checked positive flush/shutdown durations;
`RetryPolicy` holds `max_retries`, checked initial/max/sequence/Retry-After
durations, and `BoundedPercent(0..=100)`. `BackendTransportBounds` makes legacy
retry state unrepresentable for SDK. Validation, using checked arithmetic, is:

- every millisecond duration is positive and convertible to `Duration`;
- `lifecycle_shutdown_timeout_ms >= timeout_ms`;
- `lifecycle_flush_timeout_ms >= timeout_ms`;
- for legacy, `max_backoff_ms >= initial_backoff_ms`;
- for legacy, `retry_sequence_timeout_ms >= timeout_ms`;
- for legacy, `0 < retry_after_cap_ms <= retry_sequence_timeout_ms`;
- for legacy, `retry_jitter_percent <= 100`;
- reject every explicit field inapplicable to disabled transport or the
  selected backend with `ConfigFieldNotApplicable`;
- reject a requested insecure verification override when the selected backend
  does not explicitly support it with `InsecureTransportRejected`.

Checks execute in exactly this listed order and return the first failure; they
are not aggregated. Within the first bullet, fields are checked in the wire
table's top-to-bottom order. This makes every multi-violation diagnostic
deterministic.

Both factories receive only `ValidatedTransportBounds` and cannot inspect or
reparse raw fields. Deadlines use a monotonic injectable clock and start when
the public operation is admitted.

Construction order is fixed: resolve defaults and create `ResolvedField`
values; run the ordered validation list above; build
`ValidatedTransportBounds`; then check feature/backend/protocol availability.
Thus a malformed legacy config fails deterministically before D.6's reserved
`UnsupportedBackend`. Disabled transport still validates explicitly supplied
shared fields, rejects every explicit legacy-only retry field with
`ConfigFieldNotApplicable { target: OtlpConfigTarget::Disabled, .. }`, yields
`BackendTransportBounds::Disabled`, and never constructs a network
provider/worker. SDK inapplicability instead records
`OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk)`.

Configuration variants carry reviewable payloads:

```rust
ZeroDuration { field: OtlpConfigField, origin: ValueOrigin }
DurationOverflow { field: OtlpConfigField, raw: u64, origin: ValueOrigin }
InvalidBoundOrdering {
    lower: ResolvedField<u64>,
    upper: ResolvedField<u64>,
}
InvalidJitterPercent { field: OtlpConfigField, raw: u8, origin: ValueOrigin }
ConfigFieldNotApplicable { field: OtlpConfigField, target: OtlpConfigTarget }
InsecureTransportRejected { backend: ExporterBackend }
TransportConstructionFailed { backend: ExporterBackend, source: Diagnostic }
```

### D.6-L stable failure inventory

This is the complete Phase D OTLP stable-error inventory. D.6-L exclusively
owns these variants, codes, owning types, mappings, and documentation; later
sprints consume this table without adding or restating rows. Configuration
rows are construction-only `ConfigFailure` variants. Every runtime/lifecycle
variant except `Shutdown` is owned by `ExportFailure`; `Shutdown` is owned by
`TelemetryError`. Construction never uses an additional wrapper variant: it
returns the named `ConfigFailure` rows below. A legacy async-context failure
during construction is the redacted `Diagnostic` source of
`TransportConstructionFailed`; the same condition during synchronous lifecycle
is `BlockingBackendInAsyncContext` directly. Emit/lifecycle façades convert
runtime failures to `TelemetryError`, `FlushFailure`, or `ShutdownFailure`
without changing their stable code or typed source. In particular, `QueueFull` is created as an
`ExportFailure`; emit converts it through `From<ExportFailure> for
TelemetryError`, while lifecycle converts the same source through the
corresponding typed lifecycle failure.

| Variant | Stable code | Owning error type | Cause | Recovery | Redaction | Retryability |
| --- | --- | --- | --- | --- | --- | --- |
| `ZeroDuration` | `OTLP_CONFIG_ZERO_DURATION` | `ConfigFailure` | required duration is zero | provide a positive value | field/value only | after config correction |
| `DurationOverflow` | `OTLP_CONFIG_DURATION_OVERFLOW` | `ConfigFailure` | milliseconds cannot convert safely | reduce the field | field/value only | after config correction |
| `InvalidBoundOrdering` | `OTLP_CONFIG_BOUND_ORDER` | `ConfigFailure` | resolved ordering rule fails | correct the named explicit/defaulted fields | field/value/origin only | after config correction |
| `InvalidJitterPercent` | `OTLP_CONFIG_JITTER_PERCENT` | `ConfigFailure` | jitter exceeds 100 | use `0..=100` | field/value only | after config correction |
| `ConfigFieldNotApplicable` | `OTLP_CONFIG_FIELD_NOT_APPLICABLE` | `ConfigFailure` | field is inapplicable to disabled transport or the selected backend | omit it, enable transport, or select its applicable backend | field/closed target only | after config correction |
| `InsecureTransportRejected` | `OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED` | `ConfigFailure` | selected backend does not implement the requested insecure verification override | disable the override or choose an explicitly supporting backend | backend only | after config correction |
| `TransportConstructionFailed` | `OTLP_TRANSPORT_CONSTRUCTION_FAILED` | `ConfigFailure` | CA/auth/client/provider/legacy-worker initialization failed | correct the bounded typed source and reconstruct | bounded typed source; never path contents, credentials, header values, or response bodies | after config/environment correction |
| `UnsupportedBackend` | `OTLP_UNSUPPORTED_BACKEND` | `ConfigFailure` | feature/backend unavailable | enable/select a supported backend | enum values only | after build/config correction |
| `UnsupportedProtocol` | `OTLP_UNSUPPORTED_PROTOCOL` | `ConfigFailure` | protocol invalid for backend | select a matrix-supported protocol | enum values only | after config correction |
| `TokioRuntimeRequired` | `OTLP_TOKIO_RUNTIME_REQUIRED` | `ConfigFailure` | SDK construction lacks an entered Tokio runtime | construct inside the host runtime | no dynamic data | after entering a runtime |
| `BlockingBackendInAsyncContext` | `OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT` | `ExportFailure` | legacy synchronous lifecycle entered Tokio; construction preserves this condition as the redacted source of `TransportConstructionFailed` | use a plain thread or async lifecycle | no dynamic data | in a supported context |
| `AsyncLifecycleRequired` | `OTLP_ASYNC_LIFECYCLE_REQUIRED` | `ExportFailure` | SDK synchronous completion requested | await the typed async operation | no dynamic data | through async lifecycle |
| `RuntimeTerminated` | `OTLP_RUNTIME_TERMINATED` | `ExportFailure` | host runtime ended before completion | keep the runtime alive through awaited shutdown | bounded state/counts | with a live replacement runtime/instance |
| `LifecycleTimeout` | `OTLP_LIFECYCLE_TIMEOUT` | `ExportFailure` | monotonic lifecycle deadline elapsed | inspect terminal health and transport/provider | duration/state only | operation-specific |
| `QueueFull` | `OTLP_QUEUE_FULL` | `ExportFailure` | bounded admission queue saturated | preserve fail-open behavior and inspect health | capacity/depth only | yes, later admission |
| `WorkerTerminated` | `OTLP_WORKER_TERMINATED` | `ExportFailure` | SDK dispatcher or legacy worker terminated unexpectedly | correct the terminal cause and construct a new instance | bounded typed source; no credentials | only with a new instance |
| `ShutdownCancelledRetry` | `OTLP_SHUTDOWN_CANCELLED_RETRY` | `ExportFailure` | shutdown cancelled a retryable pre-barrier legacy sequence | inspect terminal health; resend only if duplicates are acceptable | attempt/count only | caller decision; duplicates possible |
| `RetryDeadlineExhausted` | `OTLP_RETRY_DEADLINE_EXHAUSTED` | `ExportFailure` | no legacy sequence budget remains | increase the validated sequence bound or restore collector health | budget/attempt only | new operation after recovery |
| `NonRetryableHttpStatus` | `OTLP_HTTP_STATUS_TERMINAL` | `ExportFailure` | collector returned a non-retryable HTTP status | correct request/auth/config before retrying | status/category only; no body/headers | after cause correction |
| `RetryAttemptsExhausted` | `OTLP_RETRY_ATTEMPTS_EXHAUSTED` | `ExportFailure` | legacy maximum attempts ended before success | restore collector health or adjust the validated policy | attempt/count only | new operation after recovery |
| `TerminalExportFailure` | `OTLP_EXPORT_TERMINAL` | `ExportFailure` | SDK or legacy provider returned a terminal export failure | inspect the preserved source and collector state | bounded typed source; no credentials | source-dependent |
| `Shutdown` | `OTLP_TELEMETRY_SHUTDOWN` | `TelemetryError` | emit was attempted after shutdown began | construct a new telemetry instance | no dynamic data | only on a new instance |

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

## Implementation order

Land the backend-neutral lifecycle core before wiring the official SDK adapter
and Tokio fixture. D.7 consumes that shared core and must not build a second
dispatcher or lifecycle state machine.

## Deliverables

1. Pin reviewed compatible versions/features of `opentelemetry`,
   `opentelemetry_sdk`, and `opentelemetry-otlp`; record dependency, license,
   Rust-version, feature, and protocol impact.
   Update `validate_repo_boundaries.sh`, `validate_dependency_bans.sh`, and
   architecture §6; automated no-exporter/SDK-only graph fixtures enforce the
   exact allowlist.
2. Implement D.6-L's one shared lifecycle core and factory, then D.6-S's
   common `ExporterSet`, official SDK signal adapters and outcome sink. Convert D.5
   neutral signals without losing resource/scope metadata, kind, flags, links,
   events, status, or histogram content.
   D.6-L owns `PositiveDuration`, `LifecycleBounds`, `RetryPolicy`,
   `BoundedPercent`, `BackendTransportBounds`, `ValidatedTransportBounds`,
   `OtlpConfigField`, `OtlpConfigTarget`, `ValueOrigin`, `ResolvedField`, the
   sole backend-aware validation constructor, and the complete stable-error
   inventory above. The four public payload types and public `ConfigFailure`
   shapes are included in API approval and the 2.0 semver manifest.
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
   recovery, while `Closing`/`Shutdown` retains it. D.7 uses the same model.
5. Add an in-repository Tokio-hosted public consumer and loopback collector
   fixture covering all signals, redaction, bounded channel pressure, timeout,
   late failure, flush barriers, concurrent admission, shutdown, cancellation,
   and immediate post-completion host teardown.
   Its fixture crate is `publish = false` and excluded from publish rosters.
6. Record the lifecycle ADR, OTLP-020/021 revisions, API approval, rustdoc, and
   migration from synchronous 1.x lifecycle to the backend-neutral async 2.0
   completion API. D.6-L owns those config/error/lifecycle documents and their
   shared validation fixtures; D.7 may only verify them by reference.
   The technical lead must accept ADR-018 before D.6-L production code.

## D.6-L validation fixtures

- Resolve every field through the one constructor and assert its value and
  `ValueOrigin`; cover each field absent and explicitly supplied.
- Freeze partial overrides and first-error order: SDK `timeout_ms = 40_000`
  first returns `InvalidBoundOrdering` for the defaulted shutdown bound; legacy
  `timeout_ms = 40_000` with explicit flush/shutdown bounds of `50_000` reaches
  the defaulted sequence-bound failure. Separate explicit flush and shutdown
  values of `2_000` each fail against the defaulted request timeout. Every
  diagnostic names the public field, values, and origins. Exercise each
  legacy-only field alone through the same constructor.
- Prove malformed values and ordering fail before unsupported backend/protocol
  checks, while disabled transport validates explicit shared values, rejects
  explicit legacy-only fields, and never constructs network state.
  Pin disabled transport with `backend = LegacyHttpJson` and explicit
  `max_retries`; it returns `ConfigFieldNotApplicable` with target `Disabled`,
  not the otherwise-applicable backend target.
- Freeze combined violations in the same ordered pipeline. SDK with explicit
  `initial_backoff_ms = 0` returns `ZeroDuration` before the later
  backend-applicability failure. Disabled legacy selection with explicit
  `max_retries` plus `lifecycle_shutdown_timeout_ms = 2_000` returns the shared
  `InvalidBoundOrdering` failure before the later disabled-target failure.
- Freeze independent legacy delay caps: fallback delay is
  `min(jittered_exponential, max_backoff, remaining_sequence_budget)`, whereas
  a valid server delay is `min(retry_after, retry_after_cap,
  remaining_sequence_budget)`. Cover both `max_backoff < retry_after_cap` and
  `retry_after_cap < max_backoff`; neither cap silently truncates the other.

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
- Construction fixtures cover unsupported insecure verification, unreadable CA,
  invalid auth-header construction, SDK/provider builder failure, and legacy
  worker/client initialization; each yields the exact D.6-L construction-only
  variant with a redacted typed source.
- Capacity-one/full/closed queue tests account each record exactly once; finite
  deadlines, worker/provider death, construction outside Tokio, all valid and
  invalid backend/protocol/config combinations, queue-depth health, and
  `debug_local_export`/`insecure_skip_verify` dispositions are asserted.
- Boundary fixtures cover zero/overflowing lifecycle values, flush and shutdown
  shorter than transport timeout, exact 30-second defaults, deterministic
  first-error ordering, and monotonic expiry.

## Required validation

- Focused common-trait/factory, dispatcher-ordering, current-thread,
  multi-thread, cancellation, late-failure, idempotency, and teardown tests.
- `cargo test -p sc-observability-otlp --features otlp-sdk --locked`.
- Workspace tests/clippy/rustdoc, dependency/license, public API/semver,
  requirements/ADR, and migration-doc consistency gates.
- Automated feature graph gates for no-exporter and SDK-only builds, including
  the updated repository-boundary/dependency-ban allowlists.

## Non-closure

`LegacyHttpJson` is not operational until D.7. No downstream `atm-core` work,
Python binding, dashboard restoration, or publication.
