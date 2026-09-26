# d-8: Legacy HTTP/JSON source transplant

Generated projection of `obs-d-8`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 11
- Assignee / model: lobs / luna
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: OTLP HTTP JSON module
- Branch: `sprint/d-8-otlp-http-json-transplant`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-8-otlp-http-json-transplant`
- PR target (merge order only): `sprint/d-7-otlp-sdk-tokio`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-004, LAY-005, LAY-006, NFR-001, NFR-004, NFR-005, NFR-006, NFR-007, NFR-009, OTLP-001, OTLP-002, OTLP-003, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-012, OTLP-013, OTLP-014, OTLP-015, OTLP-016, OTLP-017, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, OTLP-023, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-021, TYP-023, TYP-024, TYP-030, TYP-031
- ADRs: ADR-002, ADR-004, ADR-005, ADR-006, ADR-008, ADR-009, ADR-017, ADR-018
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/mod.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`
  - `docs/plans/phase-d/sprint-d-8-otlp-http-json-transplant.md`
  - `examples/otlp-legacy/src/**`

## Goal

Transplant the immutable 7b39f4e7f72b6845edec4eab4cd671611661445f HTTP/JSON exporter into the legacy adapter boundary, parallel with D.7. A plain owned worker exclusively constructs/uses/drops reqwest; no caller runtime is required.

## Deliverables

1. Copy the transport and endpoint/auth/CA/payload tests identified by the existing legacy-otlp-provenance.json into legacy_http_json/implementation.rs and tests.rs; keep exact source/blob provenance readable to transplant QA.

2. Preserve copied HTTP/client behavior and document only the four authorized safety deltas in module docs and tests: retry classification, bounded server pacing/jitter, shutdown cancellation and overall retry deadline. No additional provenance matrix or JSON is required.

3. Adapt neutral D.12 signals, configuration and crate-private exporter interfaces without adding error variants or changing the single failure mapping.

4. Implement bounded record/byte admission, capacity-one control saturation behavior, ordered barrier completion, worker panic/exit propagation, finite construction handshake and drop-without-shutdown accounting as specified below.

5. Add plain-thread, entered-Tokio rejection, async responsiveness, request/backoff cancellation, retry bounds and response-loss accounting tests; add examples/otlp-legacy source using the D.12 contract fixture.

6. Run existing dependency/boundary validation and prove the legacy-only build excludes the official SDK/tonic while acknowledging reqwest's internal Tokio graph; consume D.12's pins unchanged.

## This Sprint Does Not Close

No SDK implementation, normative/manifest/registry edit, additional validator framework, Python binding implementation or publication. D.18 composes real backends; D.9 owns collector/dashboard qualification.

## Design

## Retained implementation contract

```rust
pub(crate) struct OtlpHttpExporter {
    commands: SyncSender<LegacyCommand>,
    completion: Arc<LegacyCompletionState>,
}

impl OtlpHttpExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}

// Private worker-thread-owned value; never crosses to a caller thread.
struct LegacyHttpWorker {
    client: reqwest::blocking::Client,
    receiver: Receiver<LegacyCommand>,
}
```

Retain legacy endpoint normalization, request assembly, HTTP client/auth/CA
configuration, per-request timeout, attempt limit, capped exponential-backoff
shape, and loopback request tests. Commit `7b39f4e` retries every transport
error and every non-success HTTP status, sleeps the whole backoff on the worker
thread, has no `Retry-After` handling or jitter, and has no sequence-wide
deadline or shutdown cancellation. The following bounded safety corrections
are authorized deltas—not claims about the legacy implementation:

| Legacy behavior | Authorized transplant delta | Existing manifest disposition |
| --- | --- | --- |
| retry every non-success status | retry connection errors, 408, 429, and 5xx; terminate other 4xx | `changed: retry classification` |
| capped exponential delay only | honor bounded `Retry-After`; otherwise add per-instance-seeded bounded jitter (deterministic under an injected test seed), consuming D.12's independently capped server/fallback paths | `changed: server pacing/jitter and independent caps` |
| `thread::sleep` cannot be interrupted | use worker-owned cancelable wait woken by shutdown | `changed: shutdown cancellation` |
| per-request timeout but no overall bound | add finite sequence deadline covering attempts and waits | `changed: retry deadline` |

The no-redesign rule applies to payload encoding, endpoints, client/auth/CA
construction, request execution, maximum attempts, and the exponential/cap
algorithm. It explicitly carves out only the four safety deltas above. The
existing immutable provenance manifest must name each delta and preserve copied tests
alongside new delta-specific fixtures.

D.8 consumes the validated legacy payload defined by D.12. D.12 is
authoritative for every raw field, default, applicability rule, checked type,
ordering rule, config error, origin metadata, and shared validation fixture;
D.8 does not redefine them.

Delta-seconds and HTTP-date `Retry-After` values are parsed at most up to 128
header bytes; raw values are never retained. Invalid values produce only the
bounded categories `negative`, `malformed`, `past`, `oversized`, or `clamped`.

Production jitter is seeded independently per exporter from OS-backed entropy
so instances do not synchronize against a recovering collector. The retry
component accepts a crate-private injected seed/source for deterministic tests;
the seed is neither public configuration nor serialized evidence.

Consume the legacy-http-json dependency/version/feature allowlist from obs-d-12 design and ADR-018/architecture section 6; no manifest, allowlist or normative-doc edit belongs to D.8.

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait used by D.12, and its backend state
implements the separate common `ExporterLifecycle`. Signal methods clone and
enqueue owned batches; the worker executes the copied blocking request/retry
code outside telemetry locks. `flush_blocking`/`shutdown_blocking` wait for the
ordered barrier's real terminal result on plain callers; async lifecycle awaits
the same result. Backend choice remains construction/injection; no legacy
branch is added to `Telemetry::emit_*`, flush, or shutdown.

Legacy commands use the D.12 lifecycle core. A bounded data `sync_channel` and
a separate capacity-one control `sync_channel` feed the same worker; it drains
control with `try_recv` before waiting briefly for data. Thus flush/shutdown
barriers cannot be starved by saturated admission. There is no second lifecycle
state machine. Signal admission uses `try_send`:
full/closed fails open, records exactly one per-signal drop and health change,
and never waits. Barriers have a reserved control path so saturated data cannot
starve them; async waiters use a Tokio `oneshot` completed by the plain worker,
never a blocking receive on an executor. Worker/client initialization failure
returns D.12's transport-construction failure before the handle is published.
After successful initialization, panic, unexpected exit, or sender closure
stores its worker-termination outcome, resolves every pending barrier, accounts
abandoned admissions once, and never hangs.

Legacy uses the authoritative D.12 health/accounting contract without adding
fields or transitions. The legacy worker consumes D.12's shared fixtures
unchanged, including redaction coverage.

Each request/retry sequence has a finite overall deadline. Shutdown enters
`Closing`, cancels retry backoff, and drains only work before its barrier.
Connection errors, 408, 429, and 5xx are retryable; other 4xx are terminal.
For every retry, choose exactly one delay path and apply either the validated
D.12 fallback cap or its independent server-delay cap, then the remaining
sequence budget. D.8 consumes the cap semantics and ordering frozen by the
D.12 matrix/fixtures without redefining them. If no positive budget remains,
return the D.12 retry-deadline outcome without sleeping or issuing a
zero-budget attempt. No request, backoff, barrier, join, or client drop is
unbounded.

Shutdown cancellation of a pre-barrier retry sequence returns the D.12
shutdown-cancelled-retry outcome: account that admitted batch as dropped
exactly once, apply D.12 terminal-health accounting, and return the failure from the
first/in-flight shutdown completion. Cancelable backoff wakes immediately. A
currently blocking reqwest call cannot be interrupted, but its remaining wait
is bounded by the D.12 request and sequence budgets; public shutdown is
bounded by the D.12 lifecycle budget or returns its lifecycle-timeout
outcome. All timing bounds arrive already validated; D.8 performs no second
validation.

Delivery is **at least once across retries**: a collector may accept an
attempt whose response is lost, after which the identical batch is retried and
observed twice. The exporter supplies no idempotency key and does not promise
deduplication; collectors must aggregate Delta metrics according to their
temporality and must not infer exactly-once delivery. Attempts are recorded
separately, while an admitted batch is
counted as dropped exactly once only if the sequence exhausts/terminates; a
successful retry is not a drop. Transient, terminal, and recovery accounting
follow the D.12 health contract without additional D.8 fields or
transitions.


## Failure contract

D.8 uses the complete D.12 stable-failure table. Runtime paths return its
named outcomes through the owning types and façade mappings defined there;
D.8 owns no error variant, stable code, mapping, or error documentation.

## Saturation, construction and drop bounds

The capacity-one control channel stores the earliest admitted barrier. A flush arriving while it is occupied joins that barrier if its sequence covers the caller's admission; otherwise it records the maximum pending sequence in one fixed-size shared slot and awaits the next barrier under the same finite deadline. Shutdown atomically closes admission, upgrades the pending control intent to shutdown, wakes retry backoff, and shares one completion with all shutdown callers. No blocking control send, unbounded waiter queue or independent lifecycle state machine is allowed; timed-out observers detach without cancelling the native command.

Construction handshake uses the validated request timeout, capped by the lifecycle shutdown bound. A failed/timed-out handshake cancels startup and returns InitError::Runtime with the ConfigFailure::TransportConstructionFailed source; the plain worker is responsible for client disposal and bounded termination. No live exporter handle is published until readiness succeeds.

Drop without explicit shutdown closes admission and signals the existing worker cancellation path nonblockingly. All admitted but unfinished records consume the same shared record/byte budgets and are counted exactly once as dropped if they cannot finish by the existing shutdown deadline; the in-flight request cannot outlive its request timeout. Remaining barrier observers resolve WorkerTerminated, counters remain visible through shared health until the last observer releases it, and client destruction occurs on the worker. Explicit shutdown remains the only success/completion proof. Test final handle drop on Tokio and on a plain thread.

## Existing provenance consumer

OTLP-023 still requires the existing immutable docs/plans/phase-d/legacy-otlp-provenance.json. D.8 reads it as source authority; D.9 consumes it to validate restored documentation. The existing manifest's consumer is transplant QA/source-blob verification, gating OTLP-023 against the observed loss of the legacy exporter. Retain it for as long as transplanted code is maintained. No additional provenance JSON/matrix, line-count gate, graph artifact, or validate_log_import.py extension is created. Add source-pin/disposition assertions to the owned adapter tests; use existing dependency/boundary checks only for actual forbidden edges.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-12 (wave 1)

Created by obs-d-12, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
- `crates/sc-observability-otlp/src/legacy_http_json/mod.rs`
- `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`

## Acceptance criteria

## Acceptance criteria

- Every relevant legacy implementation symbol and test has a disposition; all
  copied tests execute in this repository against the transplanted code.
- Captured requests preserve exact signal endpoints, content type, auth, CA,
  timeout, and retry behavior while carrying D.5's current neutral fields.
- The synchronous backend works from a plain thread without a caller-owned
  Tokio runtime and never silently falls back to no-op when enabled.
- Plain-thread construction plus sync flush/shutdown return real results.
  Construction and synchronous lifecycle from current-thread/multi-thread
  Tokio reject before worker creation, buffer drain, or command admission.
  The construction fixture asserts the D.12 wrapped construction form and
  redacted blocking-context diagnostic source; lifecycle fixtures assert its
  direct runtime-failure form.
- Async lifecycle from Tokio remains responsive while the plain worker performs
  transport; dropping the final exporter handle on Tokio merely closes the
  sender, and instrumentation proves the reqwest client is ultimately dropped
  and the worker exits on its plain thread. No nested-runtime panic escapes.
- `Telemetry` uses the same trait-object call sites for both backends; only
  construction/injection selects the synchronous implementations.
- Any behavior/assertion not copied is identified by a concrete API
  incompatibility; structural rewrite or alternate HTTP/retry logic beyond the
  four authorized safety deltas fails QA.
- Failures remain fail-open at the facade and update health/dropped counts.
- Legacy health satisfies the complete D.12 health/accounting contract
  equivalently to SDK, without adding or restating fields.
- Capacity-one saturation cannot block or starve flush/shutdown; injected
  worker panic/exit resolves every sync/async waiter within the deadline.
- Retry fixtures freeze classification, bounded `Retry-After`, jitter/backoff,
  overall deadline, and prompt shutdown cancellation.
- Retry bound fixtures cover delta-seconds and HTTP-date, negative, malformed,
  past, and huge `Retry-After` values; distinct production instance seeds;
  deterministic injected seeds; and
  positive jitter at the sequence deadline proving the D.12 fallback clamp
  and no zero-budget attempt. The shared cap-ordering fixture remains owned by
  D.12 and is consumed unchanged.
- Shutdown-during-backoff asserts the D.12 cancellation outcome, its exact
  accounting/facade mapping, and prompt wake.
- Shutdown-during-request has three fixtures: a successful in-flight response
  completes with no drop; an ordinary terminal response fails/drops once with
  its D.12 terminal outcome; a retryable response becomes the D.12
  cancellation outcome and drops once. Every case is accounted exactly once
  and finishes within the D.12 lifecycle contract.
- A response-loss fixture proves at-least-once duplicate delivery, separate
  attempt accounting, one terminal drop after exhaustion, and zero drops after
  a retried-then-successful batch.


## Required validation

- Copied legacy unit/loopback tests plus current public facade fixtures.
- `cargo test -p sc-observability-otlp --features legacy-http-json --locked`.
- A plain synchronous external-consumer fixture; current-thread/multi-thread
  construction and sync-lifecycle rejection; async barrier responsiveness;
  cross-context final-handle drop/worker-exit tests; workspace
  tests/clippy/rustdoc; and review against the existing immutable source manifest.
- Existing boundary/dependency checks and the owned adapter source-pin/disposition tests pass; no line-count or parallel provenance gate.


- `cargo test -p sc-observability-otlp --lib legacy_http_json::tests --features legacy-http-json --locked` executes all adapter cases above.
- This sprint does not close production composition or collector equivalence; D.18/D.9 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
