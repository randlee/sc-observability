# d-17: Log consumer error migration

Generated projection of `obs-d-17`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 16
- Assignee / model: lobs / luna
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: log consumer migration
- Branch: `sprint/d-17-log-consumer-error-migration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-17-log-consumer-error-migration`
- PR target (merge order only): `sprint/d-16-log-error-migration`
- Blocked by: `obs-d-12-sanity`, `obs-d-13-sanity`
- Requirements: DOC-003, LAY-001, LAY-002, LAY-006, LAY-007, LOG-001, LOG-002, LOG-003, LOG-004, LOG-007, LOG-010, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-046, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, OOS-001, OOS-002, OOS-003, OOS-004, OOS-005, OOS-006, OOS-007, OOS-008, OTLP-006, OTLP-007, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, PHB-012, PHD-001, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-004, ADR-005, ADR-006, ADR-008, ADR-009, ADR-010, ADR-014, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-log-consumer-check/src/lib.rs`
  - `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
  - `docs/plans/phase-d/sprint-d-17-log-consumer-error-migration.md`
  - `examples/atm-adapter-example/src/main.rs`
  - `examples/custom-sink-example/src/main.rs`
  - `examples/tauri-logging/src-tauri/src/main.rs`

## Deliverables

1. Migrate the log-consumer check and the three named consumer examples to `obs-d-12`/`obs-d-13` (`ADR-017`, `PHD-001`) typed error signatures.
2. Retype consumer tests and example compile checks to preserve typed error handling.

## This Sprint Does Not Close

Log-crate internal construction is `obs-d-16`; bridge implementation is `obs-d-2`; workspace API/release evidence is `obs-d-18`.
The sprint document is supporting context and is not a separate closure gate.


## Design

## Consumer migration recipe

Use `obs-d-12`'s cause mapping and `obs-d-13` canonical sink contract (ADR-017, ADR-019, PHD-001). A sink write/flush produces `LogSinkError::Write/Flush` respectively; shutdown deadline produces `Timeout` and other drain/provider failure `Drain`. Preserve `TelemetryError::Shutdown` and exact typed `ExportError` runtime sources; do not invent `ExportError::Lifecycle`.

Dependency routes: log-consumer-check depends only on `sc-observability-log` and uses its canonical type re-exports (preserve macro hygiene; no direct macros/types dependency is added). `custom-sink-example` reaches `LogSinkError` through core; `atm-adapter-example` reaches OTLP error contracts through its existing core/types/OTLP dependencies; `tauri-logging` uses its existing bridge/runtime edges. D.12 alone adjusts a Cargo dependency if needed; this bead edits source and compile fixtures only. No dependency from a lower crate to OTLP or DTO to runtime is introduced. Public examples use existing strong constructors and mandatory-remediation `ErrorContext`, with bounded details and retained sources.

## Canonical boundary and handoff

This bead retargets owned consumers and tests to the accepted ADR-017 surface. Any transitional compatibility needed by unfinished sibling consumers is limited to the existing boundary and is consumed by `obs-d-18`, which owns canonical activation and final compatibility retirement. No new legacy feature or duplicate classifier is introduced. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by `obs-d-17`, owned by `obs-d-18` from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion and final compatibility retirement. The handoff includes every owned consumer fixture and the supporting sprint projection, including `atm-adapter-example/src/main.rs` and `docs/plans/phase-d/sprint-d-17-log-consumer-error-migration.md` (`REQ-QA-d17-006`).

- `crates/sc-observability-log-consumer-check/src/lib.rs`
- `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
- `examples/atm-adapter-example/src/main.rs`
- `examples/custom-sink-example/src/main.rs`
- `examples/tauri-logging/src-tauri/src/main.rs`
- `docs/plans/phase-d/sprint-d-17-log-consumer-error-migration.md`

## Contract ties

- `LOG-002`, `LOG-016`, and `LOG-046` govern the public logger surface, health result, and shutdown behavior consumed by the examples.
- `OTLP-006`/`OTLP-007` govern typed telemetry results and shutdown behavior in the ATM example.
- `ADR-004` and `ADR-018` govern the existing OTLP boundary and backend-neutral lifecycle contract.
- `ADR-019` records the cause-specific consumer migration, preserved diagnostics/source, owner-only capabilities, and the fact that wrapper removal/semver belongs solely to `obs-d-18`.

## Acceptance criteria

- [ ] `cargo test -p sc-observability-log-consumer-check --test control_consumer --locked` proves the consumer needs only the bridge dependency and matches canonical failures (`obs-d-17#1/#2`).
- [ ] `cargo check --manifest-path examples/atm-adapter-example/Cargo.toml --locked`, `cargo check --manifest-path examples/custom-sink-example/Cargo.toml --locked`, and `cargo check --manifest-path examples/tauri-logging/src-tauri/Cargo.toml --locked` compile the migrated examples (`obs-d-17#2`).
- [ ] This sprint does not close workspace release/API evidence; `obs-d-18` does. The sprint document is supporting context, not an independent gate.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; `obs-d-18` additionally runs all-features release tests and semver/removal gates.
