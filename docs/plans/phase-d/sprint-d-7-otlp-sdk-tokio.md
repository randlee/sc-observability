# d-7: Official SDK/Tokio adapter

Generated projection of `obs-d-7`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 12
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: OTLP SDK adapter module
- Branch: `sprint/d-7-otlp-sdk-tokio`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-7-otlp-sdk-tokio`
- PR target (merge order only): `sprint/d-22-otlp-types`
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

Implement the Tokio transport: send the shared OTLP requests over gRPC and HTTP/protobuf on the caller's runtime through the D.6 lifecycle core, against the D.21 `crate::contracts` seam, without owning lifecycle policy or a data model.

## Deliverables

1. Consume D.21’s pins and feature allowlist (as amended 2026-09-27), `sdk/mod.rs`, and the `sc_observability_types::otlp` contract through `crate::contracts` read-only; implement only `sdk/implementation.rs` and `sdk/tests.rs`.
2. Send obs-d-22 `OtlpLogs`/`OtlpSpans`/`OtlpMetrics` batches as OTLP export requests over gRPC (the `opentelemetry-proto` generated tonic clients) and HTTP/protobuf (`reqwest-sdk`), taking them by value or by reference and encoding them straight to the wire. There is no intermediate model, no per-record conversion and no projection through `opentelemetry_sdk` data types. A clone is allowed only where a signature forces it (such as the generated client's owned request built from a borrowed batch); each is named and justified in module docs and covered by a test.
3. Schedule on the caller's Tokio runtime through D.6: the sync `LogExporter`/`TraceExporter`/`MetricExporter` adapters in `contracts::ExporterSet` admit, `handle.spawn`, and `Admitted::complete`, returning `Ok` once scheduled. Create no runtime and never call `block_on`. Export one request per resource group, with a mixed-resource test. Preserve D.21’s explicit validated settings and typed accounting outcomes.
4. Own `examples/otlp-sdk/Cargo.toml` and source fixtures that externally exercise transport of obs-d-22 batches, pressure, deadlines, async completion, and host-runtime teardown.

## This Sprint Does Not Close

D.21 owns manifests, allowlists, the `crate::contracts` re-export, module declarations, config, and neutral→proto conversion; D.22 owns the shared OTLP types module; D.12 owns shared errors and registry definitions; D.6 owns lifecycle policy; D.18 composes production pieces; D.9 qualifies both backends against collectors.
## Design

## SDK implementation contract

D.7 owns only `sdk/implementation.rs`, `sdk/tests.rs`, `examples/otlp-sdk/Cargo.toml`, and `examples/otlp-sdk/src/**`. D.21 owns and stubs `sdk/mod.rs`, the feature/dependency declarations, config, contracts, and shared `ExporterLifecycle`; D.7 must consume them unchanged. D.6 owns the shared lifecycle barrier, shutdown ordering, and admission control. D.7 calls that core and owns only transport behavior.

Retry for this transport is an open lead decision: the SDK exporter that owned it is no longer on the send path (plan decision 10). The adapter sets validated client values explicitly, never lets ambient `OTEL_*` defaults override them, and never creates a hidden runtime or substitutes no-op. It preserves D.12 typed terminal/deadline/accounting results at its supported external consumer boundary; expected failures remain results rather than panics or false success (ADR-014).

## Handoff from obs-d-22 (wave 2)

Consume the shared `sc_observability_types::otlp` contract through `crate::contracts`; D.7 has no obs-d-22-sanity blocker and adds no second projection.

Interim placement: see D.22's **Interim placement (user ruling 2026-09-27)**; import the shared contract only through `crate::contracts`.

## Handoff from obs-d-21 (wave 1)

Consume D.21’s sanity-gated interfaces without altering its module or manifest ownership.

- `crates/sc-observability-otlp/src/sdk/implementation.rs`
- `crates/sc-observability-otlp/src/sdk/tests.rs`

## Facade-composition handoff

D.7 hands obs-d-18 the crate-private constructor contract at
`crate::sdk::implementation::build_exporter_set`. Obs-d-18 composes it only
through D.21’s `Telemetry` facade/module path; D.7 retains transport
behavior.
## Acceptance criteria

- [ ] `cargo test -p sc-observability-otlp --lib sdk::tests --features otlp-sdk --locked` runs gRPC and HTTP/protobuf transport of all three signals, mixed-resource per-group export, Ok-once-scheduled without runtime creation or `block_on`, named forced clones, deadline/terminal, explicit-config-vs-env, queue-pressure, shutdown and caller-runtime teardown tests (D1–D3).
- [ ] `cargo check --manifest-path examples/otlp-sdk/Cargo.toml --locked` passes the Tokio-hosted 2.0 consumer against contract interfaces (D4).
- [ ] This sprint does not close real shared-core composition or dual collector equivalence; D.18/D.9 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
