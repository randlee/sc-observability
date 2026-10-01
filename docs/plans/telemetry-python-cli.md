# Phase D wave 5: customer telemetry submission (Python and CLI)

Phase D wave 5 runs on `integrate/phase-d`. Its sprints are d-29, d-33, d-30,
d-31 and d-32. Deliverables, acceptance criteria, owned paths and validation
live only in the sprint docs:

| Sprint | Doc |
| --- | --- |
| d-29 | [sprint-d-29-telemetry-submission-contract.md](phase-d/sprint-d-29-telemetry-submission-contract.md) |
| d-33 | [sprint-d-33-durable-store-and-export.md](phase-d/sprint-d-33-durable-store-and-export.md) |
| d-30 | [sprint-d-30-python-telemetry-bindings.md](phase-d/sprint-d-30-python-telemetry-bindings.md) |
| d-31 | [sprint-d-31-sc-otel-cli.md](phase-d/sprint-d-31-sc-otel-cli.md) |
| d-32 | [sprint-d-32-sanity-telemetry-e2e.md](phase-d/sprint-d-32-sanity-telemetry-e2e.md) |

This document holds the design rationale, the wave-5 boundary map, the wave
table and the lead rulings. Governing records are ADR-021 and PHD-005–013.

## Outcome

A customer maps their data once into logs, completed spans, metrics and
profiles. They submit through the Python package or the `sc-otel` CLI. Both
call the same Rust code for validation, correlation, durable admission, OTLP
export and lifecycle handling. The first consumer is the repository's LLM/JEV
sanity history and QA logs.

## Base and consumed artifacts

The sequence is fixed by the user. First, PR #788 (`feat/qa-sanity-telemetry-config`,
which adds `.sc/telemetry.yaml`) and this plan land in `develop`. Then `develop`
merges into `integrate/phase-d`. Then wave 5 runs on `integrate/phase-d`.
`.sc/telemetry.yaml` is therefore present on `integrate/phase-d` before
d-29 dispatches. Its schema is specified in the d-32 doc.

What wave 5 consumes from the `integrate/phase-d` tree:

- `sc-observability-types`: v2 `MetricValue::{Gauge, Sum, Histogram}`,
  `HistogramPoint`, `AggregationTemporality`, `AttributeValue` (with `UInt`, no
  bytes variant), `TraceContext` (no `trace_state`), `SpanKind`, `SpanLink`,
  `TraceFlags`, `FiniteF64`, and the `sc_observability_types::otlp` module with
  `OtlpResource`, `OtlpInstrumentationScope` and `OtlpRecord<T>`. This module
  is the interim home for shared OTLP structs (user ruling 2026-09-27). The
  `sc-observability-otlp-types` crate does not exist on `integrate/phase-d`
  yet; d-29 creates it.
- `sc-observability-otlp`: `RuntimeTelemetry` (`v2::Telemetry`), `OtelConfig`
  with `sync_http_retry: Option<SyncHttpRetryPolicy>`, and
  `ExporterBackend::{OpenTelemetrySdk, SyncHttp}`. It also has the `sync-http`
  feature and `sync_http` module (a plain-thread OTLP/HTTP JSON encoder posting
  to `/v1/{signal}`), the `otlp-sdk` feature, and bounded record/byte
  admission credits (`contracts/credits.rs`).
- Python binding errors carry the `native_operation` wire label.
- The pinned viewer is `otel-desktop-viewer` v0.5.0 (darwin_arm64),
  installed and driven by `scripts/ci/fixtures/otlp/desktop-viewer/` and the
  `desktop-viewer-factory-conformance` job in `.github/workflows/otlp-conformance.yml`.
  See `docs/observability/otlp/local-viewer.md`.
- d-26 supplies the compatible OTLP config adapters, and d-28 supplies the
  release/compat baseline that d-29 builds its additive API on. d-29 depends
  on both through the frozen DAG edge `["d-26","d-28"]`.

## Design rationale

- **One contract, two front ends.** The front ends do not encode OTLP. They
  convert caller input (a Python dict or CLI JSON/flags) into
  `SubmissionInput`. Shared Rust (`SubmissionEnvelope::from_input` in
  `sc-observability-otlp-types`) validates, correlates and canonicalizes it.
  So Python and the CLI cannot diverge on validation.
- **Proto-shaped neutral records.** New neutral record types in
  `sc_observability_types::otlp::signals` mirror the pinned
  opentelemetry-proto messages field for field. Existing `LogEvent`,
  `SpanRecord<SpanEnded>` and v2 `MetricRecord` convert into them through
  `From` impls. No released type gains a field or variant.
- **Durability before receipt.** `emit` commits a versioned envelope to the
  local SQLite store before it returns an `AdmissionReceipt`. A drain worker
  delivers each signal independently through the sync-http backend's bounded
  admission. Delivery is at least once.
- **No Tokio in front ends.** The durable-store drain uses sync-http only. The
  SDK path keeps its current instrument-based scope. Unsupported
  backend × signal × representation combinations return typed construction
  errors.
- **Out of scope.** Dashboards, remote configuration, a general mapping DSL,
  a profiler, and Grafana testing. Customer field mapping lives in the
  consumer (d-32 importer), not in the transport.
- **Events and baggage.** Stats are metrics. Events are log records with
  `event_name`, or span events. Baggage is context, not a signal; it is not
  copied into attributes.

## Wave-5 boundary map

| Crate / manifest | Contract delta | `allowed_dependencies` / `allowed_dependents` edits |
| --- | --- | --- |
| `sc-observability-types` (`boundaries/sc-observability-types/types.toml`) | New module `otlp::signals`: `AnyValue` (with bytes), `KeyValues`, `Resource`, `InstrumentationScope`, `ResourceRecord<T>`, `TraceState`, `LogPoint`, `SpanPoint`, `MetricStream`/`MetricData` and the five point forms, `Exemplar`, profile dictionary and `Profile`; `From` conversions from existing types. Declared in `src/otlp/mod.rs`, not `src/lib.rs`. | `allowed_dependents` += `sc-observability-otlp-types` |
| `sc-observability-otlp-types` (new; `boundaries/sc-observability-otlp-types/otlp-types.toml`) | `SubmissionInput`, `SubmissionEnvelope` (with `version`), `RecordKey`, `SubmissionId`, `AdmissionReceipt`, `DeliveryStatus`, `StoreStatus`, `FlushReport`, the error enums and their codes, `TelemetryClientConfig` plus precedence resolver, the `TelemetryClient` trait, the capability matrix types, the `InMemoryTelemetryClient` test double (feature `test-double`) and the golden fixtures. Published crate. | `allowed_dependencies` = [`sc-observability-types`]; `allowed_dependents` = [`sc-observability-otlp`, `sc-observability-py`, `sc-otel-cli`] |
| `sc-observability-otlp` (`boundaries/sc-observability-otlp/otlp.toml`) | New feature `durable-store` = [`sync-http`, `dep:rusqlite`, `dep:sc-observability-otlp-types`]. New public module `durable` (`DurableTelemetryClient`, which implements `TelemetryClient`). The store schema DDL, the drain worker, and sync-http encoders for the new point forms and `/v1development/profiles`. | `allowed_dependencies` += `sc-observability-otlp-types`; `allowed_dependents` += `sc-observability-py`, `sc-otel-cli`; `policy/otlp-transport.toml` += `[transport.rusqlite]` (`=0.40.2`, features `["bundled"]`, default features off, backend `durable-store`) |
| `sc-observability-py` (`boundaries/sc-observability-py/python.toml`) | New `otlp-telemetry` Cargo feature = [`dep:sc-observability-otlp`, `dep:sc-observability-otlp-types`, `sc-observability-otlp/durable-store`]. New Python `sc_observability.telemetry` module. Release wheels enable the feature through `[tool.maturin] features`. | `allowed_dependencies` += `sc-observability-otlp`, `sc-observability-otlp-types` |
| `sc-otel-cli` (new; `boundaries/sc-otel-cli/cli.toml`) | Binary `sc-otel`: `emit`, `validate`, `flush`, `status`. Exit-code table and JSON output schema `sc-otel.result/v1`. Workspace member with `publish = false`. | `allowed_dependencies` = [`sc-observability-otlp-types`, `sc-observability-otlp`]; `forbidden_edges` = [`sc-observe`, `pyo3`, `agent-team-mail-*`]; `allowed_dependents` = [] |
| `scripts/sanity-telemetry/` (not a crate) | Python importer that consumes the installed `sc_observability.telemetry` API. | none (Python package dependency only) |

## Wave table

One track; the wave-5 stack is ordered d-29 → d-33 → d-30 → d-31 → d-32.

| Wave | Sprint | Closure | Target boundary | Owned paths (summary; sprint doc is authoritative) |
| --- | --- | --- | --- | --- |
| 5.1 | d-29 | contract | `BOUNDARY-ScObservabilityOtlpTypes` (wave-5 contract) | `crates/sc-observability-otlp-types/**`, `crates/sc-observability-types/src/otlp/**`, `crates/sc-otel-cli/Cargo.toml`, stubbed `crates/sc-otel-cli/src/main.rs`, manifests, `Cargo.toml`, `Cargo.lock`, `policy/otlp-transport.toml`, `boundaries/**` wave-5 rows, `crates/sc-observability-otlp/Cargo.toml`, `crates/sc-observability-otlp/src/lib.rs` (one registration hunk), the staged `durable/` stub and `schema.sql`, `bindings/python/sc-observability-py/Cargo.toml`, ADR-021/PHD text |
| 5.2 | d-33 | boundary (implementer) | `BOUNDARY-ScObservabilityOtlp` | `crates/sc-observability-otlp/src/durable/**` except `schema.sql`, `crates/sc-observability-otlp/src/sync_http/**`, `crates/sc-observability-otlp/tests/durable_*.rs`, `crates/sc-observability-otlp/tests/submission_*.rs` |
| 5.2 | d-30 | boundary (consumer) | `BOUNDARY-ScObservabilityPy` | `bindings/python/sc-observability-py/**` except `Cargo.toml` and `python/sc_observability/generated/**` |
| 5.2 | d-31 | boundary (consumer) | `BOUNDARY-ScOtelCli` | `crates/sc-otel-cli/src/**`, `crates/sc-otel-cli/tests/**` |
| 5.3 | d-32 | integration | wave-5 composition | `scripts/sanity-telemetry/**`, `tests/telemetry-e2e/**`, `.github/workflows/telemetry-e2e.yml`, `.sc/telemetry.yaml`, `docs/telemetry-submission.md`, release-inventory row and approval record (see d-32) |

- Tracks: 1. Waves: 3. Critical path: 3 (d-29 → d-33 → d-32, or through
  d-30 or d-31). Width: 3 (d-33 ∥ d-30 ∥ d-31). Sprint count: 5.
- Edges: d-29 ← {d-26, d-28}; {d-33, d-30, d-31} ← d-29;
  d-32 ← {d-30, d-31, d-33}. All wave-5.2 pairs are `parallel_safe`, because
  their owned paths are disjoint.
- Recommended agents: d-29 aobs/astra (hard: contract breadth). d-33
  cobs/terra (normal). d-30 cobs/terra (normal). d-31 lobs/luna (fast:
  thin consumer over a fixed contract). d-32 cobs/terra (normal).

## Lead rulings (2026-10-01)

These are lead decisions dated 2026-10-01 and are binding on the sprint docs.

- **R1 Base.** Sequence: #788 and this plan land in `develop`, then `develop`
  merges into `integrate/phase-d`, then wave 5 runs on `integrate/phase-d`.
  Existing code is described against that tree. `.sc/telemetry.yaml` from #788
  is a consumed artifact. Architecture §6 allowlist text and PHD-003 use the
  landed `sync-http` names.
- **R2 Split d-29.** d-29 is the contract sprint (wave 5.1). d-33 is the
  store/drain/export implementer (wave 5.2). d-30 and d-31 are consumers built
  against the d-29 test double and golden fixtures (wave 5.2). d-32 is
  integration (wave 5.3). sprints.jsonl: d-29 keeps `["d-26","d-28"]`;
  d-30, d-31 and d-33 depend on `["d-29"]`; d-32 depends on
  `["d-30","d-31","d-33"]`.
- **R3 Contract placement.** Shared submission contracts live in
  `sc-observability-otlp-types`. Store, drain and export live in
  `sc-observability-otlp` behind the `durable-store` feature.
- **R4 Backend.** The durable-store drain uses sync-http only. The SDK/Tokio
  path keeps its instrument-based scope. ADR-021 carries the backend × signal
  × representation matrix. Unsupported combinations are typed
  construction-time errors. Profiles are encoded by sync-http to
  `/v1development/profiles`.
- **R5 Storage engine.** SQLite via `rusqlite` with `bundled`, behind
  `durable-store`. ADR-021 is Accepted. Layering is store → drain worker →
  bounded backend admission. Multi-process ownership uses a lease plus row
  claims, with at-least-once delivery.
- **R6 CLI.** The binary is `sc-otel`, from crate `sc-otel-cli` at
  `crates/sc-otel-cli`. It is a workspace binary, not published this phase.
  Tests install it with `cargo install --path crates/sc-otel-cli --root <tmp>`.
- **R7 Python.** The Cargo feature is `otlp-telemetry`, enabled in release
  wheels. d-30 owns the wheel build config. The calls that release the GIL are
  listed in d-29.
- **R8 Profiles.** Profiles are first class in PHD-005, PHD-010 and ADR-021.
- **R9 D18/D9.** No new edges to d-18 or d-9. d-32 re-runs the semver/compat
  (D18) gate and the D9/viewer conformance gate over the wave-5 additions.
- **R10 Proof.** Cross-front-end equivalence is in d-32. d-30 and d-31 assert
  against the d-29 golden fixtures. Per-variant encoding round trips with
  loopback capture are in d-33. Viewer proof is automated readback; manual UI
  inspection is never proof.
- **R11 Importer.** It has its own path `scripts/sanity-telemetry/**`,
  separate PHD-011 deliverables, a field mapping table and a checkpoint path.
  Direct CLI emission is a named d-31 acceptance criterion.

### Provisional choices needing user decision

1. **`sc-observability-otlp-types` does not exist on `integrate/phase-d`.**
   R3 assumes it does. Provisional choice: d-29 creates the crate holding only
   the new submission contracts. The existing interim
   `sc_observability_types::otlp` module stays where it is, and its later
   extraction remains the separate mechanical refactor from the 2026-09-27
   ruling. Because `sc-observability-otlp` (published) gains an optional
   dependency on it, the new crate must be published with the next release.
   d-32 adds its `release/publish-artifacts.toml` row.
2. **Shared files with in-flight d-18.** d-18's fence includes
   `crates/sc-observability-otlp/src/lib.rs`, `release/**` and
   `docs/api-approvals/**`. Wave 5 touches them only additively: d-29 adds one
   `#[cfg(feature = "durable-store")] pub mod durable;` hunk to `lib.rs`; d-32
   adds one publish row and one new approval record file. If d-18 is still
   open when those sprints run, they merge forward d-18's pushed head before
   each round. No DAG edge is added (R9).
3. **Disk-bound policy default.** `DiskBoundPolicy::RejectNew` is the default:
   `emit` returns `AdmissionError::DiskBoundExceeded`. `EvictOldest` is
   opt-in, counted and shown in status. This keeps PHD-007 ("no silent
   eviction") while allowing the R5 bounded-eviction mode.
4. **YAML parser.** `serde_yaml` is deprecated upstream. d-29 pins a
   maintained parser (candidate `serde-saphyr`) that passes `cargo deny check`.
5. **Proto exclusions.** The d-29 inventory marks a short list of fields
   "excluded — needs user approval" (non-finite doubles, `*_strindex`
   encodings, `tracez.proto`). Until approved, those inputs are rejected with
   typed errors rather than silently dropped.
6. **Public API approval record.** The wave-5 public additions need a
   `docs/api-approvals/` record with an actual reviewer. d-32 prepares it, and
   the user signs it.

References: [signals](https://opentelemetry.io/docs/concepts/signals/),
[metric data model](https://opentelemetry.io/docs/specs/otel/metrics/data-model/),
[profiles](https://opentelemetry.io/docs/concepts/signals/profiles/).
