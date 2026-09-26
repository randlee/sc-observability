# d-6: OTLP lifecycle core

## Plan metadata

- Wave: 9
- Branch: `sprint/d-6-otlp-lifecycle-core`
- PR target: `sprint/d-5-otlp-signal-model`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-otlp/src/config.rs`
  - `crates/sc-observability-otlp/src/constants.rs`

## Deliverables

1. Implement the lifecycle core behind the D.12 exporter trait: state transitions, bounded admission, factory use, health/accounting, and adapter injection points.
2. Convert D.12 neutral signals at the core boundary without loss of resource/scope metadata, flags, links, events, status, or histogram content.
3. Implement lifecycle ordering, cancellation, fail-open health/dropped behavior, and fake-exporter fixtures using the D.12 bounds, retry policy, configuration fields, `ExporterSet`, and factory contract.
4. Document and test the lifecycle implementation and its backend-neutral async completion behavior.

## Non-closure

D.12 owns the types, factory, `ExporterSet`, and fake fixture contract; D.7/D.8 own transport adapters; D.18 owns public API integration.


## Design

Contract: obs-d-12 design, sections "Backend and trait contract" and "D.6-owned validated transport contract".

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
    pub async fn flush_async_typed(&self) -> Result<(), FlushError>;
    pub async fn shutdown_async_typed(&self) -> Result<(), ShutdownError>;
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


## Owned Paths and Exact Targets

- `crates/sc-observability-otlp/**`
- `crates/sc-observability-types/**`
- `Cargo.toml`
- `Cargo.lock`
- `release/public-api-policy.json`
- `docs/migration.md`
- `scripts/ci/validate_dependency_bans.sh`
- `scripts/ci/validate_repo_boundaries.sh`
- `docs/api-approvals/d-6-*.json`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

## Implementation targets


- `crates/sc-observability-otlp/src/config.rs`: implement `ValidatedTransportBounds::try_from_config` and lifecycle bounds (deliverable 2).
- `crates/sc-observability-otlp/src/constants.rs`: centralize validated defaults and stable codes (deliverable 3).

## Acceptance criteria

## D.6 validation fixtures

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
- An awaited final-export failure is surfaced as `ShutdownError`, and the
  host can immediately tear down its runtime after the await without loss.
- Premature runtime teardown produces `RuntimeTerminated`, accounts pending
  records as dropped, and never reports successful completion.
- Shutdown, including its final flush, drops every incomplete started span,
  increments the dropped export accounting once per span, and never passes an
  incomplete span to either backend (OTLP-009).
- Two telemetry instances remain isolated; enabled SDK config cannot resolve
  to no-op; disabled config makes no request; credentials never enter errors.
- Construction fixtures cover unsupported insecure verification, unreadable CA,
  invalid auth-header construction, SDK/provider builder failure, and legacy
  worker/client initialization; each yields the exact D.6 construction-only
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


