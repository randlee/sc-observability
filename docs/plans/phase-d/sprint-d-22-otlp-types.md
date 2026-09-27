# d-22: Shared OTLP types crate

Generated projection of `obs-d-22`; the bead is authoritative.

## Plan metadata

- Wave: 1.5
- Layer: 11
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability-otlp-types shared OTLP wire model
- Branch: `sprint/d-22-otlp-types`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-22-otlp-types`
- PR target (merge order only): `sprint/d-6-otlp-lifecycle-core`
- Blocked by: `obs-d-21-sanity`
- Requirements: LAY-001, LAY-005, NFR-004, NFR-009, OTLP-003, OTLP-010, PHD-003, TYP-002, TYP-009, TYP-010, TYP-030
- ADRs: ADR-002, ADR-004, ADR-005, ADR-009, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-otlp-types/**`
  - `boundaries/sc-observability-otlp-types/**`
  - `Cargo.toml`
  - `Cargo.lock`
  - `policy/otlp-transport.toml`
  - `crates/sc-observability-otlp/Cargo.toml`
  - `docs/plans/phase-d/sprint-d-22-otlp-types.md`

## Goal

Create `sc-observability-otlp-types`, the one OTLP wire data model shared by the obs-d-7 and obs-d-8 transports, and its lossless move-only projection from the obs-d-12 neutral v2 records.

## Deliverables

1. Create `crates/sc-observability-otlp-types` (manifest, `lib.rs`, `constants.rs`), register it as a workspace member, add `boundaries/sc-observability-otlp-types/otlp-types.toml`, and apply the dependency declarations of the D.21 amendment (2026-09-27) to the root manifest, `Cargo.lock`, `policy/otlp-transport.toml` and the `sc-observability-otlp` manifest. The crate depends only on `sc-observability-types`, `opentelemetry-proto =0.33.0` and `prost =0.14.4`; only `sc-observability-otlp` depends on it.

2. Define the data model on `opentelemetry-proto` 0.33 messages directly, or on `#[repr(transparent)]` newtypes that map 1:1 to them; it follows upstream, not the sc crates, so an upstream bump or a switch to upstream types is mechanical. Per signal, one resource group (`OtlpLogs`, `OtlpSpans`, `OtlpMetrics` over `ResourceLogs`, `ResourceSpans`, `ResourceMetrics`) is the exporter batch element. Callers build logs, spans and metrics directly in this representation.

3. Own the only conversion: a lossless neutral→proto projection that takes its inputs by value and moves them, never cloning attribute, event or link data. Inputs are the `sc-observability-types` records: `LogEvent` with its trace flags and attributes; `SpanRecord<SpanEnded>` with its ordered `SpanEvent`s and links, represented by this crate's `CompleteSpan`; and `MetricRecord` including histogram buckets/count/sum, temporality and start time; plus resource, scope and trace context. Grouping by resource moves each record into its `ResourceLogs`/`ResourceSpans`/`ResourceMetrics`. The projection is infallible over validated neutral types and defines no error variant or code; non-trivial constants live in `constants.rs`.

4. Add lossless tests: every projected field is asserted, and a prost encode/decode round trip is equal, for mixed-resource batches (group order is first appearance, record order kept within a group), span events and links, histogram buckets/count/sum with temporality and start time, and trace/span ids and flags.

5. Prove zero copy: a test builds a record once, projects and groups it, and asserts the heap buffers of its attribute, event and link data are the same allocations inside the resulting resource group. Crate docs list every clone a signature forces, with its justification; the list is empty unless a signature forces one.

6. Move the shared D.21 OTLP contract surface into this crate. The `git grep` inventory is `origin/fix/obs-d-21-qa-f2:crates/sc-observability-otlp/src/contracts.rs` (that file is absent from `origin/integrate/phase-d`): structs `Resource`, `InstrumentationScope`, `ExportRecord<T>`, `LogRecord`, `CompleteSpan`, and `ExporterSet<L, S, M>`; the `LifecycleFuture` companion alias; and public traits `ExporterLifecycle`, `LogExporter<T>`, `TraceExporter<T>`, and `MetricExporter<T>`. `sc-observability-otlp` re-exports or consumes that surface. This crate owns the definitions but no implementations: each behavior-owning crate implements the traits it needs—`sc-observability-otlp` for its facade behavior, obs-d-7 for the Tokio transport, obs-d-8 for the blocking transport, and any later behavior owner likewise—and depends on `sc-observability-otlp-types` for the traits.

## This Sprint Does Not Close

No transport, client, runtime, exporter trait implementation or JSON layer: obs-d-7 sends the model over tonic/reqwest, obs-d-8 over its blocking OTLP/JSON client. Public exporter trait definitions and `ExporterSet` are in scope here, but implementations are not: they stay with the crate that owns the behavior. D.12 owns the neutral types and errors; D.21 owns config; D.6 owns lifecycle behavior; D.18 composes the facade.

## Design

## Dependency fence

`opentelemetry-proto =0.33.0` with default features off and exactly `gen-tonic-messages`, `logs`, `trace`, `metrics` and `with-serde`. `gen-tonic` (the generated tonic clients) is not enabled here; this crate has no HTTP or gRPC client, no Tokio and no `opentelemetry-otlp`. `opentelemetry` and `opentelemetry_sdk` enter only as `opentelemetry-proto`'s non-optional default-features-off dependencies. Its sole `sc-*` dependency is `sc-observability-types`; that crate gains no dependency (ADR-019).

The shared files `Cargo.toml`, `Cargo.lock`, `policy/otlp-transport.toml` and `crates/sc-observability-otlp/Cargo.toml` are owned here only for the declarations the D.21 amendment names; D.21's existing entries are unchanged.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-21 (wave 1)

Consume D.21's sanity-gated workspace, manifests and transport policy under its 2026-09-27 amendment. Move the shared contract surface out of D.21's `contracts.rs`; `sc-observability-otlp` then re-exports or consumes the D.22-owned definitions, while backends instantiate `ExporterSet<L, S, M>` with this crate's resource-group types.

## Handoff to obs-d-7 and obs-d-8 (wave 2)

obs-d-7 and obs-d-8 consume the crate read-only after obs-d-22-sanity. They take `OtlpLogs`/`OtlpSpans`/`OtlpMetrics` batches by value or by reference and encode them straight to the wire (obs-d-7 protobuf over tonic/reqwest, obs-d-8 OTLP/JSON) with no intermediate model and no per-record conversion. Each implements its owned transport behavior against this crate's public traits; neither backend adds a second projection or moves a trait implementation into `sc-observability-otlp-types`.

## Handoff to obs-d-18 (wave 3)

obs-d-18 composes the facade emit path on this projection, moving neutral records into resource groups before admission.

## Acceptance criteria

- [ ] Deliverable 1: `cargo tree -p sc-observability-otlp-types -e normal` shows no `tonic`, `reqwest`, `tokio` or `opentelemetry-otlp`; `bash scripts/ci/validate_repo_boundaries.sh` and `bash scripts/ci/validate_dependency_bans.sh` pass.
- [ ] Deliverables 2–4: `cargo test -p sc-observability-otlp-types --locked` runs nonzero lossless cases for logs, spans (events and links) and metrics (histogram, temporality, start time), including a mixed-resource batch and a prost round trip.
- [ ] Deliverable 5: the zero-copy test passes and the crate docs name every forced clone with its justification.
- [ ] Deliverable 6: the D.21 contract-surface inventory is defined in `sc-observability-otlp-types`; `sc-observability-otlp` re-exports or consumes it, and each concrete exporter implementation remains in its behavior-owning crate.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass as the lead's intermediate-workspace invariant.
