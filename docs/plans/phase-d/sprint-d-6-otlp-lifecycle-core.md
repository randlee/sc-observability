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



## Implementation targets

 implement or update the named contract consumer and its focused test for the corresponding numbered deliverable.\n

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


