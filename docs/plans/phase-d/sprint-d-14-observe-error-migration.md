# d-14: Observe error migration

## Plan metadata

- Wave: 12
- Branch: `sprint/d-14-observe-error-migration`
- PR target: `sprint/d-8-otlp-http-json-transplant`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observe/src/lib.rs`
  - `crates/sc-observe/tests/**`
  - `docs/plans/phase-d/sprint-d-14-observe-error-migration.md`

## Deliverables

1. Migrate the ObserveError and error wrappers in sc-observe to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observe.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.


## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.
Update the sprint doc as explanatory evidence alongside code; documentation is not a separate closure gate.


## Design

## Migration recipe

Consume obs-d-12's one cause-to-variant mapping and ErrorContext contract (ADR-017). Remove local same-name canonical enum/wrapper copies and re-export sc-observability-types definitions. Preserve ObservationError runtime guards (Shutdown, QueueFull, RoutingFailure) and their nested sources; do not rename the runtime guard into EventError. Init validation maps Configuration, startup failure Runtime; flush maps Drain; shutdown deadline maps Timeout and other drain/provider failure Drain; sink write/flush map their distinct variants; projection/subscriber failures keep their exact canonical categories. Migrate neutral span/metric consumers inside this crate as D.12 specifies; never add an OTLP runtime dependency. Use structured Diagnostic.details for route/projector/etc., not invented ErrorContext fields. Retype routing_integration.rs and typed_observation.rs with per-cause assertions.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-14, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observe/src/lib.rs`
- `crates/sc-observe/tests/routing_integration.rs`
- `crates/sc-observe/tests/typed_observation.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observe --locked` passes canonical and neutral-model cases, including routing guards and per-cause variants/source identity (D1/D3).
- [ ] No local same-name shared error definitions or legacy wrapper constructors remain on the migrated v2 path in sc-observe; only the surviving types-crate definitions are re-exported (D2).
- [ ] This sprint does not close workspace bindings, collector behavior or release approvals; D.18/D.9 do.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

