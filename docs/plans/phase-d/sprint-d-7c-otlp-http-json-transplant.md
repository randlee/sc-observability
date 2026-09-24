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

Each transplanted exporter implements the same crate-private `LogExporter`,
`TraceExporter`, or `MetricExporter` trait and `ExporterLifecycle` supertrait
used by D.7b. Its synchronous trait method performs the copied blocking request
outside telemetry locks; flush has no hidden queue to drain and shutdown closes
admission idempotently. Backend choice remains construction/injection; no
legacy branch is added to `Telemetry::emit_*`, flush, or shutdown.

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
   safe when called from non-Tokio consumers. Credentials remain redacted.
5. Add public external-consumer-style construction/flush/shutdown proof with
   no Tokio runtime dependency enabled for this backend.

## Acceptance criteria

- Every relevant legacy implementation symbol and test has a disposition; all
  copied tests execute in this repository against the transplanted code.
- Captured requests preserve exact signal endpoints, content type, auth, CA,
  timeout, and retry behavior while carrying D.7a's current neutral fields.
- The synchronous backend works without a Tokio runtime and never silently
  falls back to no-op when enabled.
- `Telemetry` uses the same trait-object call sites for both backends; only
  construction/injection selects the synchronous implementations.
- Any behavior/assertion not copied is identified by a concrete API
  incompatibility; structural rewrite or alternate HTTP/retry logic fails QA.
- Failures remain fail-open at the facade and update health/dropped counts.

## Required validation

- Copied legacy unit/loopback tests plus current public facade fixtures.
- `cargo test -p sc-observability-otlp --features legacy-http-json --locked`.
- A no-Tokio external-consumer fixture, workspace tests/clippy/rustdoc, and a
  review of the committed source-transplant matrix.

## Non-closure

No SDK changes beyond consuming D.7b's selector, no rewrite, no dashboards,
no Python binding change (the binding is a motivating consumer only), and no
publication.
