# d-14: Observe error migration

Generated projection of `obs-d-14`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 13
- Assignee / model: cobs / terra
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: sc-observe
- Branch: `sprint/d-14-observe-error-migration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-14-observe-error-migration`
- PR target (merge order only): `sprint/d-8-otlp-http-json-transplant`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-002, LAY-003, LAY-004, LAY-006, LAY-007, LOG-004, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-047, LOG-048, NFR-001, NFR-002, NFR-003, NFR-004, NFR-005, NFR-006, NFR-007, NFR-009, OBS-001, OBS-002, OBS-003, OBS-004, OBS-005, OBS-006, OBS-007, OBS-008, OBS-009, OBS-010, OBS-011, OBS-012, OBS-013, OBS-014, OBS-015, OBS-016, OBS-017, OBS-018, OBS-019, OBS-020, OBS-021, OBS-022, OBS-023, OBS-024, OBS-025, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, PHD-001, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-001, ADR-002, ADR-003, ADR-004, ADR-005, ADR-007, ADR-009, ADR-010, ADR-014, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observe/src/lib.rs`
  - `crates/sc-observe/tests/**`
  - `docs/plans/phase-d/sprint-d-14-observe-error-migration.md`

## Deliverables

1. Migrate the `ObserveError` and error call sites in `sc-observe` to the D.12 non-exhaustive error enums carrying `Box<ErrorContext>`.
2. Retype every owned call site and preserve the `sc-observe` runtime guards while consuming the canonical types-crate definitions.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.

## This Sprint Does Not Close

Canonical boundary activation, workspace-wide compatibility retirement, public re-exports, and release/API approval are owned by `obs-d-18`.
Update the sprint doc as explanatory evidence alongside the migration; documentation is not a separate closure gate.


## Design

## Migration recipe

Consume `obs-d-12`'s cause-to-variant mapping and `ErrorContext` contract (ADR-017, PHD-001). Use the canonical sc-observability-types definitions and re-export them where the existing `sc-observe` surface requires it. Preserve `ObservationError` runtime guards (`Shutdown`, `QueueFull`, `RoutingFailure`) and their nested sources; do not rename the runtime guard into `EventError`. Init validation maps `Configuration`, startup failure `Runtime`; flush maps `Drain`; shutdown deadline maps `Timeout` and other drain/provider failure `Drain`; sink write/flush map their distinct variants; projection/subscriber failures keep their exact canonical categories. Migrate neutral span/metric consumers inside this crate as D.12 specifies; never add an OTLP runtime dependency. Use structured `Diagnostic.details` for route/projector/etc., not invented `ErrorContext` fields. Retype `routing_integration.rs` and `typed_observation.rs` with per-cause assertions.

## Canonical boundary and handoff

This bead retargets owned call sites and tests to the accepted ADR-017 surface. Any transitional compatibility needed by unfinished sibling consumers is limited to the existing boundary and is consumed by `obs-d-18`, which owns canonical activation and final compatibility retirement. No new legacy feature or duplicate classifier is introduced. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by `obs-d-14`, owned by `obs-d-18` from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion and final compatibility retirement.

- `crates/sc-observe/src/lib.rs`
- `crates/sc-observe/tests/routing_integration.rs`
- `crates/sc-observe/tests/typed_observation.rs`

## Contract ties

- `PHD-001` governs canonical same-name error variants, cause mapping, and retained source/diagnostic information.
- `NFR-004` and `ADR-004` constrain this crate to remain free of OTLP transport complexity.
- `ADR-014` is retained for result-preserving error boundaries consumed by downstream language-facing callers.

## Implementation evidence

- `*_typed` construction and lifecycle APIs now return the staged
  `sc_observability_types::v2::{InitError, FlushError, ShutdownError}` values.
  Validation uses `InitError::Configuration`, logger startup uses
  `InitError::Runtime`, and logger drain failures use `FlushError::Drain`.
- The retained 1.x methods form the only compatibility boundary: they consume
  the canonical error's original boxed context without rebuilding its
  diagnostic or source chain. `ObservationError::{Shutdown, QueueFull,
  RoutingFailure}` remains the runtime admission/routing guard contract.
- Focused tests assert canonical variants, stable diagnostic codes, and the
  original source context across the canonical-to-legacy compatibility
  boundary. The currently
  published subscriber/projector and `LogSink` traits still expose their 1.x
  callback signatures; their final public-surface activation is handed to
  `obs-d-18`.


## Acceptance criteria

- [ ] `cargo test -p sc-observe --locked` passes canonical and neutral-model cases, including routing guards and per-cause variants/source identity (`obs-d-14#1/#3`).
- [ ] Owned call sites use the canonical types-crate definitions and preserve runtime guards, diagnostics, and source identity (`obs-d-14#2`).
- [ ] This sprint does not close workspace bindings, collector behavior, canonical activation, or release approvals; `obs-d-18`/`obs-d-9` do.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; `obs-d-18` additionally runs all-features release tests and semver/removal gates.
