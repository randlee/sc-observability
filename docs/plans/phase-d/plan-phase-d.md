> **Release-policy supersession:** This document preserves the historical Phase D projection. Its 2.0 activation, compatibility-removal and breaking-approval instructions are superseded by [the compatible 1.x amendment](compatible-1x-amendment.md) and the amended authoritative beads. Do not execute those historical instructions.

# Phase D plan

Generated projection of `obs-phase-d`; beads are authoritative.

## Phase D — logging, OTLP, and Python distribution

The plan has twenty dev sprints in waves 1–4 plus five wave-5 sprints (obs-d-29, obs-d-33, obs-d-30, obs-d-31, obs-d-32; see "Wave 5"), twenty-five in all. obs-d-12 is the types root; obs-d-21 consumes it as the OTLP contract stage. Types-only consumers release after obs-d-12-sanity, while obs-d-5–8 release after obs-d-21-sanity. obs-d-11 remains folded into obs-d-10. Beads are authoritative; documents project them. Concrete artifact gates determine execution; layer/pr_target records merge order only. One append-only phase stack targets integrate/phase-d for waves 1–4; wave 5 is its own stack on integrate/phase-d (wave-5 ruling R12).

## Sprint and wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | aobs/astra | root | 1 | sc-observability-types 2.0 contract |
| 1 | obs-d-21 | lobs/luna | must_follow | 2 | sc-observability-otlp contract and workspace registration |
| 1 | obs-d-13 | cobs/terra | root | 3 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 4 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 5 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 6 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 7 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 8 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 9 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 10 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 11 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 12 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 13 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 14 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 15 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 16 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 17 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 18 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 19 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 20 | OTLP dual-path qualification |
| 5.1 | obs-d-29 | aobs/astra | must_follow | wave-5 stack 1 | wave-5 telemetry submission contract (`sc_observability_types::otlp`) |
| 5.2 | obs-d-33 | aobs/astra | must_follow | wave-5 stack 2 | sc-observability-otlp durable store and export |
| 5.2 | obs-d-30 | cobs/terra | must_follow | wave-5 stack 3 | sc-observability-py telemetry bindings |
| 5.2 | obs-d-31 | cobs/terra | must_follow | wave-5 stack 4 | sc-otel-cli |
| 5.3 | obs-d-32 | cobs/terra | must_follow | wave-5 stack 5 | wave-5 composition (sanity telemetry e2e) |

## Boundary map and execution graph

obs-d-12 freezes canonical errors/signals and wire projection specifications; obs-d-21 consumes them to freeze OTLP config/lifecycle contracts and workspace registration. obs-d-13 independently freezes logging shapes/signatures with baseline-only private fixtures. obs-d-1–8/14–17 consume their relevant contract gates and implement bounded modules. obs-d-19 owns DTO/schema/generated models; obs-d-20 owns language extraction/transport adapters. They consume the same frozen wire contract and close independently using local fixtures; integration supplies real cross-layer composition. obs-d-18 activates the completed library and owns release/API closure; obs-d-9 qualifies real collectors. obs-d-10 independently closes native Windows ARM64 preparation and the folded open-ended Python guard/artifact proof, handing the release policy specification/evidence to obs-d-18.

## Rulings

1. obs-d-12 owns shared types/error/model source, its Cargo manifest, dedicated non-OTLP registry/constants files and shared normative architecture/requirements/API-design documents. obs-d-21 owns root Cargo.toml/Cargo.lock, other workspace manifests/features, OTLP module roots/stubs, boundary allowlists and atomic workspace version activation. The SDK example Cargo.toml stays with obs-d-7; project-plan.md stays with obs-d-18.
2. obs-d-12 publishes the reviewed normative specification and section6 allowlist as read-only input to obs-d-21; OTLP exact implementation pins live in obs-d-21 manifests/lock/boundaries. obs-d-12 owns all ConfigFailure/ExportError variants and types error_codes.rs::otlp; obs-d-21 supplies re-exports and validation/conversion only. obs-d-21 creates every declared OTLP file before handing assembly/projector/lifecycle/backend implementation stubs to obs-d-5–8. Module roots remain read-only to implementations.

3. **ADR-017/018 acceptance: accepted 2026-09-26 via user merge of PR #225**. ADR-012 is superseded only for enumerated 2.0 breaks. PHB-003/004/005 are explicitly 1.x; PHD-001–004 define the reviewed 2.0 requirements. **ADR-019 acceptance: accepted 2026-09-26 via PR #227**. The separate retrospective acceptance of ADR-011–016 is treated as Accepted per the lead ruling; no new decision is hidden in those historical records.
4. obs-d-18 alone owns release/**, API approvals, changelog/release notes, migration guidance, final semver baseline and wrapper/classification/adapter removal. Migration beads change call sites and tests only. DTO/schema/generated output belongs to obs-d-19; Python/TS/Tauri adapter implementation belongs to obs-d-20. Their wave-2 fences are disjoint and paired sanity checks gate obs-d-18.
5. PLAN-SCOPE-016 lead ruling: obs-d-13's final concrete signatures are specifications; compiled wave-1 fixtures use private error-parameterized harnesses with baseline types only. No new obs-d-12 error or registry imports occur there. obs-d-1/2/3 bind both contract artifacts in owned runtime/bridge/builder files in wave 2; obs-d-18 activates typed.rs exports. No edge between roots.
6. Same-wave ownership overlaps must be zero. Cross-wave path/artifact handoffs are named by both producer and consumer. Shared files are not a reason to serialize otherwise independent wave-2 work. The two new beads are must_follow consumers of the frozen contract; the stack chain is not an execution edge.
7. **wave 4: accepted 2026-09-26T06:28Z by the lead under user-delegated authority**. The four-stage baseline was accepted; the subsequent scoped obs-d-12/21 split produces the measured five-sprint path recorded below. obs-d-9 remains its own conformance/qualification sprint after obs-d-18 with its CI workflow, fixtures and Grafana smoke; folding it into integration would widen that already bounded sprint. obs-d-11 is folded into obs-d-10 and no longer a fourth-wave bead.

8. Scoped split authorized by the user; no plan-review round is restarted. Lead ruling 01M3E7Z46AA9DNNMC73BDE37JG assigns obs-d-12 to aobs and obs-d-21 to lobs. Numbered wave 1 contains independent roots obs-d-12/13/10 plus obs-d-21 as a second contract stage following obs-d-12-sanity. Only the four OTLP implementation consumers (obs-d-5–8) move from types sanity to OTLP sanity; obs-d-18 explicitly waits on obs-d-21-sanity too. Layers 1..20 are unique; layer2 targets the types branch, layer3 targets the OTLP-contract branch.
9. Version-line exception: the types manifest fence stays with obs-d-12, which stages the v2 API at the current Cargo version. A handoff in both designs permits obs-d-21 to edit only the types manifest version literal during the atomic workspace bump. This explicit exception is serialized by obs-d-12-sanity and does not duplicate path fences. Shared normative documents remain read-only to obs-d-21.

## Workspace invariant and replace-versus-coexist sequence

Every sprint closes with `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` green. obs-d-12 introduces canonical errors_v2.rs/signals_v2.rs under an explicit v2 path alongside functioning 1.x exports at the current Cargo version. obs-d-21 later activates all workspace 2.0 versions atomically using the explicit types-version-line handoff; types-only consumers need not wait for that activation. Implementations migrate their call sites using temporary compatibility; they do not remove wrappers, classifiers or adapters. obs-d-18 switches canonical public exports and alone removes compatibility after all implementation sanity gates. This is temporary sequencing, not the final API: ADR-017 replacement remains the release contract. Private fixtures and module stubs are contract artifacts only; enabled production construction never reports success through a no-op. obs-d-18 additionally runs all-features release tests, real combined bindings and semver/removal gates. This invariant is stated once here; sprint criteria reference it.

## Parallelism after correction

20 dev sprints and four numbered waves; numbered wave 1 has two dependency stages. Measured critical path is five dev sprints: obs-d-12 → obs-d-21 → obs-d-7 → obs-d-18 → obs-d-9 (sanity/plan-QA gates excluded). Do not label it four merely because both contracts are in wave 1. The scoped split releases seven direct types-contract consumers independently of OTLP work: obs-d-4/14/15/16/17/19/20; obs-d-17 also retains its logging-contract gate. obs-d-1/2/3 keep their existing logging-contract gates. Wave 2 has fourteen beads; maximum dependency-independent width remains fifteen with root obs-d-10 alongside released implementations. The useful gain is earlier release of types-only work, not a claimed shorter OTLP critical path.

## Wave table

| Wave | Bead | Assignee/model | Relation | Layer | Target boundary |
| --- | --- | --- | --- | --- | --- |
| 1 | obs-d-12 | aobs/astra | root | 1 | sc-observability-types 2.0 contract |
| 1 | obs-d-21 | lobs/luna | must_follow | 2 | sc-observability-otlp contract and workspace registration |
| 1 | obs-d-13 | cobs/terra | root | 3 | logging contract |
| 1 | obs-d-10 | lobs/luna | root | 4 | Windows ARM64 Python distribution and open-ended guard |
| 2 | obs-d-1 | cobs/terra | must_follow | 5 | sc-observability logging settings implementation |
| 2 | obs-d-2 | lobs/luna | must_follow | 6 | sc-observability-log bridge module |
| 2 | obs-d-3 | cobs/terra | must_follow | 7 | sc-observability typed sink module |
| 2 | obs-d-4 | lobs/luna | parallel_safe | 8 | sc-observability error migration |
| 2 | obs-d-5 | cobs/terra | must_follow | 9 | OTLP signal projectors |
| 2 | obs-d-6 | lobs/luna | must_follow | 10 | OTLP lifecycle module |
| 2 | obs-d-7 | cobs/terra | must_follow | 11 | OTLP SDK adapter module |
| 2 | obs-d-8 | lobs/luna | must_follow | 12 | OTLP HTTP JSON module |
| 2 | obs-d-14 | cobs/terra | parallel_safe | 13 | sc-observe |
| 2 | obs-d-15 | lobs/luna | parallel_safe | 14 | sc-observability-binding-runtime |
| 2 | obs-d-16 | cobs/terra | parallel_safe | 15 | sc-observability-log error migration |
| 2 | obs-d-17 | lobs/luna | parallel_safe | 16 | log consumer migration |
| 2 | obs-d-19 | cobs2/terra | must_follow | 17 | neutral DTO/schema and generated models |
| 2 | obs-d-20 | lobs2/luna | must_follow | 18 | Python and TypeScript/Tauri adapters |
| 3 | obs-d-18 | cobs/terra | must_follow | 19 | phase integration |
| 4 | obs-d-9 | cobs/terra | must_follow | 20 | OTLP dual-path qualification |
| 5.1 | obs-d-29 | aobs/astra | must_follow | wave-5 stack 1 | wave-5 telemetry submission contract (`sc_observability_types::otlp`) |
| 5.2 | obs-d-33 | aobs/astra | must_follow | wave-5 stack 2 | sc-observability-otlp durable store and export |
| 5.2 | obs-d-30 | cobs/terra | must_follow | wave-5 stack 3 | sc-observability-py telemetry bindings |
| 5.2 | obs-d-31 | cobs/terra | must_follow | wave-5 stack 4 | sc-otel-cli |
| 5.3 | obs-d-32 | cobs/terra | must_follow | wave-5 stack 5 | wave-5 composition (sanity telemetry e2e) |

## Wave 5: customer telemetry submission (Python and CLI)

Wave 5 runs on `integrate/phase-d` as its own stack (ruling R12). Its
sprints are d-29, d-33, d-30, d-31 and d-32. Deliverables, acceptance
criteria, owned paths and validation live only in the sprint docs:

| Sprint | Doc |
| --- | --- |
| d-29 | [sprint-d-29-telemetry-submission-contract.md](sprint-d-29-telemetry-submission-contract.md) |
| d-33 | [sprint-d-33-durable-store-and-export.md](sprint-d-33-durable-store-and-export.md) |
| d-30 | [sprint-d-30-python-telemetry-bindings.md](sprint-d-30-python-telemetry-bindings.md) |
| d-31 | [sprint-d-31-sc-otel-cli.md](sprint-d-31-sc-otel-cli.md) |
| d-32 | [sprint-d-32-sanity-telemetry-e2e.md](sprint-d-32-sanity-telemetry-e2e.md) |

This section holds the design rationale, the wave-5 boundary map, the wave
table and the wave-5 lead rulings (R1–R17, P1–P6). Governing records are
ADR-021 and PHD-005–013. Wave 5 adds five dev sprints to the twenty of
waves 1–4.

### Outcome

A customer maps their data once into logs, completed spans, metrics and
profiles. They submit through the Python package or the `sc-otel` CLI. Both
call the same Rust code for validation, correlation, durable admission, OTLP
export and lifecycle handling. The first consumer is the repository's LLM/JEV
sanity history and QA logs.

### Base and consumed artifacts

The sequence is fixed by the user. First, PR #788 (`feat/qa-sanity-telemetry-config`,
which adds `.sc/telemetry.yaml`) and the wave-5 plan land in `develop`. Then `develop`
merges into `integrate/phase-d`. Then wave 5 runs on `integrate/phase-d`.
`.sc/telemetry.yaml` is therefore present on `integrate/phase-d` before
d-29 dispatches. Its schema is specified in the d-32 doc.

What wave 5 consumes from the `integrate/phase-d` tree:

- `sc-observability-types`: v2 `MetricValue::{Gauge, Sum, Histogram}`,
  `HistogramPoint`, `AggregationTemporality`, `AttributeValue` (with `UInt`, no
  bytes variant), `TraceContext` (no `trace_state`), `SpanKind`, `SpanLink`,
  `TraceFlags`, `FiniteF64`, and the `sc_observability_types::otlp` module with
  `OtlpResource`, `OtlpInstrumentationScope` and `OtlpRecord<T>`. This module
  is the interim home for shared OTLP structs (user ruling 2026-09-27). No
  `sc-observability-otlp-types` crate exists on any branch, and wave 5 adds
  none (lead ruling P1): the submission contract goes into this module.
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

### Design rationale

- **One contract, two front ends.** The front ends do not encode OTLP. They
  convert caller input (a Python dict or CLI JSON/flags) into
  `SubmissionInput`. Shared Rust (`SubmissionEnvelope::from_input` in
  `sc_observability_types::otlp::submission`) validates, correlates and canonicalizes it.
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

### Wave-5 boundary map

| Crate / manifest | Contract delta | `allowed_dependencies` / `allowed_dependents` edits |
| --- | --- | --- |
| `sc-observability-types` (`boundaries/sc-observability-types/types.toml`) | New module `otlp::signals`: `AnyValue` (with bytes and `StringIndex`), `AttributeKey`, `KeyValues`, `OtlpDouble` (NaN/±Inf), `Resource`, `InstrumentationScope`, `ResourceRecord<T>`, `TraceState`, `LogPoint`, `SpanPoint`, `MetricStream`/`MetricData` and the five point forms, `Exemplar`, profile dictionary and `Profile`; `From` conversions from existing types. New module `otlp::submission`: `SubmissionInput`, `SubmissionEnvelope` (with `version`), `RecordKey`, `SubmissionId`, `AdmissionReceipt`, `DeliveryStatus`, `StoreStatus`, `FlushReport`, the error enums and their codes, `TelemetryClientConfig`, `TelemetryFileConfig` and the precedence resolver, the `TelemetryClient` trait, the capability matrix types, and `InMemoryTelemetryClient` plus the conformance suite (new feature `test-double`). Both modules are declared in `src/otlp/mod.rs`, not `src/lib.rs`. One new optional dependency: `uuid` under `test-double` (d-29 "Dependency set"). | `allowed_dependents` += `sc-otel-cli`; `allowed_dependencies` += `uuid` (optional); `allowed_test_double_paths` += `crates/sc-observability-types/src/otlp/submission/testing/**` |
| `sc-observability-otlp` (`boundaries/sc-observability-otlp/otlp.toml`) | New feature `durable-store` = [`sync-http`, `dep:rusqlite`, `dep:serde-saphyr`, `dep:uuid`]. New public module `durable` (`DurableTelemetryClient`, which implements `TelemetryClient`, and `load_telemetry_file`). The store schema DDL, the drain worker, and sync-http encoders for the new point forms and `/v1development/profiles`. Crate-private seams staged by d-29 (ruling R14). Dev-dependencies += `sc-observability-types[test-double]`, `tempfile`. | `allowed_dependents` += `sc-observability-py`, `sc-otel-cli`; `policy/otlp-transport.toml` += separate `[transport.rusqlite]` (`=0.40.2`, `["bundled"]`), `[transport.serde-saphyr]` (`=1.3.0`, `["deserialize"]`) and `[transport.uuid]` (`=1.26.1`, `["std", "v7"]`) rows, each default features off and backend `durable-store`; `[dev_dependencies]` += the two dev rows |
| `sc-observability-py` (`boundaries/sc-observability-py/python.toml`) | New `otlp-telemetry` Cargo feature = [`dep:sc-observability-otlp`, `sc-observability-otlp/durable-store`]; `test-hooks` += `sc-observability-types/test-double`. New Python `sc_observability.telemetry` module. Release wheels enable the feature through `[tool.maturin] features`. | `allowed_dependencies` += `sc-observability-otlp` |
| `sc-otel-cli` (new; `boundaries/sc-otel-cli/cli.toml`) | Binary `sc-otel`: `emit`, `validate`, `flush`, `status`. Exit-code table and JSON output schema `sc-otel.result/v1`. Workspace member with `publish = false`; feature `test-double`. | `allowed_dependencies` = [`sc-observability-types`, `sc-observability-otlp`, `clap`, `serde_json`]; `forbidden_edges` = [`sc-observe`, `pyo3`, `agent-team-mail-*`]; `allowed_dependents` = [] |
| `scripts/sanity-telemetry/` (not a crate) | Python importer that consumes the installed `sc_observability.telemetry` API. | none (Python package dependency only) |

### Wave-5 wave table

One track. Wave 5 is its own gh stack, `phase-d-wave5`, on `integrate/phase-d` (ruling R12), ordered d-29 → d-33 → d-30 → d-31 → d-32 as layers 1–5.

| Wave | Sprint | Closure | Target boundary | Owned paths (summary; sprint doc is authoritative) |
| --- | --- | --- | --- | --- |
| 5.1 | d-29 | contract | `BOUNDARY-ScObservabilityTypes` (wave-5 contract in `sc_observability_types::otlp`) | `crates/sc-observability-types/src/otlp/**`, `crates/sc-observability-types/Cargo.toml` (feature only), its `otlp_*` tests and fixtures, `crates/sc-otel-cli/Cargo.toml`, stubbed `crates/sc-otel-cli/src/main.rs`, `Cargo.toml`, `Cargo.lock`, `policy/otlp-transport.toml`, `boundaries/**` wave-5 rows, `crates/sc-observability-otlp/Cargo.toml`, `crates/sc-observability-otlp/src/lib.rs` (one registration hunk), the staged `durable/` stubs, `schema.sql` and `config_file.rs`, the crate-private seam hunks (`lifecycle.rs`, one `contracts.rs` line, `contracts/profiles.rs`, `contracts/credits.rs`), `tests/contract_*.rs`, `bindings/python/sc-observability-py/Cargo.toml`, `.github/workflows/telemetry-platforms.yml`, ADR-021/PHD text, the new API approval record |
| 5.2 | d-33 | boundary (implementer) | `BOUNDARY-ScObservabilityOtlp` | `crates/sc-observability-otlp/src/durable/**` except `schema.sql`, `crates/sc-observability-otlp/src/sync_http/**`, `crates/sc-observability-otlp/tests/durable_*.rs`, `crates/sc-observability-otlp/tests/submission_*.rs`, `crates/sc-observability-otlp/tests/support/**`, the staging attributes on the d-29 seams |
| 5.2 | d-30 | boundary (consumer) | `BOUNDARY-ScObservabilityPy` | `bindings/python/sc-observability-py/**` except `Cargo.toml` and `python/sc_observability/generated/**` (telemetry tests in `tests_telemetry/**`) |
| 5.2 | d-31 | boundary (consumer) | `BOUNDARY-ScOtelCli` | `crates/sc-otel-cli/src/**`, `crates/sc-otel-cli/tests/**` |
| 5.3 | d-32 | integration | wave-5 composition | `scripts/sanity-telemetry/**`, `tests/telemetry-e2e/**`, `.github/workflows/telemetry-e2e.yml`, `.sc/telemetry.yaml`, the `.gitignore` line for `.sc/telemetry-state/`, `docs/telemetry-submission.md` |

- Tracks: 1. Waves: 3. Critical path: 3 (d-29 → d-33 → d-32, or through
  d-30 or d-31). Width: 3 (d-33 ∥ d-30 ∥ d-31). Sprint count: 5.
- Edges: d-29 ← {d-26, d-28}; {d-33, d-30, d-31} ← d-29;
  d-32 ← {d-30, d-31, d-33}. All wave-5.2 pairs are `parallel_safe`, because
  their owned paths are disjoint.
- Difficulty (`docs/plans/phase-d/difficulty.csv`): d-29 hard, d-33 hard,
  d-30 normal, d-31 normal, d-32 normal. Recommended agents: d-29 aobs/astra
  (contract breadth), d-33 aobs/astra (store, lease and every encoder),
  d-30, d-31 and d-32 cobs/terra.

### Lead rulings (2026-10-01)

These are lead decisions dated 2026-10-01 and are binding on the sprint docs.

- **R1 Base.** Sequence: #788 and the wave-5 plan land in `develop`, then `develop`
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
- **R3 Contract placement.** Superseded by P1 below: shared submission
  contracts live in `sc_observability_types::otlp::submission`. Store, drain
  and export live in `sc-observability-otlp` behind the `durable-store`
  feature.
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

#### Lead rulings on the provisional choices (2026-10-01)

These are lead decisions dated 2026-10-01. They replace the earlier list of
provisional choices.

- **P1 No new crate.** No `sc-observability-otlp-types` crate exists; the
  landed D22 types live in `crates/sc-observability-types/src/otlp/`. This
  overrides R3. The shared submission contracts (envelope, receipt, delivery
  status, error codes, config DTO and the `TelemetryClient` trait) go in
  `sc_observability_types::otlp::submission`. The CLI depends on
  `sc-observability-types` plus `sc-observability-otlp` (feature
  `durable-store`). No crate is added to the release inventory.
- **P2 Shared files with in-flight d-18.** Accepted. d-18's fence includes
  `crates/sc-observability-otlp/src/lib.rs` and `docs/api-approvals/**`.
  d-29 touches them only additively: one
  `#[cfg(feature = "durable-store")] pub mod durable;` hunk and one new
  approval file. If d-18 is still open, d-29 merges forward d-18's pushed
  head before each round. No DAG edge is added (R9). Wave 5 edits nothing
  under `release/**`.
- **P3 Disk-bound default.** Accepted: `DiskBoundPolicy::RejectNew`, which
  returns `AdmissionError::DiskBoundExceeded`, is counted in
  `rejected_by_disk_bound` and is shown in `StoreStatus`. `EvictOldest` is
  opt-in and also counted.
- **P4 YAML parser.** `feat/qa-sanity-telemetry-config` adds
  `.sc/telemetry.yaml` but no parser for it. The d-32 importer is Python and
  uses the repository's existing PyYAML. The Rust config loader
  (`load_telemetry_file`, used by the CLI `--config` flag and Python
  `Telemetry(config=...)`) uses `serde-saphyr =1.3.0` under `durable-store`,
  recorded in the d-29 dependency audit. `sc-observability-types` gets no
  YAML dependency.
- **P5 Full payload support.** Every pinned v1.10.0 field is supported,
  including `string_value_strindex` and `key_strindex` (the profile
  dictionary forms) and non-finite doubles in the proto-JSON
  `"NaN"`/`"Infinity"`/`"-Infinity"` encoding, with round-trip tests in d-29
  and d-33. `tracez.proto` is excluded as out of scope: it is zPages, not an
  OTLP payload.
- **P6 Public API approval record.** d-29 prepares
  `docs/api-approvals/phase-d-wave5-telemetry-submission.json`, and the user
  signs it as a d-29 closeout gate. It does not block the plan or the start
  of wave 5.2.

#### Lead rulings, round 2 (2026-10-01)

These are lead decisions on the round-2 plan review. They are binding on the
sprint docs.

- **R12 One stack per wave.** Wave 5 is its own gh stack, `phase-d-wave5`,
  on `integrate/phase-d`. This amends, for phase D, the planning.md rule
  that a phase is one append-only stack: phase D uses one stack per wave.
  Wave-5 layers are numbered 1–5 within the `phase-d-wave5` stack and do not
  continue the waves 1–4 layer sequence. d-29 targets `integrate/phase-d`;
  each later layer targets the branch of the layer below. Beads carry
  `metadata.stack = "phase-d-wave5"`, so "two beads claim the same stack and
  layer" is checked per stack.
- **R13 Plan location.** The wave-5 plan lives in this section of
  `plan-phase-d.md`. `docs/plans/telemetry-python-cli.md` is deleted, and
  ADR-021 and the sprint docs link here. The plan branch keeps the name
  `plan/telemetry-python-cli-hardening`; the plan PR description records
  that exception to the `plan/phase-<phase>` form.
- **R14 Crate-private seams, contract first.** d-29 fixes and stages the
  `sc-observability-otlp` seams d-33 builds on, in d-29-owned files:
  `SignalKind::Profiles`, the `ProfileExporter` trait (new
  `contracts/profiles.rs`), `AdmissionCredits::wait_for_release`, and the
  `durable::adapter::otel_config_from` signature. The one-line `contracts.rs`
  hunk (`pub(crate) mod profiles;`) is additive under the P2 merge-forward
  rule. `ExporterSet` is unchanged. The d-33 doc lists every crate-private
  item d-33 may change.
- **R15 Dependency set.** d-29 commits every wave-5 dependency: exact pins
  `uuid =1.26.1` (`std`, `v7`), `rusqlite =0.40.2` (`bundled`),
  `serde-saphyr =1.3.0` (`deserialize`) and `clap =4.6.7` (`std`, `derive`,
  `help`, `usage`, `error-context`), all checked against crates.io, plus the
  reused `serde_json` and `tempfile`. `uuid` v7 generates `SubmissionId` and
  the lease holder randomness; the holder ID is `<pid>:<uuid>`, with no
  hostname. `serde-saphyr` has its own transport row. `opentelemetry-proto
  =0.33.0` serde rejects proto-JSON non-finite strings outside
  `ValueAtQuantile`, so d-33 decodes with its own proto-JSON reader. d-29
  carries `cargo check --locked` criteria for every wave-5 graph.
- **R16 No validator edits.** The validator scripts are not edited. The
  claim that they check the new manifests is removed. The `sc-otel-cli`
  edges are enforced by explicit `cargo tree` criteria in d-29 and d-31, and
  the `durable-store` binding by the named d-29 test `durable_store_binding`.
- **R17 Platform, CI and public-surface proof.** Accepted as written by the
  reviewer: d-29 adds the `workflow_dispatch` workflow
  `telemetry-platforms.yml` (six targets) and d-30 dispatches
  `b4a-python-distributions.yml`, each with a recorded run URL; d-32's
  `telemetry-e2e` triggers on `sprint/*` and `integrate/*`, D9 runs by
  dispatch on the d-32 head, and the D18 gate and cargo-deny run in Required
  validation with captured logs. The wave-5 public surface is frozen at d-29:
  the approval record holds per-crate `api_sha256` and `feature_api_sha256`
  values, and d-33, d-30 and d-31 each prove their crates still match.

References: [signals](https://opentelemetry.io/docs/concepts/signals/),
[metric data model](https://opentelemetry.io/docs/specs/otel/metrics/data-model/),
[profiles](https://opentelemetry.io/docs/concepts/signals/profiles/).

## Requirement mapping


Source: the 56 numbered deliverables of D.1 to D.11 as written in develop's `docs/plans/phase-d` sprint docs, mapped by content to the bead items that carry the work after the re-cut. Updated after round-2 splitting/folding on 2026-09-26; each current target is checked against its bead deliverables. Retire this section when phase d closes.

| Original item | Bead#item(s) |
| --- | --- |
| D.1.1 | d-13#1 |
| D.1.2 | d-1#1 |
| D.1.3 | d-1#2 |
| D.1.4 | d-1#3 |
| D.2.1 | d-13#2 |
| D.2.2 | d-2#1 |
| D.2.3 | d-2#2 |
| D.2.4 | d-2#3 |
| D.2.5 | d-2#4 |
| D.2.6 | d-2#5 |
| D.3.1 | d-13#3, d-3#1 |
| D.3.2 | d-13#3, d-3#2 |
| D.3.3 | d-3#3 |
| D.3.4 | d-3#3 |
| D.4.1 | d-12#2 |
| D.4.2 | d-12#2 |
| D.4.3 | d-4#1, d-14#1, d-14#2, d-15#1, d-15#2, d-16#1, d-17#1, d-18#1, d-19#1, d-20#1, d-20#2 |
| D.4.4 | d-12#1, d-21#3, d-18#3 |
| D.4.5 | d-12#1, d-18#4 |
| D.4.6 | d-13#4, d-18#1 |
| D.4.7 | d-12#1, d-21#3, d-18#3 |
| D.4.8 | d-18#4 |
| D.5.1 | d-12#3 |
| D.5.2 | d-5#1 |
| D.5.3 | d-5#2 |
| D.5.4 | d-18#4 |
| D.5.5 | d-12#3, d-14#1, d-15#1, d-19#1 |
| D.5.6 | d-12#2, d-5#3 |
| D.6.1 | d-21#1, d-6#1 |
| D.6.2 | d-6#2 |
| D.6.3 | d-6#3 |
| D.6.4 | d-6#3 |
| D.6.5 | d-6#3 |
| D.6.6 | d-6#4, d-18#4 |
| D.7.1 | d-21#2, d-7#1 |
| D.7.2 | d-7#2 |
| D.7.3 | d-7#3, d-18#2 |
| D.7.4 | d-7#4 |
| D.8.1 | d-8#1 |
| D.8.2 | d-8#2 |
| D.8.3 | d-8#3 |
| D.8.4 | d-8#4 |
| D.8.5 | d-8#5 |
| D.8.6 | d-21#2, d-8#6 |
| D.9.1 | d-9#1 |
| D.9.2 | d-9#2 |
| D.9.3 | d-9#3 |
| D.9.4 | d-9#4 |
| D.10.1 | d-10#1 |
| D.10.2 | d-10#2 |
| D.10.3 | d-10#3, d-10#7 |
| D.10.4 | d-10#4, d-18#3 |
| D.11.1 | d-10#5 |
| D.11.2 | d-10#6 |
| D.11.3 | d-10#7 |
| D.11.4 | d-10#8 |



## Current deliverable additions

obs-d-19 #1–3 own DTO/schema/generated-model migration formerly included in obs-d-18 #5; obs-d-20 #1–3 own language adapter migration. obs-d-18 #5 now qualifies their real composition with logging and transport artifacts. Former obs-d-11 #1–4 map to obs-d-10 #5–8. Per-sprint document projection is supporting review material, not an independent closure gate.

Measured against the 56 original deliverables (lead, 2026-09-26, fix round 2): nine items are additions with no original row: d-4#2, d-14#3, d-15#3, d-16#2 and d-17#2 retype the local tests of each migration boundary; d-18#5 qualifies the composed d-19/d-20 outputs; d-19#2 and d-19#3 update the schema generator, conformance corpus and typing checks; d-20#3 updates transport/runtime/typing fixtures and examples. Every other item in every bead has an original row above.
Split accounting: old obs-d-12 #1 → obs-d-12 #1 plus obs-d-21 #2 (boundary records); old #2 → obs-d-12 #2; old #3 → obs-d-12 #3; old #4 → obs-d-21 #1 plus obs-d-12 #3 (neutral tests); old #5 → obs-d-21 #2; old #6 → obs-d-12 #1 (types manifest/specification) plus obs-d-21 #3 (atomic workspace activation). No numbered deliverable is dropped. The new sprint projection is the only added owned path; the old path/REQ/ADR unions are preserved.

## Acceptance criteria

- [ ] every sprint of the phase closed
- [ ] every finding closed with a close reason
- [ ] phase PR integrate/phase-d -> develop merged
