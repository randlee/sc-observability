# d-7: Official SDK/Tokio adapter

## Plan metadata

- Wave: 10
- Branch: `sprint/d-7-otlp-sdk-tokio`
- PR target: `sprint/d-6-otlp-lifecycle-core`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-otlp/src/sdk/implementation.rs`
  - `crates/sc-observability-otlp/src/sdk/mod.rs`
  - `crates/sc-observability-otlp/src/sdk/tests.rs`
  - `docs/plans/phase-d/sprint-d-7-otlp-sdk-tokio.md`
  - `examples/otlp-sdk/src/**`

## Goal

Implement the official SDK/Tokio adapter against obs-d-12 crate-private contracts, parallel with D.6/D.8.

## Deliverables

1. Consume the SDK dependency pins, features and module declarations recorded by D.12; implement only sdk/implementation.rs and sdk/tests.rs.

2. Convert neutral logs/spans/metrics into SDK data without losing resource/scope, trace kind/flags/links/events/status or histogram buckets and temporal metadata.

3. Implement the common exporter/lifecycle interface with the caller Tokio runtime and SDK batch processor; use explicit validated settings and preserve all stable failure/accounting outcomes.

4. Add adapter loopback fixtures and examples/otlp-sdk source for all signals, pressure, timeout, async completion and host-runtime teardown using the D.12 contract fixture.

## This Sprint Does Not Close

D.12 owns manifests/allowlists/registries; D.6 owns the shared lifecycle implementation; D.18 composes the production pieces; D.9 qualifies both backends against collectors.


## Design

## SDK implementation contract

Only sdk/implementation.rs and sdk/tests.rs implement the SDK adapter. D.12 owns sdk/mod.rs, config, dependency pins and shared ExporterLifecycle. Use the frozen recording lifecycle fixture to test independently of D.6. D.18 performs final factory composition, so no sibling dependency is needed.

SDK retry behavior is delegated exclusively to the pinned official SDK exporter; add no second retry loop, per-request queue or hidden runtime. SDK-supported transient retry must remain inside the configured request/export and lifecycle deadlines; terminal/expired outcomes map to D.12's TerminalExportFailure/LifecycleTimeout with exact accounting. Document the pinned SDK's actual retry capability in sdk module docs; if it cannot meet a bound, fail construction rather than silently override it. Legacy-only retry fields reject as ConfigFieldNotApplicable. Set all builder values explicitly; ambient OTEL_* defaults never override validated config. No provider is built before matrix/runtime validation, and enabled construction never substitutes no-op.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-12 (wave 1)

Created by obs-d-12, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-otlp/src/sdk/implementation.rs`
- `crates/sc-observability-otlp/src/sdk/mod.rs`
- `crates/sc-observability-otlp/src/sdk/tests.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observability-otlp --lib sdk::tests --features otlp-sdk --locked` runs all signal mappings, retry-deadline/terminal, explicit-config-vs-env, queue-pressure, shutdown and caller-runtime teardown tests (D1–D3).
- [ ] `cargo check --manifest-path examples/otlp-sdk/Cargo.toml --locked` passes the Tokio-hosted 2.0 consumer against contract interfaces (D4).
- [ ] This sprint does not close real shared-core composition or dual collector equivalence; D.18/D.9 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

