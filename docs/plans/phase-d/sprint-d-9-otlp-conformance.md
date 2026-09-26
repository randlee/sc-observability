# d-9: Cross-path qualification and observability docs

## Plan metadata

- Wave: 17
- Branch: `sprint/d-9-otlp-conformance`
- PR target: `sprint/d-18-integration-and-public-api`
- Blocked by: `obs-d-18-sanity`
- Owned paths:
  - `crates/sc-observability-otlp/tests/full_stack_integration.rs`
  - `docs/observability/otlp/**`
  - `scripts/ci/fixtures/otlp/**`

## Goal and dependency

Qualify the complete restored OTLP surface and close the regression with
collector evidence and current-schema documentation. D.9 `must_follow`s D.7
and D.8; it adds no third transport.


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
   schema. No stale ATM-only label is presented as a generic contract. Use the exact blobs and
   destinations in `legacy-otlp-provenance.json`, including
   `docs/observability/otlp/` and `scripts/ci/`; validate the import manifest's
   pinned-source and allowed-delta checks.


## Non-closure

No `atm-core` implementation, Python OTEL binding (#88), registry publication,
or new transport beyond the two qualified paths.


## Design



## Implementation targets

 implement or update the named contract consumer and its focused test for the corresponding numbered deliverable.\n

## Acceptance criteria

## Acceptance criteria

- Both backends export all three signal families from the shared corpus and
  decoded collector output is semantically equivalent for every required field.
- Every lifecycle/failure negative case asserts the exact D.6 health and
  accounting contract for both backends, with no credential leakage; D.9
  neither extends nor restates that model.
- Awaited SDK shutdown surfaces the actual final-export failure on both
  current-thread and multi-thread runtimes; after successful await the host can
  tear its runtime down immediately without losing an admitted export.
- Runtime teardown before async completion asserts the corresponding D.6
  failure-table outcome and accounting; concurrent emit/flush/shutdown follows
  D.6's sequence/barrier contract.
- Feature isolation proves the synchronous path needs no caller-owned Tokio runtime and
  no official OTel SDK/tonic dependency while explicitly recording reqwest's
  internal transitive Tokio graph.
- Restored dashboards/queries work against the current collector fixture.
- Import provenance validates pinned source commit/blob ids and the named
  allowed deltas for transplanted destinations.


## Required validation

- Shared dual-backend conformance suite and negative matrix.
- Feature-isolated tests for `otlp-sdk` and `legacy-http-json`, plus combined
  feature tests for both graphs.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public API/semver, dependency/license, and CI workflow validation.
- Local collector smoke commands for both backends.


