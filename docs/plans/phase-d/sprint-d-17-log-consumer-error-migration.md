# d-17: Log consumer error migration

## Plan metadata

- Wave: 15
- Branch: `sprint/d-17-log-consumer-error-migration`
- PR target: `sprint/d-16-log-error-migration`
- Blocked by: `obs-d-12-sanity`, `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability-log-consumer-check/src/lib.rs`
  - `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
  - `docs/plans/phase-d/sprint-d-17-log-consumer-error-migration.md`
  - `examples/atm-adapter-example/src/main.rs`
  - `examples/custom-sink-example/src/main.rs`
  - `examples/tauri-logging/src-tauri/src/main.rs`

## Deliverables

1. Migrate the log-consumer check and the three named consumer examples to obs-d-12/obs-d-13 (ADR-017) typed error signatures.
2. Retype consumer tests and example compile checks to preserve typed error handling.
3. Update the consumer migration sprint documentation.

## This Sprint Does Not Close

Log-crate internal construction is D16; bridge implementation is D2; workspace API/release evidence is D18.
Update the sprint doc as explanatory evidence alongside code; documentation is not a separate closure gate.


## Design

## Consumer migration recipe

Use obs-d-12's cause mapping and obs-d-13 canonical sink contract (ADR-017). A sink write/flush produces LogSinkError::Write/Flush respectively; shutdown deadline produces Timeout and other drain/provider failure Drain. Preserve TelemetryError::Shutdown and exact typed ExportError runtime sources, never invent ExportError::Lifecycle.

Dependency routes: log-consumer-check depends only on sc-observability-log and uses its canonical type re-exports (preserve macro hygiene; no direct macros/types dependency is added). custom-sink-example reaches LogSinkError through core; atm-adapter-example reaches OTLP error contracts through its existing core/types/OTLP dependencies; tauri-logging uses its existing bridge/runtime edges. D.12 alone adjusts a Cargo dependency if needed; this bead edits source and compile fixtures only. No dependency from a lower crate to OTLP or DTO to runtime is introduced. Public examples use existing strong constructors and mandatory-remediation ErrorContext, with bounded details and retained sources.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-17, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log-consumer-check/src/lib.rs`
- `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
- `examples/custom-sink-example/src/main.rs`
- `examples/tauri-logging/src-tauri/src/main.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observability-log-consumer-check --test control_consumer --locked` proves the consumer needs only the bridge dependency and matches canonical failures (D1/D2).
- [ ] `cargo check --manifest-path examples/atm-adapter-example/Cargo.toml --locked`, `cargo check --manifest-path examples/custom-sink-example/Cargo.toml --locked` and `cargo check --manifest-path examples/tauri-logging/src-tauri/Cargo.toml --locked` compile the migrated examples (D2).
- [ ] Supporting sprint documentation matches source/fixture results (D3); no sc-observability-log-macros runtime migration or nonexistent D4 criterion. This sprint does not close workspace release/API evidence; D.18 does.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

