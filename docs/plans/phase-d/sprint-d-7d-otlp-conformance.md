---
id: D.7d
status: proposed
branch: feature/phase-d-7d-otlp-conformance
base: develop
release_train: '2.0'
recommended_agent: rust-developer
recommended_model: deep-reasoning
---

# D.7d — Cross-path qualification and observability docs

## Goal and dependency

Qualify the complete restored OTLP surface and close the regression with
collector evidence and current-schema documentation. D.7d `must_follow`s D.7c;
D.7b is also a completion prerequisite. It adds no third transport.

## Shared conformance contract

```rust
struct OtlpConformanceCase {
    logs: Vec<LogEvent>,
    spans: Vec<CompleteSpan>,
    metrics: Vec<MetricRecord>,
}

fn assert_collector_semantics(
    backend: ExporterBackend,
    case: &OtlpConformanceCase,
    captured: &CollectorCapture,
);
```

The same logical fixture corpus must run through both production backends;
wire/protocol differences are allowed, signal meaning loss is not.

## Deliverables

1. Add a shared conformance corpus covering resources/scope, log severity/body/
   attributes, trace parent/kind/flags/links/events/status/timing, and counter,
   gauge, and zero/one/many-bucket histograms. The harness must also prove that
   both modes exercise identical crate-private exporter-trait call sites and
   differ only in construction/injection.
2. Run both backends against hermetic collectors and compare decoded semantic
   output. Add negative cases for disabled no-network, unsupported selections,
   invalid models, auth redaction, timeout, retry exhaustion, partial signal
   failure, recovery, flush, and idempotent shutdown.
3. Add CI jobs/features proving both backends from the same immutable source
   SHA, with exact commands, dependency features, collector versions, redacted
   receipts, and no hidden external service requirement.
4. Restore Grafana dashboard and LogQL/trace/metric recipes from legacy phases
   AV–AY only after translating them to current neutral resource/attribute
   schema. Record a disposition for each retained or omitted recipe; no stale
   ATM-only label is presented as a generic contract.
5. Finalize OTLP-001–022, architecture, migration guide, API approvals,
   dependency/license inventory, release notes, and operational docs for both
   backends and the no-enabled-noop rule.

## Acceptance criteria

- Both backends export all three signal families from the shared corpus and
  decoded collector output is semantically equivalent for every required field.
- Every lifecycle/failure negative case has observable health/dropped-count
  assertions and no credential leakage.
- CI retains complete, redacted, same-SHA receipts for both paths; feature
  isolation proves the synchronous path does not require Tokio.
- Restored dashboards/queries work against the current collector fixture and
  have a complete legacy-to-current disposition inventory.
- Requirements, architecture, API, migration, release, and operational docs
  agree; no enabled configuration is documented or implemented as no-op.

## Required validation

- Shared dual-backend conformance suite and negative matrix.
- Feature-isolated tests for `otlp-sdk` and `legacy-http-json`, plus combined
  feature tests.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public API/semver, docs consistency, dependency/license, and CI workflow
  validation.
- Local collector smoke commands for both backends at one exact source SHA.

## Non-closure

No `atm-core` implementation, Python OTEL binding (#88), registry publication,
or new transport beyond the two qualified paths.
