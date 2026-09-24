---
id: D.7c
status: complete
branch: feature/phase-d-7c-otlp-http-json-transplant
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-7c-otlp-http-json-transplant
depends_on: [D.7b]
relation: must_follow
owned_docs: [docs/requirements.md, docs/architecture.md, docs/api-design.md, docs/migrate-error-api.md, docs/plans/phase-d/legacy-otlp-provenance.json]
release_train: '2.0'
recommended_agent: rust-developer
recommended_model: deep-reasoning
---

# D.7c — Legacy HTTP/JSON source transplant

## Goal and dependency

Make `ExporterBackend::LegacyHttpJson` operational by transplanting—not
rewriting—the tested synchronous exporter and its tests from
`agent-team-mail`. D.7c `must_follow`s D.7b because both touch the same config,
crate dependencies, facade, and exporter ownership boundary.

`sc-observability-py` is the concrete in-repository motivating consumer: a
simple Python application should not need to host Tokio or pull in the full
SDK/tonic/protobuf stack merely to send OTLP records. The synchronous route is
also already proven by the legacy implementation. There is no historical ADR
that makes it the only route, so it coexists with—not replaces—the SDK backend.

“Synchronous” means a consumer needs no caller-owned Tokio runtime. It does
**not** mean the dependency graph contains no Tokio: `reqwest::blocking` uses
an internal Tokio runtime and its client construction, request waits, and final
drop cannot safely be owned by a Tokio executor thread.

This sprint therefore fixes one ownership model now: one plain owned worker
thread constructs, exclusively owns, uses, and finally drops the blocking
reqwest client. Public exporter handles contain only a bounded command sender
and shared completion state—never the reqwest client or its worker join. Plain
synchronous lifecycle waits for an ordered worker barrier. Async lifecycle
awaits the same barrier receiver without blocking an executor. Final handle
`Drop` is nonblocking in every context: it closes its sender; after draining
already-admitted commands, the worker drops the client on its own plain thread
and exits. No caller directly creates or drops a reqwest blocking client.

Construction synchronously waits for worker/client initialization and is
supported only from a plain thread. If a Tokio runtime is entered, construction
returns `BlockingBackendInAsyncContext` before spawning the worker or calling
reqwest. Likewise, synchronous flush/shutdown preflight the calling context
before removing buffered records or sending commands and return that typed
error on Tokio. The nonblocking signal-admission methods and async lifecycle
may be used from either context because all blocking transport work stays on
the owned worker. Tokio-first hosts should normally select D.7b.

Authoritative source evidence is commit
`7b39f4e7f72b6845edec4eab4cd671611661445f`, path
`crates/sc-observability-otlp/src/lib.rs`, from the read-only legacy repository.
The transient scratchpad path is not part of the implementation contract.
The complete immutable source/blob/destination and documentation disposition
is [`legacy-otlp-provenance.json`](legacy-otlp-provenance.json); that manifest,
not a local clone path, is the transplant authority.

## Retained implementation contract

```rust
pub(crate) struct OtlpHttpExporter {
    commands: SyncSender<LegacyCommand>,
    completion: Arc<LegacyCompletionState>,
}

impl OtlpHttpExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportFailure>;
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportFailure>;
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportFailure>;
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

| Legacy behavior | Authorized transplant delta | Required matrix disposition |
| --- | --- | --- |
| retry every non-success status | retry connection errors, 408, 429, and 5xx; terminate other 4xx | `changed: retry classification` |
| capped exponential delay only | honor bounded `Retry-After`; otherwise add per-instance-seeded bounded jitter (deterministic under an injected test seed) | `changed: server pacing/jitter` |
| `thread::sleep` cannot be interrupted | use worker-owned cancelable wait woken by shutdown | `changed: shutdown cancellation` |
| per-request timeout but no overall bound | add finite sequence deadline covering attempts and waits | `changed: retry deadline` |

The no-redesign rule applies to payload encoding, endpoints, client/auth/CA
construction, request execution, maximum attempts, and the exponential/cap
algorithm. It explicitly carves out only the four safety deltas above. The
source-to-destination matrix must name each delta and preserve copied tests
alongside new delta-specific fixtures.

The authorized retry configuration adds optional flat wire fields
`OtelConfig::retry_sequence_timeout_ms`,
`OtelConfig::retry_after_cap_ms`, and `OtelConfig::retry_jitter_percent`.
For `LegacyHttpJson`, absence resolves defaults `30_000`, `5_000`, and `20`
(a symmetric ±20% interval around the capped exponential delay). Supplying any
of these fields for `OpenTelemetrySdk` returns `ConfigFieldNotApplicable`; the
SDK never ignores them. `ValidatedTransportBounds::try_from_config` alone
validates `timeout_ms > 0`,
`retry_sequence_timeout_ms >= timeout_ms`, `0 < retry_after_cap_ms <=
retry_sequence_timeout_ms`, and `retry_jitter_percent <= 100`, with checked
duration arithmetic. Delta-seconds and HTTP-date `Retry-After` values are
supported and clamped to the cap/remaining sequence deadline. Negative,
malformed, or past-date values are ignored with a bounded categorical attempt
diagnostic and use the jittered exponential fallback; huge values clamp rather
than overflow. Parse at most 128 header bytes and never retain the raw value;
diagnostics contain only `negative`, `malformed`, `past`, `oversized`, or
`clamped`. If the clamped delay consumes the remaining sequence budget,
terminate as `RetryDeadlineExhausted` without sleeping or making a zero-budget
attempt.

Production jitter is seeded independently per exporter from OS-backed entropy
so instances do not synchronize against a recovering collector. The retry
component accepts a crate-private injected seed/source for deterministic tests;
the seed is neither public configuration nor serialized evidence.

Pin the transplanted client to the legacy tested selection
`reqwest = "=0.12.28"` with `default-features = false` and features
`["blocking", "json", "rustls-tls"]`, subject only to a separately reviewed
security update. Add a minimal optional direct Tokio `rt` feature solely for
construction/synchronous-lifecycle context preflight; it does not create or
own a runtime. The feature/dependency evidence must explicitly show
reqwest's transitive Tokio/hyper/rustls graph and the absence of
`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, and tonic in the
legacy-only build.
Pin `httpdate = "=1.0.3"` for RFC 7231 HTTP-date `Retry-After` parsing, record
its license/dependency disposition, and keep it inside the legacy-only feature.

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait used by D.7b, and its backend state
implements the separate common `ExporterLifecycle`. Signal methods clone and
enqueue owned batches; the worker executes the copied blocking request/retry
code outside telemetry locks. `flush_blocking`/`shutdown_blocking` wait for the
ordered barrier's real terminal result on plain callers; async lifecycle awaits
the same result. Backend choice remains construction/injection; no legacy
branch is added to `Telemetry::emit_*`, flush, or shutdown.

Legacy commands use the D.7b-L lifecycle core and one bounded `sync_channel`;
there is no second lifecycle state machine. Signal admission uses `try_send`:
full/closed fails open, records exactly one per-signal drop and health change,
and never waits. Barriers have a reserved control path so saturated data cannot
starve them; async waiters use a Tokio `oneshot` completed by the plain worker,
never a blocking receive on an executor. Initialization failure, panic,
unexpected exit, or sender closure stores one terminal `WorkerTerminated`
result, resolves every pending barrier, accounts abandoned admissions once,
and never hangs.

Legacy and SDK health use one field model: queue depth/capacity,
worker/exporter state, `last_terminal_failure`, per-signal overflow/drop counts,
and a `retry_attempt_failures` counter. Transient failed attempts increment the
counter but never overwrite `last_terminal_failure`. Exhaustion, admission
failure, worker/provider death, or lifecycle failure writes the terminal field.
The next successfully exported batch while `Open` clears it and records
recovery; `Closing`/`Shutdown` retains it. No field contains credentials.

Each request/retry sequence has a finite overall deadline. Shutdown enters
`Closing`, cancels retry backoff, and drains only work before its barrier.
Connection errors, 408, 429, and 5xx are retryable; other 4xx are terminal.
Honor bounded `Retry-After`, otherwise use capped exponential backoff with
per-instance-seeded bounded jitter (deterministic under an injected test seed).
No request, backoff, barrier, join, or client
drop is unbounded.

Shutdown cancellation of a pre-barrier retry sequence is terminal
`ShutdownCancelledRetry`: account that admitted batch as dropped exactly once,
set degraded health/`last_terminal_failure`, and return the failure from the
first/in-flight shutdown completion. Cancelable backoff wakes immediately. A
currently blocking reqwest call cannot be interrupted, but its remaining wait
is at most `min(timeout_ms, remaining retry_sequence_timeout_ms)`; the public
shutdown call still returns no later than `lifecycle_shutdown_timeout_ms`, or
returns `LifecycleTimeout`. Cross-field validation requires the lifecycle
shutdown bound to be at least `timeout_ms`.

Delivery is **at least once across retries**: a collector may accept an
attempt whose response is lost, after which the identical batch is retried and
observed twice. The exporter supplies no idempotency key and does not promise
deduplication. Attempts are recorded separately, while an admitted batch is
counted as dropped exactly once only if the sequence exhausts/terminates; a
successful retry is not a drop. Transient attempts affect only
`retry_attempt_failures`; terminal exhaustion writes `last_terminal_failure`,
which the next successful open-state batch clears.

## Stable failure inventory

The D.7b-L inventory and `migrate-error-api.md` must contain these exact D.7c
codes; diagnostics are bounded/redacted and preserve their typed source:

| Variant | Stable code | Cause | Recovery |
| --- | --- | --- | --- |
| `BlockingBackendInAsyncContext` | `OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT` | legacy construction/sync lifecycle entered Tokio | construct/call on a plain thread or select SDK/async lifecycle |
| `WorkerTerminated` | `OTLP_WORKER_TERMINATED` | init failure, panic, channel closure, or unexpected exit | create a new telemetry instance after correcting the worker cause |
| `ShutdownCancelledRetry` | `OTLP_SHUTDOWN_CANCELLED_RETRY` | shutdown cancels a retryable pre-barrier sequence | inspect terminal health; resend only if duplicates are acceptable |
| `RetryDeadlineExhausted` | `OTLP_RETRY_DEADLINE_EXHAUSTED` | no sequence budget remains for another delay/attempt | increase the validated sequence bound or restore collector health |
| `LifecycleTimeout` | `OTLP_LIFECYCLE_TIMEOUT` | flush/shutdown exceeds its monotonic deadline | preserve terminal evidence and investigate stuck transport/provider |
| `ZeroDuration` | `OTLP_CONFIG_ZERO_DURATION` | a required duration is zero | provide a positive value |
| `DurationOverflow` | `OTLP_CONFIG_DURATION_OVERFLOW` | raw milliseconds cannot convert safely | reduce the field to the documented range |
| `InvalidBoundOrdering` | `OTLP_CONFIG_BOUND_ORDER` | cap/sequence/request/lifecycle ordering is invalid | satisfy the documented cross-field inequalities |
| `InvalidJitterPercent` | `OTLP_CONFIG_JITTER_PERCENT` | jitter exceeds 100 | use `0..=100` |
| `ConfigFieldNotApplicable` | `OTLP_CONFIG_FIELD_NOT_APPLICABLE` | a legacy-only retry field is present for SDK | omit it or select the legacy backend |

## Deliverables

1. Copy the exporter implementation and its `/v1/logs`, `/v1/traces`,
   `/v1/metrics`, authorization, CA, retry, and payload tests into this crate.
2. Commit a source-to-destination matrix naming every copied symbol/test and
   every changed, omitted, or newly wrapped behavior with its exact current-API
   incompatibility rationale. The worker is an ownership/context adapter around
   copied transport code; no HTTP/client configuration/retry redesign beyond
   the four enumerated safety deltas is permitted.
   Extend `scripts/ci/validate_log_import.py` to validate the Phase D manifest,
   Git blob ids and disposition-aware destination evidence. Planned import
   hashes are null until files land and become mandatory before closure;
   the parent module and each split child file have independent hashes;
   reference/dependency-only rows keep null hashes and validate their cited
   disposition rather than byte identity. Reject scratch/`/tmp` paths, ATM
   imports/labels, or the stale repository name in imported outputs.
3. Adapt inputs to current `TelemetryConfig`, D.7a signal types, diagnostic
   errors, D.7b backend selector, and common crate-private exporter traits while
   preserving the original HTTP/JSON behavior and current
   health/dropped-count facade contract.
4. Implement bounded command admission, ordered barrier completion, and the
   exact construction/use/drop contract above. The worker alone constructs,
   calls, and drops reqwest outside telemetry locks. Context preflight occurs
   before mutation; credentials remain redacted.
   Reuse D.7b-L's state/deadline/accounting core and implement the reserved
   control path, retry cancellation, worker-panic propagation, and oneshot
   async barrier described above.
5. Add public external-consumer-style construction/flush/shutdown proof with
   no caller-owned Tokio runtime and no official OTel SDK/tonic dependencies.
6. Retain exact reqwest features/version and commit `cargo tree` evidence for
   the legacy-only feature graph, including its acknowledged transitive Tokio.
7. Split transplanted source into bounded `worker`, `request`, `retry`, and
   `payload` modules plus tests; enforce the repository line-count check.
   Update both dependency-boundary scripts and architecture §6 with the exact
   legacy allowlist and automated graph assertions.
8. Update OTLP-020/config requirements, API design, rustdoc, and the migration
   guide with every flat field, backend applicability rule, default,
   `ValidatedTransportBounds` conversion, stable failure above, and validation
   inequality. D.7d docs consistency must compare all four owners.

## Acceptance criteria

- Every relevant legacy implementation symbol and test has a disposition; all
  copied tests execute in this repository against the transplanted code.
- Captured requests preserve exact signal endpoints, content type, auth, CA,
  timeout, and retry behavior while carrying D.7a's current neutral fields.
- The synchronous backend works from a plain thread without a caller-owned
  Tokio runtime and never silently falls back to no-op when enabled.
- Plain-thread construction plus sync flush/shutdown return real results.
  Construction and synchronous lifecycle from current-thread/multi-thread
  Tokio reject before worker creation, buffer drain, or command admission.
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
- Legacy health reports queue depth/capacity, worker/exporter state, last
  terminal failure, and per-signal overflow/drop counts equivalently to SDK.
- Capacity-one saturation cannot block or starve flush/shutdown; injected
  worker panic/exit resolves every sync/async waiter within the deadline.
- Retry fixtures freeze classification, bounded `Retry-After`, jitter/backoff,
  overall deadline, and prompt shutdown cancellation.
- Retry bound fixtures cover delta-seconds and HTTP-date, negative, malformed,
  past, and huge `Retry-After` values; zero/overflow/cross-field validation;
  distinct production instance seeds; and deterministic injected seeds.
- Shutdown-during-backoff asserts one `ShutdownCancelledRetry` drop, degraded
  terminal health, first-caller `ShutdownFailure`, and prompt wake.
- Shutdown-during-request has three fixtures: a successful in-flight response
  completes with no drop; an ordinary terminal response fails/drops once with
  its terminal code; a retryable response becomes `ShutdownCancelledRetry` and
  drops once. Every case is accounted exactly once and shutdown returns by
  `lifecycle_shutdown_timeout_ms` (or the typed lifecycle timeout).
- A response-loss fixture proves at-least-once duplicate delivery, separate
  attempt accounting, one terminal drop after exhaustion, and zero drops after
  a retried-then-successful batch.

## Required validation

- Copied legacy unit/loopback tests plus current public facade fixtures.
- `cargo test -p sc-observability-otlp --features legacy-http-json --locked`.
- A plain synchronous external-consumer fixture; current-thread/multi-thread
  construction and sync-lifecycle rejection; async barrier responsiveness;
  cross-context final-handle drop/worker-exit tests; workspace
  tests/clippy/rustdoc; and review of the source-transplant matrix.
- `cargo tree -p sc-observability-otlp -e features` under the legacy-only
  feature, with assertions that the acknowledged reqwest-internal Tokio is
  present while official OTel SDK and tonic crates are absent.
- Import-provenance validation and automated no-exporter/legacy-only/combined
  dependency graph gates; module line-count validation.

## Non-closure

No SDK changes beyond consuming D.7b's selector, no rewrite, no dashboards,
no Python binding change (the binding is a motivating consumer only), and no
publication.
