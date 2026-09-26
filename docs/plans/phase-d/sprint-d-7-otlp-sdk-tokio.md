# d-7: Official SDK/Tokio adapter

Generated projection of `obs-d-7`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 11
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: OTLP SDK adapter module
- Branch: `sprint/d-7-otlp-sdk-tokio`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-7-otlp-sdk-tokio`
- PR target (merge order only): `sprint/d-6-otlp-lifecycle-core`
- Blocked by: `obs-d-21-sanity`
- Requirements: LAY-005, NFR-004, NFR-007, OTLP-012, OTLP-013, OTLP-021, PHD-003, PHD-004
- ADRs: ADR-004, ADR-005, ADR-014, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp/src/sdk/implementation.rs`
  - `crates/sc-observability-otlp/src/sdk/tests.rs`
  - `docs/plans/phase-d/sprint-d-7-otlp-sdk-tokio.md`
  - `examples/otlp-sdk/Cargo.toml`
  - `examples/otlp-sdk/src/**`

## Goal

Implement the official SDK/Tokio adapter against D.21 crate-private contracts, using the D.6 lifecycle core without owning lifecycle policy.

## Deliverables

1. Consume D.21’s SDK pins, feature allowlist, and `sdk/mod.rs`; implement only `sdk/implementation.rs` and `sdk/tests.rs`.
2. Convert neutral logs, spans, and metrics to SDK data without loss of resource/scope, trace, status, histogram, or temporal data.
3. Call D.6’s shared lifecycle/admission core from the caller Tokio runtime and preserve D.21’s explicit validated settings and typed accounting outcomes.
4. Own `examples/otlp-sdk/Cargo.toml` and source fixtures that externally exercise signal mappings, pressure, deadlines, async completion, and host-runtime teardown.

## This Sprint Does Not Close

D.21 owns manifests, allowlists, OTLP registry re-exports, module declarations, config, and exporter contract types; D.12 owns shared errors and registry definitions; D.6 owns lifecycle policy; D.18 composes production pieces; D.9 qualifies both backends against collectors.
## Design

## SDK implementation contract

D.7 owns only `sdk/implementation.rs`, `sdk/tests.rs`, `examples/otlp-sdk/Cargo.toml`, and `examples/otlp-sdk/src/**`. D.21 owns and stubs `sdk/mod.rs`, the feature/dependency declarations, config, contracts, and shared `ExporterLifecycle`; D.7 must consume them unchanged. D.6 owns the shared lifecycle barrier, shutdown ordering, and admission control. D.7 calls that core and owns only SDK provider/batch-processor behavior.

The adapter delegates retry exclusively to the pinned official SDK, sets validated builder values explicitly, never lets ambient `OTEL_*` defaults override them, and never creates a hidden runtime or substitutes no-op. It preserves D.12 typed terminal/deadline/accounting results at its supported external consumer boundary; expected failures remain results rather than panics or false success (ADR-014).

## Handoff from obs-d-21 (wave 1)

Consume D.21’s sanity-gated interfaces without altering its module or manifest ownership.

- `crates/sc-observability-otlp/src/sdk/implementation.rs`
- `crates/sc-observability-otlp/src/sdk/tests.rs`

## Facade-composition handoff

D.7 hands obs-d-18 the crate-private constructor contract at
`crate::sdk::implementation::build_exporter_set`. Obs-d-18 composes it only
through D.21’s `Telemetry` facade/module path; D.7 retains provider and
batch-processor behavior.
## Acceptance criteria

- [ ] `cargo test -p sc-observability-otlp --lib sdk::tests --features otlp-sdk --locked` runs all signal mappings, retry-deadline/terminal, explicit-config-vs-env, queue-pressure, shutdown and caller-runtime teardown tests (D1–D3).
- [ ] `cargo check --manifest-path examples/otlp-sdk/Cargo.toml --locked` passes the Tokio-hosted 2.0 consumer against contract interfaces (D4).
- [ ] This sprint does not close real shared-core composition or dual collector equivalence; D.18/D.9 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
