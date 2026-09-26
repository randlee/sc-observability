# d-6: OTLP lifecycle core

Generated projection of `obs-d-6`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 10
- Assignee / model: lobs / luna
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: OTLP lifecycle module
- Branch: `sprint/d-6-otlp-lifecycle-core`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-6-otlp-lifecycle-core`
- PR target (merge order only): `sprint/d-5-otlp-signal-model`
- Blocked by: `obs-d-21-sanity`
- Requirements: LAY-005, NFR-007, OTLP-012, OTLP-013, OTLP-021, PHD-003, PHD-004
- ADRs: ADR-004, ADR-005, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp/src/lifecycle.rs`
  - `crates/sc-observability-otlp/src/lifecycle_tests.rs`
  - `docs/plans/phase-d/sprint-d-6-otlp-lifecycle-core.md`

## Deliverables

1. Implement the shared lifecycle core in `lifecycle.rs`: ordered barriers, bounded record/byte admission, shutdown ordering, cancellation, health/accounting, and adapter injection points.
2. Consume D.12 neutral signals and D.21 validated bounds without redefining types, config/defaults, factories, `ExporterSet`, error variants, or registries.
3. Prove fail-open admission, terminal lifecycle outcomes, and backend-neutral completion with D.21 recording-exporter fixtures.
4. Document and test the shared lifecycle contract.

## This Sprint Does Not Close

D.12 owns shared types/errors; D.21 owns config/defaults/validation, factory, `ExporterSet`, and fixtures. D.7 and D.8 own only backend behavior and call this lifecycle core. D.18 owns the public `Telemetry` facade and its `lib.rs` integration.
## Design

## Ownership split

D.6 exclusively owns the lifecycle barrier, shutdown ordering, and admission control in `lifecycle.rs` and `lifecycle_tests.rs`. D.7 and D.8 call this core and own only SDK and legacy HTTP/JSON backend behavior respectively; neither owns a second lifecycle state machine, barrier, shutdown policy, or admission policy.

D.6 consumes D.21’s frozen contracts, validated record/byte bounds, config/defaults, factory, `ExporterSet`, and recording fixtures unchanged. It defines no exporter trait, config field, error variant, constant, registry, module declaration, or public facade behavior.

## Lifecycle contract

One short admission lock gives each export or lifecycle command a monotonic sequence. A flush barrier completes only after all earlier admissions have terminal outcomes. Shutdown atomically changes `Open -> Closing`, rejects later emits with `TelemetryError::Shutdown`, drains prior admissions, performs provider shutdown once, stores its terminal result, and changes `Closing -> Shutdown`; concurrent callers share that completion and later terminal calls are idempotent. Full or closed admission fails open, increments per-signal dropped accounting, and records degraded health without waiting.

D.6 supplies internal backend-neutral barriers and preflight hooks. It does not expose or integrate the public `Telemetry` facade: **Handoff to obs-d-18:** integrate those internal lifecycle operations into `Telemetry` and `lib.rs` while preserving the same ordering, typed outcomes, and no-backend-branch rule.

## Handoff from obs-d-21 (wave 1)

Consume D.21’s sanity-gated contract artifact without changing its ownership.

- `crates/sc-observability-otlp/src/lifecycle.rs`
- `crates/sc-observability-otlp/src/lifecycle_tests.rs`

## ADR applicability

ADR-004 constrains this OTLP-only lifecycle module; ADR-005 constrains its
registry-backed terminal results; ADR-017 constrains its 2.0 typed outcomes;
and ADR-019 governs the shared lifecycle/registry ownership split. None grants
D.6 configuration, constants, or facade ownership.
## Acceptance criteria

- [ ] Deliverable 1: `cargo test -p sc-observability-otlp --lib lifecycle_tests --all-features --locked` externally exercises ordered barriers, repeated shutdown, cancelled waiters, deadlines, runtime termination, and exact-once drop accounting.
- [ ] Deliverable 2: an external lifecycle test proves resource/scope/flags/links/histogram payload preservation and enforced nonblocking record/byte admission.
- [ ] Deliverable 3: fixture tests prove fail-open full/closed admission, degraded health, and stable terminal outcomes without `block_on`, a second runtime, or a backend branch.
- [ ] Deliverable 4: `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass.
- [ ] D.6 does not close SDK/legacy transport behavior or public-facade integration; D.7/D.8 and D.18 own those outcomes.

## Implementation notes

The staged lifecycle core now provides the shared contract for D.7/D.8:

- admission reserves record and byte capacity under one short lock and assigns
  a monotonic sequence; releasing an admission wakes only the barriers that
  can now advance;
- flush and shutdown are shared futures, so concurrent callers do not create
  duplicate provider operations; shutdown closes admission before draining and
  transitions to the terminal state exactly once;
- full queues, closed admission, dropped permits, backend failures, and
  lifecycle deadlines update fail-open health/accounting without requiring an
  executor-specific `block_on` path;
- payloads remain neutral `T` values at this layer. Signal projectors and
  backend adapters retain ownership of resource, scope, flags, links,
  histogram, and wire-specific interpretation.

The public facade and backend construction remain intentionally out of scope;
obs-d-18 integrates these internal operations after D.7/D.8 provide their
exporter adapters.
