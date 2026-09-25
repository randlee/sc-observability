---
id: D.8
status: planned
branch: feature/phase-d-8-otlp-conformance
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-8-otlp-conformance
depends_on: [D.7]
relation: must_follow
assignee: aobs
model_class: astra
owned_docs: [docs/observability/otlp]
release_train: '2.0'
---

# D.8 — Cross-path qualification and observability docs

## Goal and dependency

Qualify the complete restored OTLP surface and close the regression with
collector evidence and current-schema documentation. D.8 `must_follow`s D.7;
D.6 is also a completion prerequisite. It adds no third transport.

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
   failure, recovery, flush, and idempotent shutdown. SDK cases must use and
   await D.6's async lifecycle; legacy cases must exercise both its async
   worker-barrier completion and synchronous compatibility lifecycle.
3. Add CI jobs/features for both backends with hermetic collectors and no
   hidden external service requirement.
4. Restore Grafana dashboard and LogQL/trace/metric recipes from legacy phases
   AV–AY only after translating them to current neutral resource/attribute
   schema. Record a disposition for each retained or omitted recipe; no stale
   ATM-only label is presented as a generic contract. Use the exact blobs and
   destinations in `legacy-otlp-provenance.json`, including
   `docs/observability/otlp/` and `scripts/ci/`; validate the import manifest's
   pinned-source and allowed-delta checks.
5. After D.6 records technical-lead acceptance of ADR-018, check that the
   operational docs link to the settled lifecycle contract. D.8 edits only its
   operational docs; architecture, dependency/license, release, and D.6-owned
   contract corrections are reported to their owning sprint rather than
   restated here. This is review work, not a permanent CI gate.

## Acceptance criteria

- Both backends export all three signal families from the shared corpus and
  decoded collector output is semantically equivalent for every required field.
- Every lifecycle/failure negative case asserts the exact D.6-L health and
  accounting contract for both backends, with no credential leakage; D.8
  neither extends nor restates that model.
- Awaited SDK shutdown surfaces the actual final-export failure on both
  current-thread and multi-thread runtimes; after successful await the host can
  tear its runtime down immediately without losing an admitted export.
- Runtime teardown before async completion asserts the corresponding D.6-L
  failure-table outcome and accounting; concurrent emit/flush/shutdown follows
  D.6's sequence/barrier contract.
- Feature isolation proves the synchronous path needs no caller-owned Tokio runtime and
  no official OTel SDK/tonic dependency while explicitly recording reqwest's
  internal transitive Tokio graph.
- Restored dashboards/queries work against the current collector fixture and
  have a complete legacy-to-current disposition inventory.
- Import provenance validates pinned source commit/blob ids and the named
  allowed deltas for transplanted destinations.
- ADR-018 is accepted and OTLP-021 is active: neither document retains
  proposed/"until that acceptance" wording at sprint closure.

## Required validation

- Shared dual-backend conformance suite and negative matrix.
- Feature-isolated tests for `otlp-sdk` and `legacy-http-json`, plus combined
  feature tests and `cargo tree -e features` assertions for both graphs.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public API/semver, dependency/license, and CI workflow validation.
- Local collector smoke commands for both backends at one exact source SHA.
- A backend × protocol × runtime × feature conformance matrix covering every
  valid and rejected combination, `debug_local_export`,
  `insecure_skip_verify`, disabled transport, queue pressure, deadline, and
  worker/provider failure.

## Non-closure

No `atm-core` implementation, Python OTEL binding (#88), registry publication,
or new transport beyond the two qualified paths.
