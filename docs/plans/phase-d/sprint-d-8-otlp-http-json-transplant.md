---
id: D.8
status: planned
branch: sprint/d-8-otlp-http-json-transplant
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/sprint/d-8-otlp-http-json-transplant
depends_on: ["D.6", "D.7"]
relation: must_follow
assignee: aobs
model_class: astra
owned_docs: ["docs/architecture.md", "docs/plans/phase-d/legacy-otlp-provenance.json"]
release_train: "2.0"
requirements: ["OTLP-011", "OTLP-012", "OTLP-013", "OTLP-020", "OTLP-021", "OTLP-023"]
adrs: ["ADR-004", "ADR-018"]
closure_type: boundary
target_boundary: "legacy HTTP/JSON exporter adapter"
owned_paths: ["crates/sc-observability-otlp/**", "Cargo.toml", "Cargo.lock", "examples/otlp-legacy/**", "scripts/ci/validate_log_import.py", "scripts/ci/tests/test_validate_log_import.py", "scripts/ci/validate_dependency_bans.sh", "scripts/ci/validate_repo_boundaries.sh", "docs/architecture.md", "docs/plans/phase-d/legacy-otlp-provenance.json"]
---

# D.8 — Legacy HTTP/JSON source transplant

## Goal and dependency

Make `ExporterBackend::LegacyHttpJson` operational by transplanting—not
rewriting—the tested synchronous exporter and its tests from
`agent-team-mail`. D.8 `must_follow`s D.6 because both touch the same config,
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
returns the D.6 transport-construction failure whose redacted diagnostic
source is its blocking-backend-in-async-context outcome, before spawning the
worker or calling reqwest. Synchronous flush/shutdown instead return that
D.6 runtime outcome directly after preflighting the calling context and
before removing buffered records or sending commands. The nonblocking
signal-admission methods and async lifecycle may be used from either context
because all blocking transport work stays on the owned worker. Tokio-first
hosts should normally select D.6.

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

| Legacy behavior | Authorized transplant delta | Required matrix disposition |
| --- | --- | --- |
| retry every non-success status | retry connection errors, 408, 429, and 5xx; terminate other 4xx | `changed: retry classification` |
| capped exponential delay only | honor bounded `Retry-After`; otherwise add per-instance-seeded bounded jitter (deterministic under an injected test seed), consuming D.6's independently capped server/fallback paths | `changed: server pacing/jitter and independent caps` |
| `thread::sleep` cannot be interrupted | use worker-owned cancelable wait woken by shutdown | `changed: shutdown cancellation` |
| per-request timeout but no overall bound | add finite sequence deadline covering attempts and waits | `changed: retry deadline` |

The no-redesign rule applies to payload encoding, endpoints, client/auth/CA
construction, request execution, maximum attempts, and the exponential/cap
algorithm. It explicitly carves out only the four safety deltas above. The
source-to-destination matrix must name each delta and preserve copied tests
alongside new delta-specific fixtures.

D.8 consumes the validated legacy payload defined by D.6. D.6 is
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

Pin the transplanted client to the legacy tested selection
`reqwest = "=0.12.28"` with `default-features = false` and features
`["blocking", "json", "rustls-tls"]`, subject only to a separately reviewed
security update. Add a minimal optional direct Tokio dependency with only the
`rt` feature (`Handle::try_current` construction/synchronous-lifecycle context
preflight) and the `sync` feature (the async-waiter `oneshot`); it does not
create or own a runtime. The feature/dependency evidence must explicitly show
reqwest's transitive Tokio/hyper/rustls graph and the absence of
`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, and tonic in the
legacy-only build.
Pin `httpdate = "=1.0.3"` for RFC 7231 HTTP-date `Retry-After` parsing, record
its license/dependency disposition, and keep it inside the legacy-only feature.
The direct Tokio dependency enables only Tokio's `rt` and `sync` features; the
per-exporter jitter seed uses OS-backed `getrandom` entropy, with the injected
test source remaining crate-private.

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait used by D.6, and its backend state
implements the separate common `ExporterLifecycle`. Signal methods clone and
enqueue owned batches; the worker executes the copied blocking request/retry
code outside telemetry locks. `flush_blocking`/`shutdown_blocking` wait for the
ordered barrier's real terminal result on plain callers; async lifecycle awaits
the same result. Backend choice remains construction/injection; no legacy
branch is added to `Telemetry::emit_*`, flush, or shutdown.

Legacy commands use the D.6 lifecycle core. A bounded data `sync_channel` and
a separate capacity-one control `sync_channel` feed the same worker; it drains
control with `try_recv` before waiting briefly for data. Thus flush/shutdown
barriers cannot be starved by saturated admission. There is no second lifecycle
state machine. Signal admission uses `try_send`:
full/closed fails open, records exactly one per-signal drop and health change,
and never waits. Barriers have a reserved control path so saturated data cannot
starve them; async waiters use a Tokio `oneshot` completed by the plain worker,
never a blocking receive on an executor. Worker/client initialization failure
returns D.6's transport-construction failure before the handle is published.
After successful initialization, panic, unexpected exit, or sender closure
stores its worker-termination outcome, resolves every pending barrier, accounts
abandoned admissions once, and never hangs.

Legacy uses the authoritative D.6 health/accounting contract without adding
fields or transitions. The legacy worker consumes D.6's shared fixtures
unchanged, including redaction coverage.

Each request/retry sequence has a finite overall deadline. Shutdown enters
`Closing`, cancels retry backoff, and drains only work before its barrier.
Connection errors, 408, 429, and 5xx are retryable; other 4xx are terminal.
For every retry, choose exactly one delay path and apply either the validated
D.6 fallback cap or its independent server-delay cap, then the remaining
sequence budget. D.8 consumes the cap semantics and ordering frozen by the
D.6 matrix/fixtures without redefining them. If no positive budget remains,
return the D.6 retry-deadline outcome without sleeping or issuing a
zero-budget attempt. No request, backoff, barrier, join, or client drop is
unbounded.

Shutdown cancellation of a pre-barrier retry sequence returns the D.6
shutdown-cancelled-retry outcome: account that admitted batch as dropped
exactly once, apply D.6 terminal-health accounting, and return the failure from the
first/in-flight shutdown completion. Cancelable backoff wakes immediately. A
currently blocking reqwest call cannot be interrupted, but its remaining wait
is bounded by the D.6 request and sequence budgets; public shutdown is
bounded by the D.6 lifecycle budget or returns its lifecycle-timeout
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
follow the D.6 health contract without additional D.8 fields or
transitions.

## Failure contract

D.8 uses the complete D.6 stable-failure table. Runtime paths return its
named outcomes through the owning types and façade mappings defined there;
D.8 owns no error variant, stable code, mapping, or error documentation.

## Deliverables

1. Copy the exporter implementation and its `/v1/logs`, `/v1/traces`,
   `/v1/metrics`, authorization, CA, retry, and payload tests into this crate.
2. Commit a source-to-destination matrix naming every copied symbol/test and
   every changed, omitted, or newly wrapped behavior with its exact current-API
   incompatibility rationale. The worker is an ownership/context adapter around
   copied transport code; no HTTP/client configuration/retry redesign beyond
   the four enumerated safety deltas is permitted.
   Extend `scripts/ci/validate_log_import.py` to verify the manifest's pinned
   commit/blob/SHA and compare each transplant destination with its source blob.
   Permit only the named matrix deltas; this prevents a rewrite being presented
   as a transplant. Translation/reference rows validate only their disposition.
3. Adapt inputs to current `TelemetryConfig`, D.5 signal types, diagnostic
   errors, D.6 backend selector, and common crate-private exporter traits while
   preserving the original HTTP/JSON behavior and current
   health/dropped-count facade contract.
4. Implement bounded command admission, ordered barrier completion, and the
   exact construction/use/drop contract above. The worker alone constructs,
   calls, and drops reqwest outside telemetry locks. Context preflight occurs
   before mutation; credentials remain redacted.
   Reuse D.6's state/deadline/accounting core and implement the reserved
   control path, retry cancellation, worker-panic propagation, and oneshot
   async barrier described above.
5. Add public external-consumer-style construction/flush/shutdown proof with
   no caller-owned Tokio runtime and no official OTel SDK/tonic dependencies.
6. Record the exact reqwest features/version and legacy-only dependency
   boundary in architecture §6. D.8 owns legacy runtime/provenance and
   architecture-boundary documentation; it does not redefine D.6 contracts.
   Run the repository's existing `just lint` boundary check after the
   transplant; no separate identifier-scrub process is introduced.

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
  The construction fixture asserts the D.6 wrapped construction form and
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
- Legacy health satisfies the complete D.6 health/accounting contract
  equivalently to SDK, without adding or restating fields.
- Capacity-one saturation cannot block or starve flush/shutdown; injected
  worker panic/exit resolves every sync/async waiter within the deadline.
- Retry fixtures freeze classification, bounded `Retry-After`, jitter/backoff,
  overall deadline, and prompt shutdown cancellation.
- Retry bound fixtures cover delta-seconds and HTTP-date, negative, malformed,
  past, and huge `Retry-After` values; distinct production instance seeds;
  deterministic injected seeds; and
  positive jitter at the sequence deadline proving the D.6 fallback clamp
  and no zero-budget attempt. The shared cap-ordering fixture remains owned by
  D.6 and is consumed unchanged.
- Shutdown-during-backoff asserts the D.6 cancellation outcome, its exact
  accounting/facade mapping, and prompt wake.
- Shutdown-during-request has three fixtures: a successful in-flight response
  completes with no drop; an ordinary terminal response fails/drops once with
  its D.6 terminal outcome; a retryable response becomes the D.6
  cancellation outcome and drops once. Every case is accounted exactly once
  and finishes within the D.6 lifecycle contract.
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
- Import-provenance validation and automated no-exporter/legacy-only/combined
  dependency graph gates; module line-count validation.

## Owned Paths and Exact Targets

- `crates/sc-observability-otlp/**`
- `Cargo.toml`
- `Cargo.lock`
- `examples/otlp-legacy/**`
- `scripts/ci/validate_log_import.py`
- `scripts/ci/tests/test_validate_log_import.py`
- `scripts/ci/validate_dependency_bans.sh`
- `scripts/ci/validate_repo_boundaries.sh`
- `docs/architecture.md`
- `docs/plans/phase-d/legacy-otlp-provenance.json`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

Must also follow D.7 because both edit the OTLP crate manifest/factory, Cargo.lock, dependency allowlists, and docs/architecture.md. The two backend scopes stay distinct.

## Non-closure

No SDK changes beyond consuming D.6's selector, no rewrite, no dashboards,
no Python binding change (the binding is a motivating consumer only), and no
publication.
