---
id: D.7c
status: proposed
branch: feature/phase-d-7c-otlp-http-json-transplant
base: develop
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

## Retained implementation contract

```rust
pub struct OtlpHttpExporter {
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
configuration, timeout, bounded exponential retry/backoff, and loopback request
tests. Only current-type/config adapters and necessary module extraction are
authorized changes.

Pin the transplanted client to the legacy tested selection
`reqwest = "=0.12.28"` with `default-features = false` and features
`["blocking", "json", "rustls-tls"]`, subject only to a separately reviewed
security update. Add a minimal optional direct Tokio `rt` feature solely for
construction/synchronous-lifecycle context preflight; it does not create or
own a runtime. The feature/dependency evidence must explicitly show
reqwest's transitive Tokio/hyper/rustls graph and the absence of
`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, and tonic in the
legacy-only build.

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait used by D.7b, and its backend state
implements the separate common `ExporterLifecycle`. Signal methods clone and
enqueue owned batches; the worker executes the copied blocking request/retry
code outside telemetry locks. `flush_blocking`/`shutdown_blocking` wait for the
ordered barrier's real terminal result on plain callers; async lifecycle awaits
the same result. Backend choice remains construction/injection; no legacy
branch is added to `Telemetry::emit_*`, flush, or shutdown.

## Deliverables

1. Copy the exporter implementation and its `/v1/logs`, `/v1/traces`,
   `/v1/metrics`, authorization, CA, retry, and payload tests into this crate.
2. Commit a source-to-destination matrix naming every copied symbol/test and
   every changed, omitted, or newly wrapped behavior with its exact current-API
   incompatibility rationale. The worker is an ownership/context adapter around
   copied transport code; no HTTP/client configuration/retry redesign is
   permitted.
3. Adapt inputs to current `TelemetryConfig`, D.7a signal types, diagnostic
   errors, D.7b backend selector, and common crate-private exporter traits while
   preserving the original HTTP/JSON behavior and current
   health/dropped-count facade contract.
4. Implement bounded command admission, ordered barrier completion, and the
   exact construction/use/drop contract above. The worker alone constructs,
   calls, and drops reqwest outside telemetry locks. Context preflight occurs
   before mutation; credentials remain redacted.
5. Add public external-consumer-style construction/flush/shutdown proof with
   no caller-owned Tokio runtime and no official OTel SDK/tonic dependencies.
6. Retain exact reqwest features/version and commit `cargo tree` evidence for
   the legacy-only feature graph, including its acknowledged transitive Tokio.

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
  incompatibility; structural rewrite or alternate HTTP/retry logic fails QA.
- Failures remain fail-open at the facade and update health/dropped counts.

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

## Non-closure

No SDK changes beyond consuming D.7b's selector, no rewrite, no dashboards,
no Python binding change (the binding is a motivating consumer only), and no
publication.
