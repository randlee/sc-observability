# d-22: Shared OTLP types module

Generated projection of `obs-d-22`; the bead is authoritative.

## Plan metadata

- Wave: 1.5
- Layer: 11
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability-types/src/otlp shared OTLP contract module
- Branch: `sprint/d-22-otlp-types`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-22-otlp-types`
- PR target (merge order only): `sprint/d-6-otlp-lifecycle-core`
- Blocked by: `obs-d-21-sanity`
- Requirements: LAY-001, LAY-005, NFR-004, NFR-009, OTLP-003, OTLP-010, PHD-003, TYP-002, TYP-009, TYP-010, TYP-030
- ADRs: ADR-002, ADR-004, ADR-005, ADR-009, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-types/src/otlp/**`
  - `crates/sc-observability-types/src/lib.rs`
  - `crates/sc-observability-otlp/src/contracts.rs`
  - `crates/sc-observability-otlp/src/contract_tests.rs`
  - `docs/plans/phase-d/sprint-d-22-otlp-types.md`

## Goal

Create `sc_observability_types::otlp`, the shared OTLP contract module for waves 1 and 2. It holds plain Rust wire-shaped types and public OTLP traits without adding a proto dependency to `sc-observability-types`; `sc-observability-otlp` owns conversion to the generated proto messages.

## Deliverables

1. Create `crates/sc-observability-types/src/otlp/mod.rs` by copying `origin/fix/obs-d-21-qa-f2:crates/sc-observability-otlp/src/contracts.rs` byte for byte, then make only the `use`-path and visibility edits needed to compile from the types crate. Register the module from the types crate root. It is a module, not a new workspace crate: `sc-observability-types` adds no `opentelemetry-proto`, `prost`, or `opentelemetry` dependency (ADR-019).

2. Define the plain-Rust OTLP wire model in that module. Every structure representing an OTLP proto message is field-for-field identical to its proto counterpart—same field names, order and types (`Vec<u8>` for bytes, `u64` for fixed64, `String`, `Vec<T>`, `Option<T>` for message fields, and `i32` for enums). Per signal, one resource group (`OtlpLogs`, `OtlpSpans`, `OtlpMetrics`) is the exporter batch element. `ExporterSet<L, S, M>` remains the shared behavior aggregate rather than a proto-message type.

3. In `sc-observability-otlp`, implement `From`/`Into` conversion between the plain-Rust module and `opentelemetry-proto` by field move: no re-encoding, allocation, or `transmute` (prost structs are not `#[repr(C)]`, so layout identity is neither promised nor safe). The neutral→OTLP projection takes inputs by value and moves them, never cloning attribute, event, or link data.

4. Add lossless tests for mixed-resource batches (group order is first appearance, record order kept within a group), span events and links, histogram buckets/count/sum with temporality and start time, and trace/span ids and flags. In `sc-observability-otlp`, round-trip every proto-backed structure through its proto type and assert equality.

5. Prove zero copy: a test builds a record once, projects and groups it, and asserts the heap buffers of its attribute, event and link data are the same allocations inside the resulting resource group. Crate docs list every clone a signature forces, with its justification; the list is empty unless a signature forces one.

6. Move the shared D.21 OTLP contract surface into `sc_observability_types::otlp`. The `git grep` inventory is `origin/fix/obs-d-21-qa-f2:crates/sc-observability-otlp/src/contracts.rs` (that file is absent from `origin/integrate/phase-d`): structs `Resource`, `InstrumentationScope`, `ExportRecord<T>`, `LogRecord`, `CompleteSpan`, and `ExporterSet<L, S, M>`; the `LifecycleFuture` companion alias; and public traits `ExporterLifecycle`, `LogExporter<T>`, `TraceExporter<T>`, and `MetricExporter<T>`. `sc-observability-otlp` has exactly one import seam: `crate::contracts` contains only `pub use sc_observability_types::otlp::*;`; its facade and both transport implementations import only through that seam. The module owns definitions but no implementations: each behavior-owning crate implements the traits it needs—`sc-observability-otlp` for its facade behavior, obs-d-7 for the Tokio transport, obs-d-8 for the blocking transport, and any later behavior owner likewise. Extracting `otlp/` to a future `sc-observability-otlp-types` crate is a later mechanical refactor and blocks no wave-1 or wave-2 work.

## This Sprint Does Not Close

No transport, client, runtime, exporter trait implementation or JSON layer: obs-d-7 sends the model over tonic/reqwest, obs-d-8 over its blocking OTLP/JSON client. Public exporter trait definitions and `ExporterSet` are in scope here, but implementations are not: they stay with the crate that owns the behavior. D.12 owns the neutral types and errors; D.21 owns config and proto conversion; D.6 owns lifecycle behavior; D.18 composes the facade.

## Design

## Dependency fence

`sc_observability_types::otlp` is plain Rust and adds no dependency: in particular, no `opentelemetry-proto`, `prost`, `opentelemetry`, HTTP/gRPC client, Tokio, or `opentelemetry-otlp` dependency. `sc-observability-otlp` keeps its own transport/proto dependencies and performs field-move conversion there. ADR-019 holds.

The module's types are a direct, dependency-free contract. A future extraction of `otlp/` to `sc-observability-otlp-types` is mechanical and does not gate D.7 or D.8.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Interim placement (user ruling 2026-09-27)

Write the inventory once in its final form at `sc_observability_types::otlp`: plain Rust that mirrors OTLP proto fields, with no OpenTelemetry or prost dependency in `sc-observability-types` (ADR-019). `sc-observability-otlp` converts fields to proto and has the sole `crate::contracts` re-export seam; consumers never import the types module directly. The later separate-crate extraction is planned work, not missing work. QA must not request the separate crate, proto derives or dependency now, or a transmute/cast.

## Handoff from obs-d-21 (wave 1)

Consume D.21's sanity-gated types/config contract. Move the shared contract surface out of D.21's `contracts.rs` into `sc_observability_types::otlp`; `sc-observability-otlp` then re-exports or consumes those definitions and owns field-move conversion to proto types.

## Handoff to obs-d-7 and obs-d-8 (wave 2)

obs-d-7 and obs-d-8 are not blocked on a new crate. They consume the shared types module through their existing `sc-observability-types` dependency, take `OtlpLogs`/`OtlpSpans`/`OtlpMetrics` batches by value or by reference, and encode them straight to the wire (obs-d-7 protobuf over tonic/reqwest, obs-d-8 OTLP/JSON) with no intermediate model and no per-record conversion. Each implements its owned transport behavior against the module's public traits.

## Handoff to obs-d-18 (wave 3)

obs-d-18 composes the facade emit path on this projection, moving neutral records into resource groups before admission.

## Acceptance criteria

- [ ] Deliverable 1: `cargo tree -p sc-observability-types -e normal` shows no new `opentelemetry-proto`, `prost`, or `opentelemetry` dependency; `bash scripts/ci/validate_repo_boundaries.sh` and `bash scripts/ci/validate_dependency_bans.sh` pass.
- [ ] Deliverables 2–4: tests cover nonzero lossless cases for logs, spans (events and links), and metrics (histogram, temporality, start time), including a mixed-resource batch and a field-move proto round trip for each proto-backed struct with no `transmute`.
- [ ] Deliverable 5: the zero-copy test passes and the crate docs name every forced clone with its justification.
- [ ] Deliverable 6: the D.21 contract-surface inventory is defined in `sc_observability_types::otlp`; `sc-observability-otlp` re-exports or consumes it, and each concrete exporter implementation remains in its behavior-owning crate.
- [ ] Interim placement now: `git show origin/fix/obs-d-21-qa-f2:crates/sc-observability-otlp/src/contracts.rs > crates/sc-observability-types/src/otlp/mod.rs`; the diff of the copied file against that source is empty except for the permitted `use`-path and visibility edits.
- [ ] Interim placement later: `git mv crates/sc-observability-types/src/otlp/ crates/sc-observability-otlp-types/src/`; update `Cargo.toml` and the one `crate::contracts` re-export line, with no consumer changes beyond that seam.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass as the lead's intermediate-workspace invariant.
