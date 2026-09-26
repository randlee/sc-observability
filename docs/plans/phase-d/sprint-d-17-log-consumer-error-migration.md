# d-17: Log consumer error migration

Generated projection of `obs-d-17`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 15
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

1. Migrate the log-consumer check and the three named consumer examples to obs-d-12/obs-d-13 (ADR-017, PHD-001) typed error signatures.
2. Retype consumer tests and example compile checks to preserve typed error handling.

## This Sprint Does Not Close

Log-crate internal construction is obs-d-16; bridge implementation is obs-d-2; workspace API/release evidence is obs-d-18. The sprint doc is supporting context, not a separate closure gate.

## Design

## Consumer migration recipe

Use obs-d-12's cause mapping and obs-d-13 canonical sink contract (ADR-017, ADR-019, PHD-001). A sink write/flush produces LogSinkError::Write/Flush respectively; shutdown deadline produces Timeout and other drain/provider failure Drain. Preserve TelemetryError::Shutdown and exact typed ExportError runtime sources, never invent ExportError::Lifecycle.

Dependency routes: log-consumer-check depends only on sc-observability-log and uses its canonical type re-exports (preserve macro hygiene; no direct macros/types dependency is added). custom-sink-example reaches LogSinkError through core; atm-adapter-example reaches OTLP error contracts through its existing core/types/OTLP dependencies; tauri-logging uses its existing bridge/runtime edges. D.12 alone adjusts a Cargo dependency if needed; this bead edits source and compile fixtures only. No dependency from a lower crate to OTLP or DTO to runtime is introduced. Public examples use existing strong constructors and mandatory-remediation ErrorContext, with bounded details and retained sources.

## Canonical boundary and handoff

This bead retargets owned consumers and tests to the accepted ADR-017 surface. Existing transitional compatibility is consumed by obs-d-18, which owns canonical activation and final compatibility retirement. ADR-019 records cause-specific migration, diagnostics/source preservation, and owner-only capabilities.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-17, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log-consumer-check/src/lib.rs`
- `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
- `examples/custom-sink-example/src/main.rs`
- `examples/tauri-logging/src-tauri/src/main.rs`
- `examples/atm-adapter-example/src/main.rs`
- `docs/plans/phase-d/sprint-d-17-log-consumer-error-migration.md`

## Acceptance criteria

- [ ] `cargo test -p sc-observability-log-consumer-check --test control_consumer --locked` proves bridge-only dependency and canonical failures (obs-d-17#1/#2).
- [ ] The three named example manifests compile with locked `cargo check` (obs-d-17#2).
- [ ] This sprint does not close workspace release/API evidence; obs-d-18 does. The sprint document is supporting context, not an independent gate.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; obs-d-18 additionally runs all-features release tests and semver/removal gates.
