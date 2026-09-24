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

“Synchronous” means the caller owns no Tokio runtime and calls from a normal
blocking thread. It does **not** mean the dependency graph contains no Tokio:
`reqwest::blocking` uses an internal Tokio runtime. Calling this backend from a
thread currently entered into a Tokio runtime is unsupported and must return a
typed `BlockingBackendInAsyncContext` error before invoking reqwest; Tokio hosts
use D.7b instead or explicitly move work to a plain dedicated blocking thread.
Exporter construction and drop perform no request/blocking wait and are
supported in either context; dedicated current-thread and multi-thread tests
must pin that reqwest-version-specific behavior. If that assumption fails at
implementation time, keep D.7c open and isolate client construction/use/drop
on one plain owned worker rather than allowing a nested-runtime panic.

Authoritative source evidence is commit
`7b39f4e7f72b6845edec4eab4cd671611661445f`, path
`crates/sc-observability-otlp/src/lib.rs`, from the read-only legacy repository.
The transient scratchpad path is not part of the implementation contract.

## Retained implementation contract

```rust
pub struct OtlpHttpExporter { /* reqwest::blocking client + current config */ }

impl OtlpHttpExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportFailure>;
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportFailure>;
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportFailure>;
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
`Handle::try_current` context detection; it does not create or own a runtime.
The feature/dependency evidence must explicitly show
reqwest's transitive Tokio/hyper/rustls graph and the absence of
`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, and tonic in the
legacy-only build.

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait used by D.7b, and its backend state
implements the separate common `ExporterLifecycle`. Signal methods perform the
copied blocking request outside telemetry locks. `flush_blocking` and
`shutdown_blocking` return the real terminal result; the async lifecycle methods
return an immediately-ready future for that same completed result. Flush has no
hidden queue to drain and shutdown closes admission idempotently. Backend choice
remains construction/injection; no legacy branch is added to
`Telemetry::emit_*`, flush, or shutdown.

## Deliverables

1. Copy the exporter implementation and its `/v1/logs`, `/v1/traces`,
   `/v1/metrics`, authorization, CA, retry, and payload tests into this crate.
2. Commit a source-to-destination matrix naming every copied symbol/test and
   every changed, omitted, or newly wrapped behavior with its exact current-API
   incompatibility rationale. No transport/client/retry redesign is permitted.
3. Adapt inputs to current `TelemetryConfig`, D.7a signal types, diagnostic
   errors, D.7b backend selector, and common crate-private exporter traits while
   preserving the original HTTP/JSON behavior and current
   health/dropped-count facade contract.
4. Ensure blocking export/retry occurs outside the telemetry state lock and is
   safe from plain blocking threads. Detect/reject use from an entered Tokio
   runtime before reqwest can panic. Credentials remain redacted.
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
- Construction/use/teardown fixtures prove plain-thread support and typed
  rejection when called from current-thread and multi-thread Tokio contexts;
  no reqwest nested-runtime panic escapes.
- `Telemetry` uses the same trait-object call sites for both backends; only
  construction/injection selects the synchronous implementations.
- Any behavior/assertion not copied is identified by a concrete API
  incompatibility; structural rewrite or alternate HTTP/retry logic fails QA.
- Failures remain fail-open at the facade and update health/dropped counts.

## Required validation

- Copied legacy unit/loopback tests plus current public facade fixtures.
- `cargo test -p sc-observability-otlp --features legacy-http-json --locked`.
- A plain synchronous external-consumer fixture, Tokio-context rejection
  fixtures, workspace tests/clippy/rustdoc, and review of the committed
  source-transplant matrix.
- `cargo tree -p sc-observability-otlp -e features` under the legacy-only
  feature, with assertions that the acknowledged reqwest-internal Tokio is
  present while official OTel SDK and tonic crates are absent.

## Non-closure

No SDK changes beyond consuming D.7b's selector, no rewrite, no dashboards,
no Python binding change (the binding is a motivating consumer only), and no
publication.
