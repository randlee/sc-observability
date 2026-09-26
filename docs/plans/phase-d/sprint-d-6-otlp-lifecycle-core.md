# d-6: OTLP lifecycle core

Generated projection of `obs-d-6`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 9
- Assignee / model: lobs / luna
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: OTLP lifecycle module
- Branch: `sprint/d-6-otlp-lifecycle-core`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-6-otlp-lifecycle-core`
- PR target (merge order only): `sprint/d-5-otlp-signal-model`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-004, LAY-005, LAY-006, NFR-001, NFR-004, NFR-005, NFR-006, NFR-007, NFR-009, OTLP-001, OTLP-002, OTLP-003, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-012, OTLP-013, OTLP-014, OTLP-015, OTLP-016, OTLP-017, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-021, TYP-023, TYP-024, TYP-030, TYP-031
- ADRs: ADR-002, ADR-004, ADR-005, ADR-009, ADR-012, ADR-017, ADR-018
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp/src/lifecycle.rs`
  - `crates/sc-observability-otlp/src/lifecycle_tests.rs`
  - `docs/plans/phase-d/sprint-d-6-otlp-lifecycle-core.md`

## Deliverables

1. Implement the lifecycle core behind the D.12 exporter trait: state transitions, bounded admission, factory use, health/accounting, and adapter injection points.
2. Convert D.12 neutral signals at the core boundary without loss of resource/scope metadata, flags, links, events, status, or histogram content.
3. Implement lifecycle ordering, cancellation, fail-open health/dropped behavior, and fake-exporter fixtures using the D.12 bounds, retry policy, configuration fields, `ExporterSet`, and factory contract.
4. Document and test the lifecycle implementation and its backend-neutral async completion behavior.

## This Sprint Does Not Close

D.12 owns the types, factory, `ExporterSet`, and fake fixture contract; D.7/D.8 own transport adapters; D.18 owns public API integration.

## Design

Contract: obs-d-12 design, sections "Backend and trait contract" and "Validated transport contract".

## 2.0 lifecycle decision

Synchronous `emit_*` remains admission-only for the SDK backend. Construction
requires an entered Tokio handle and fails before mutation outside a runtime.
D.7 owns the SDK provider/batch-processor dispatcher; this sprint implements only the shared lifecycle/admission contract and tests it with D.12 recording exporters. Trait calls clone owned batches and use nonblocking bounded
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
OTLP-021 contract; D.12 owns its normative requirement update and D.18 owns the migration guide.

## Implementation ownership

Implement lifecycle.rs/lifecycle_tests.rs against obs-d-12 contracts.rs/config.rs; define no exporter trait, config field, error variant, constant or registry. D.7 and D.8 independently implement the same already-frozen interface, then D.18 connects production lifecycle and exporters. Adapter builders must set every validated endpoint/header/timeout/batch parameter explicitly and must not read ambient OTEL_* environment over the validated configuration. A fixture sets conflicting OTEL_* values and proves they cannot change the factory's resolved contract. Config validation itself remains D.12.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-12 (wave 1)

Created by obs-d-12, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-otlp/src/lifecycle.rs`
- `crates/sc-observability-otlp/src/lifecycle_tests.rs`

## Acceptance criteria

- [ ] `cargo test -p sc-observability-otlp --lib lifecycle_tests --all-features --locked` runs barrier_order, repeated_shutdown, cancelled_waiter, request_deadline, runtime_terminated and exact_once_drop_count against D.12 recording exporters (D1/D3).
- [ ] boundary:OTLP lifecycle — resource/scope/flags/links/histogram payloads remain unchanged across admission, record and byte credit bounds are enforced, and failed admission is nonblocking (D2).
- [ ] Conflicting ambient OTEL_* settings cannot override explicitly validated config in the factory test; no block_on/second runtime or backend switch appears in shared emit/lifecycle dispatch (D4).
- [ ] This sprint does not close real SDK/legacy transport or collector behavior; D.7/D.8 and D.18/D.9 close those.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
