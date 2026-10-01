# d-29: telemetry submission contract

## Plan metadata

- Wave: 5.1 (wave-5 contract)
- Stack / layer: `phase-d-wave5` stack, layer 1 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: aobs / astra
- Difficulty: `hard` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `contract`
- Target boundary: BOUNDARY-ScObservabilityTypes
- vertical_rationale: "contract artifacts only: schema.sql is contract DDL d-33 implements; the load_telemetry_file signature is the config entry point d-30 and d-31 call and d-33 implements; the staged crate-private seams in sc-observability-otlp (SubmissionExporter, ProfileExporter, SignalKind::Profiles, wait_for_release, otel_config_from, exporter_for) are where d-33 and d-34 meet; the codes and constants are the ADR-005 registries all four consume (wave-5 rulings R14, R20)". The target boundary covers the neutral signal types and the submission contract in `sc_observability_types::otlp`; criteria for the otlp work are rooted at `boundary:BOUNDARY-ScObservabilityOtlp`.
- Branch: `sprint/d-29-telemetry-submission-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-29-telemetry-submission-contract`
- PR target: `integrate/phase-d` (base of the `phase-d-wave5` stack)
- Blocked by: `obs-d-26-sanity`, `obs-d-28-sanity`
- Requirements: PHB-010, PHD-001, PHD-002, PHD-003, PHD-004, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-011, PHD-012, PHD-013
- ADRs: ADR-002, ADR-004, ADR-005, ADR-009, ADR-012, ADR-017, ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - Cargo.toml
  - Cargo.lock
  - policy/otlp-transport.toml
  - policy/deny-durable-store.toml
  - boundaries/sc-observability-types/types.toml
  - boundaries/sc-observability-otlp/otlp.toml
  - boundaries/sc-observability-py/python.toml
  - boundaries/sc-otel-cli/**
  - crates/sc-observability-types/Cargo.toml
  - crates/sc-observability-types/src/error_codes.rs
  - crates/sc-observability-types/src/constants.rs
  - crates/sc-observability-types/src/otlp/**
  - crates/sc-observability-types/tests/otlp_signals_contract.rs
  - crates/sc-observability-types/tests/otlp_submission_contract.rs
  - crates/sc-observability-types/tests/fixtures/otlp_submission/**
  - crates/sc-observability-otlp/Cargo.toml
  - crates/sc-observability-otlp/src/lib.rs
  - crates/sc-observability-otlp/src/constants.rs
  - crates/sc-observability-otlp/src/error_codes.rs
  - crates/sc-observability-otlp/src/contracts.rs
  - crates/sc-observability-otlp/src/contracts/profiles.rs
  - crates/sc-observability-otlp/src/contracts/submission.rs
  - crates/sc-observability-otlp/src/contracts/credits.rs
  - crates/sc-observability-otlp/src/lifecycle.rs
  - crates/sc-observability-otlp/src/sync_http/mod.rs
  - crates/sc-observability-otlp/src/sync_http/submission.rs
  - crates/sc-observability-otlp/src/durable/mod.rs
  - crates/sc-observability-otlp/src/durable/adapter.rs
  - crates/sc-observability-otlp/src/durable/config_file.rs
  - crates/sc-observability-otlp/src/durable/schema.sql
  - crates/sc-observability-otlp/tests/contract_schema.rs
  - crates/sc-observability-otlp/tests/contract_manifest.rs
  - crates/sc-otel-cli/Cargo.toml
  - crates/sc-otel-cli/src/main.rs
  - bindings/python/sc-observability-py/Cargo.toml
  - docs/architecture.md
  - docs/requirements.md
  - docs/api-approvals/phase-d-wave5-telemetry-submission.json

Ownership notes:

- `crates/sc-observability-types/Cargo.toml`: the `test-double` feature and
  the optional `uuid` line only.
- The four registry and constants files (`src/error_codes.rs` and
  `src/constants.rs` in types and otlp): additive wave-5 entries only
  (ADR-005).
- `crates/sc-observability-otlp/src/lib.rs`: one registration hunk.
  `src/contracts.rs`: two module lines. `src/sync_http/mod.rs`: one module
  line. See the shared-file note under Relations.
- `src/contracts/credits.rs`: the staged `wait_for_release` signature only.
  `src/lifecycle.rs`: the `SignalKind::Profiles` hunk only.
- Staged and handed off at d-29 sanity: `src/durable/mod.rs`,
  `durable/adapter.rs`, `durable/config_file.rs` and
  `src/contracts/submission.rs`'s attribute to d-33;
  `src/sync_http/submission.rs` to d-34; `crates/sc-otel-cli/src/main.rs` to
  d-31. `src/durable/schema.sql` stays d-29's contract DDL.
- `docs/architecture.md`: ADR-021, the §6 rows and the binding edges.
  `docs/requirements.md`: PHD-005–013. `docs/api-approvals/`: one new file
  (see Closeout gate).

## Relations

- `must_follow` d-26: consumes the compatible OTLP config adapters
  (`OtelConfig`, `ExporterBackend::SyncHttp`, `SyncHttpRetryPolicy`) that
  `TelemetryClientConfig` resolves into.
- `must_follow` d-28: consumes the 1.x release/compat baseline that the
  additive public surface is checked against.
- d-33, d-34, d-30 and d-31 `must_follow` d-29. They consume the
  `TelemetryClient` trait, the envelope/receipt/error types, the codes and
  constants, `schema.sql`, the `load_telemetry_file` signature, the staged
  crate-private seams, the dependency set, the test double and the golden
  fixtures. d-35 and d-32 reach d-29 through d-30 and the d-30/d-31/d-33/d-34
  edges.
- Shared-file note: `crates/sc-observability-otlp/src/lib.rs`,
  `crates/sc-observability-otlp/src/contracts.rs` and `docs/api-approvals/**`
  are also in the in-flight d-18 fence. d-29 adds only
  `#[cfg(feature = "durable-store")] pub mod durable;` to `lib.rs`, only
  `pub(crate) mod profiles;` and `pub(crate) mod submission;` to
  `contracts.rs`, and only one new file under
  `docs/api-approvals/`. If d-18 is unmerged, d-29 merges forward d-18's
  pushed head before every round. No DAG edge is added (lead ruling P2,
  wave-5 ruling R14). `lifecycle.rs` and `contracts/credits.rs` are outside
  the d-18 fence.

## Goal

Fix every wave-5 interface before any implementation, so that d-33, d-34,
d-30 and d-31 run in parallel without renegotiating.

d-29 delivers contract artifacts only, each consumed by a named wave-5.2
sprint:

- The signal types and envelope canonicalization, fully implemented, because
  the canonical JSON is what both front ends submit (d-30, d-31), what the
  store holds (d-33) and what the encoders read back (d-34).
- `schema.sql` and its versioning policy: the DDL d-33 implements.
- The `load_telemetry_file` signature and `TelemetryFileConfig`: the config
  entry point d-30 and d-31 call. d-33 implements the body.
- The error codes and defaults in the ADR-005 registries, which all four
  sprints use without adding entries.
- The test double, `DoubleScript` and the conformance suite: d-30, d-31 and
  d-35 test against the double, and d-33 runs the suite.
- The staged crate-private seams in `sc-observability-otlp`: the d-33 drain
  and the d-34 exporter meet at `SubmissionExporter`, so neither waits for
  the other.

No runtime behaviour is implemented here: the store, drain,
`wait_for_release`, the loader body and the platform workflow are d-33's,
and the OTLP/JSON export is d-34's (wave-5 ruling R20).

## Deliverables

1. Pin the protocol: workspace `opentelemetry-proto = "=0.33.0"` (already
   pinned), which vendors upstream **opentelemetry-proto v1.10.0**. d-29 adds
   no feature to it and no prost dependency to the sync-http path; d-34
   encodes OTLP/JSON by hand. Commit the field inventory below as the doc
   comment of `crates/sc-observability-types/src/otlp/signals/mod.rs`.
   [PHD-013]
2. Add the neutral signal types in `crates/sc-observability-types/src/otlp/signals/`,
   declared from `src/otlp/mod.rs`, with validating constructors, serde that
   deserializes through those constructors, and `TryFrom` conversions from
   `LogEvent`, `SpanRecord<SpanEnded>`, v2 `MetricRecord`, `OtlpResource` and
   `OtlpInstrumentationScope`. Add no variant or field to any existing type.
   [PHD-005, PHD-006, PHD-013]
3. Add module `sc_observability_types::otlp::submission` (declared from
   `src/otlp/mod.rs`, not `src/lib.rs`; no new crate, lead ruling P1) with the
   submission contract: `SubmissionInput` with the full `LogInput`/`SpanInput`
   field lists and the plain-JSON value form, `SubmissionEnvelope`,
   `from_input` canonicalization, receipts, status, the flush result rules,
   the error enums and their codes, `TelemetryClientConfig` with its per-field
   source table and precedence resolver, and the `TelemetryClient` trait.
   The error enums are additive typed errors carrying `ErrorContext`
   (ADR-012). Their codes go in `crates/sc-observability-types/src/error_codes.rs`
   and are re-exported from `otlp::submission::error_codes`; the defaults in
   the per-field source table are named constants in
   `crates/sc-observability-types/src/constants.rs` (ADR-005).
   [PHB-010, PHD-001, PHD-005, PHD-007, PHD-008, PHD-010]
4. Add the `InMemoryTelemetryClient` test double with the `DoubleScript`
   scripted-outcome type, and the public conformance suite
   `testing::conformance::run_all` with its `ConformanceHarness` trait, behind
   the new `sc-observability-types` feature `test-double`, at the manifest's
   `allowed_test_double_paths`
   (`crates/sc-observability-types/src/otlp/submission/testing/**`). Add the
   golden fixture set at
   `crates/sc-observability-types/tests/fixtures/otlp_submission/golden/`.
   [PHD-009, PHD-010, PHD-013]
5. Commit `crates/sc-observability-otlp/src/durable/schema.sql` and the
   versioning policy, plus the staged `durable/mod.rs` stub declaring
   `DurableTelemetryClient`. Its methods return
   `AdmissionError::StoreUnavailable`, never `todo!()`. Stage
   `durable/config_file.rs` with the `load_telemetry_file` signature and
   `TelemetryFileConfig`: the stub returns
   `TelemetryConfigError::ConfigFile { path }` for every path, so no caller
   gets a partial config. d-33 implements the body and its fixture tests
   (wave-5 ruling R20). Add
   `crates/sc-observability-otlp/tests/contract_schema.rs`, which loads
   `schema.sql` into an empty in-memory SQLite database.
   [PHD-007, PHD-008, PHD-010]
6. Commit the complete wave-5 dependency set in the section "Dependency set"
   below: every workspace pin, per-crate normal, dev and feature-gated
   dependency, the `sc-otel-cli` workspace member and skeleton manifest, the
   matching `policy/otlp-transport.toml` rows, and the boundary allowlist
   rows. Add `crates/sc-observability-otlp/tests/contract_manifest.rs`, which
   checks the `durable-store` binding. Run the dependency and license audit
   (cargo-deny with `policy/deny-durable-store.toml`) over the
   `durable-store` and `sc-otel-cli` graphs. [PHD-003, PHD-007, PHD-009, PHD-010]
7. Keep ADR-021, PHD-005–013 and the §6 rows consistent with any contract
   change made in this sprint. The normative text itself lands with the plan
   (wave-5 ruling R1); d-29 edits it only when a contract detail changes.
   This covers the importer and e2e requirements PHD-011 and PHD-012 in
   `docs/requirements.md`. [PHD-002, PHD-003, PHD-011, PHD-012, PHD-013]
8. Add contract tests: `crates/sc-observability-types/tests/otlp_signals_contract.rs`
   and `crates/sc-observability-types/tests/otlp_submission_contract.rs`
   (golden fixtures, precedence, error codes, flush rules, scripted double,
   test-double conformance). [PHD-005, PHD-006, PHD-013]
9. Prepare `docs/api-approvals/phase-d-wave5-telemetry-submission.json`
   with, per changed crate, the `api_sha256` that `validate_public_api.py diff`
   reports and a `feature_api_sha256` map for the wave-5 features. This
   freezes the wave-5 public surface (wave-5 ruling R17). The user signs it at
   d-29 closeout. [PHD-002]
10. Stage the crate-private seams in `sc-observability-otlp` that d-33 and
    d-34 build on (wave-5 rulings R14, R20), as specified in "Crate-private
    seams" below: `SignalKind::Profiles`, the `ProfileExporter` trait, the
    `SubmissionExporter` trait and `SubmissionExportFailure`, the
    `AdmissionCredits::wait_for_release` signature, the
    `durable::adapter::otel_config_from` signature, the staged
    `SyncHttpSubmissionExporter` and `exporter_for`, and the wave-5 otlp
    constants and the `SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED` code
    in otlp `src/constants.rs` and `src/error_codes.rs` (ADR-005). The
    crate-private items stay out of the public surface (ADR-004: OTel types
    stay in `sc-observability-otlp`). [PHB-010, PHD-004, PHD-013]

## This Sprint Does Not Close

- The store, drain, lease, backpressure (`wait_for_release` body), the
  `load_telemetry_file` body and the platform workflow. d-33 owns them.
- OTLP/JSON encoding, `ProfileExporter` and per-variant round trips. d-34
  owns them.
- Python bindings and the wheel feature in `pyproject.toml`. d-30 owns them.
- CLI argument parsing, output and the exit-code mapping implementation.
  d-31 owns them.
- The sanity-history importer. d-35 owns it.
- Installed front-end submission, viewer readback and the D18/D9 re-run.
  d-32 owns them.

## Design

### Protocol pin and field inventory

Pin: Rust crate `opentelemetry-proto =0.33.0`, upstream tag `v1.10.0`.
Profiles are `opentelemetry.proto.profiles.v1development`, exported to
`/v1development/profiles`.

| Proto message (v1.10.0) | Fields | Neutral type |
| --- | --- | --- |
| `common.AnyValue` | `string_value`, `bool_value`, `int_value`, `double_value`, `array_value`, `kvlist_value`, `bytes_value` | `AnyValue::{String, Bool, Int, Double, Array, KvList, Bytes}` (plus `UInt`; see the policy below) |
| `common.AnyValue` | `string_value_strindex` | `AnyValue::StringIndex(StringIndex)`: an index into `ProfilesDictionary.string_table`, valid only inside a profiles payload |
| `common.KeyValue` | `key`, `value` | `KeyValues` entry `(String, AnyValue)`; duplicate keys rejected |
| `common.KeyValue` | `key_strindex` | `AttributeKey::Index(StringIndex)`, under the same profiles-only rule |
| `common.InstrumentationScope` | `name`, `version`, `attributes`, `dropped_attributes_count` | `InstrumentationScope` |
| `common.EntityRef` | `schema_url`, `type`, `id_keys`, `description_keys` | `EntityRef` |
| `resource.Resource` | `attributes`, `dropped_attributes_count`, `entity_refs` | `Resource` |
| `Resource*`/`Scope*` wrappers | `schema_url` | `Resource.schema_url`, `InstrumentationScope.schema_url` |
| `logs.LogRecord` | `time_unix_nano`, `observed_time_unix_nano`, `severity_number`, `severity_text`, `body`, `attributes`, `dropped_attributes_count`, `flags`, `trace_id`, `span_id`, `event_name` | `LogPoint` |
| `trace.Span` | `trace_id`, `span_id`, `trace_state`, `parent_span_id`, `flags`, `name`, `kind`, `start_time_unix_nano`, `end_time_unix_nano`, `attributes`, `dropped_attributes_count`, `events`, `dropped_events_count`, `links`, `dropped_links_count`, `status` | `SpanPoint` |
| `trace.Span.Event` | `time_unix_nano`, `name`, `attributes`, `dropped_attributes_count` | `SpanEventPoint` |
| `trace.Span.Link` | `trace_id`, `span_id`, `trace_state`, `attributes`, `dropped_attributes_count`, `flags` | `SpanLinkPoint` |
| `trace.Status` | `message`, `code` | `SpanStatusPoint` |
| `metrics.Metric` | `name`, `description`, `unit`, `metadata`, `data` | `MetricStream` |
| `metrics.Gauge` / `Sum` / `Histogram` / `ExponentialHistogram` / `Summary` | `data_points`, `aggregation_temporality`, `is_monotonic` | `MetricData::{Gauge, Sum, Histogram, ExponentialHistogram, Summary}` |
| `metrics.NumberDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `as_double`/`as_int`, `exemplars`, `flags` | `NumberPoint` |
| `metrics.HistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `bucket_counts`, `explicit_bounds`, `exemplars`, `flags`, `min`, `max` | `HistogramDataPoint` |
| `metrics.ExponentialHistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `scale`, `zero_count`, `positive`, `negative`, `flags`, `exemplars`, `min`, `max`, `zero_threshold` | `ExponentialHistogramDataPoint`, `ExponentialBuckets` |
| `metrics.SummaryDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `quantile_values`, `flags` | `SummaryDataPoint`, `ValueAtQuantile` |
| `metrics.Exemplar` | `filtered_attributes`, `time_unix_nano`, `as_double`/`as_int`, `span_id`, `trace_id` | `Exemplar` |
| `profiles.ProfilesDictionary` | `mapping_table`, `location_table`, `function_table`, `link_table`, `string_table`, `attribute_table`, `stack_table` | `ProfilesDictionary` |
| `profiles.Profile` | `sample_type`, `samples`, `time_unix_nano`, `duration_nano`, `period_type`, `period`, `profile_id`, `dropped_attributes_count`, `original_payload_format`, `original_payload`, `attribute_indices` | `Profile` |
| `profiles.Sample`, `ValueType`, `Mapping`, `Stack`, `Location`, `Line`, `Function`, `Link`, `KeyValueAndUnit` | all fields | same-named neutral structs |
| any `double` field | NaN / ±Inf | `OtlpDouble` carries every IEEE-754 value, including NaN and ±Inf; see the non-finite policy |
| `tracez.proto` | all | excluded — out of scope (lead ruling P5, 2026-10-01): zPages, not an OTLP payload |
| `collector.*.Export*ServiceRequest/Response` | all | not neutral: built and parsed by the d-34 sync-http encoder |

Enumerations: `SeverityNumber` 0–24, `SpanKind` including `Unspecified`, and
`StatusCode::{Unset, Ok, Error}`. `AggregationTemporality::Unspecified` is
rejected as invalid input, because the spec forbids it on Sum and Histogram.
`DataPointFlags(u32)` keeps `NO_RECORDED_VALUE`. Span and log flags are kept
as `u32`.

### Neutral signal types (`sc_observability_types::otlp::signals`)

All new types are `#[non_exhaustive]` and constructed through `try_new` or
builders. Released exhaustive types are not extended. Every type with a
validation rule deserializes through its validating constructor:
`#[serde(try_from = "<Type>Raw")]`, where `<Type>Raw` is a private
field-for-field mirror. So no deserialized value bypasses validation. This
covers `TraceState`, `StringIndex`, `KeyValues`, `NumberPoint`,
`HistogramDataPoint`, `ExponentialHistogramDataPoint`, `SummaryDataPoint`,
`ValueAtQuantile` and `MetricData` (which owns the temporality and
monotonicity rules). Cross-table profile references are checked by
`ProfilesDictionary::validate_references`, which `from_input` calls.

```rust
/// Canonical (envelope) form is the adjacently tagged form below. Input
/// deserialization also accepts the plain-JSON form; see "Input value forms".
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AnyValue {
    String(String),
    Bool(bool),
    Int(i64),
    /// Accepted as input; see the uint policy.
    #[serde(rename = "uint")]
    UInt(u64),
    Double(OtlpDouble),
    /// Serialized as lowercase hex in neutral JSON.
    Bytes(Vec<u8>),
    /// `string_value_strindex`; profiles payloads only.
    StringIndex(StringIndex),
    Array(Vec<AnyValue>),
    KvList(KeyValues),
}
impl<'de> Deserialize<'de> for AnyValue { /* manual: tagged or plain form */ }

/// Ordered; duplicate keys rejected (by `try_from_iter` and by deserialization).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct KeyValues(Vec<(AttributeKey, AnyValue)>);
impl<'de> Deserialize<'de> for KeyValues { /* manual: canonical list or plain map */ }

/// `key` or `key_strindex`. `Index` is valid only inside a profiles payload.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeKey { Name(String), Index(StringIndex) }

/// Index into `ProfilesDictionary.string_table` (proto `int32`, never negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i32")]
pub struct StringIndex(i32);

/// Any IEEE-754 double. Neutral and OTLP/JSON encode finite values as JSON
/// numbers and non-finite values as the proto-JSON strings "NaN", "Infinity"
/// and "-Infinity". Serialize, Deserialize and PartialEq are manual impls:
/// derives cannot produce the string forms, and equality is bitwise
/// (`f64::to_bits`), so NaN round trips compare equal.
#[derive(Debug, Clone, Copy)]
pub struct OtlpDouble(f64);
impl Serialize for OtlpDouble { /* number, or "NaN"/"Infinity"/"-Infinity" */ }
impl<'de> Deserialize<'de> for OtlpDouble { /* number or one of the three strings */ }
impl PartialEq for OtlpDouble { /* self.0.to_bits() == other.0.to_bits() */ }

#[non_exhaustive]
pub struct Resource {
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub entity_refs: Vec<EntityRef>,
    pub schema_url: Option<String>,
}
#[non_exhaustive]
pub struct EntityRef { pub schema_url: Option<String>, pub r#type: String,
    pub id_keys: Vec<String>, pub description_keys: Vec<String> }
#[non_exhaustive]
pub struct InstrumentationScope {
    pub name: String,
    pub version: Option<String>,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub schema_url: Option<String>,
}
impl TryFrom<OtlpResource> for Resource { type Error = SignalValidationError; /* checked attribute conversion */ }
impl TryFrom<OtlpInstrumentationScope> for InstrumentationScope { type Error = SignalValidationError; /* checked attribute conversion */ }

#[non_exhaustive]
pub struct ResourceRecord<T> { pub resource: Resource, pub scope: InstrumentationScope, pub record: T }

/// W3C tracestate; validated (≤32 list members, key/value grammar, ≤512 bytes).
#[non_exhaustive]
pub struct TraceState(String);
impl TraceState { pub fn try_new(value: impl Into<String>) -> Result<Self, SignalValidationError>; }

#[non_exhaustive]
pub struct LogPoint {
    pub time: Option<Timestamp>,
    pub observed_time: Timestamp,
    pub severity_number: SeverityNumber,
    pub severity_text: Option<String>,
    pub event_name: Option<String>,
    pub body: Option<AnyValue>,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub flags: u32,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
}

#[non_exhaustive]
pub struct SpanPoint {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub trace_state: Option<TraceState>,
    pub parent_span_id: Option<SpanId>,
    pub flags: u32,
    pub name: String,
    pub kind: SpanKindPoint,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub events: Vec<SpanEventPoint>,
    pub dropped_events_count: u32,
    pub links: Vec<SpanLinkPoint>,
    pub dropped_links_count: u32,
    pub status: SpanStatusPoint,
}
#[non_exhaustive] pub struct SpanEventPoint { pub time: Timestamp, pub name: String,
    pub attributes: KeyValues, pub dropped_attributes_count: u32 }
#[non_exhaustive] pub struct SpanLinkPoint { pub trace_id: TraceId, pub span_id: SpanId,
    pub trace_state: Option<TraceState>, pub attributes: KeyValues,
    pub dropped_attributes_count: u32, pub flags: u32 }
#[non_exhaustive] pub struct SpanStatusPoint { pub code: StatusCode, pub message: Option<String> }

#[non_exhaustive]
pub struct MetricStream {
    pub name: MetricName,
    pub description: Option<String>,
    pub unit: Option<String>,
    pub metadata: KeyValues,
    pub data: MetricData,
}
#[non_exhaustive]
pub enum MetricData {
    Gauge { points: Vec<NumberPoint> },
    Sum { points: Vec<NumberPoint>, temporality: AggregationTemporality, monotonic: bool },
    Histogram { points: Vec<HistogramDataPoint>, temporality: AggregationTemporality },
    ExponentialHistogram { points: Vec<ExponentialHistogramDataPoint>, temporality: AggregationTemporality },
    Summary { points: Vec<SummaryDataPoint> },
}
#[non_exhaustive] pub enum NumberValue { Int(i64), Double(OtlpDouble) }
#[non_exhaustive] pub struct DataPointFlags(u32);
#[non_exhaustive]
pub struct NumberPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub value: NumberValue, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags }
#[non_exhaustive]
pub struct HistogramDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: Option<OtlpDouble>, pub bucket_counts: Vec<u64>,
    pub explicit_bounds: Vec<OtlpDouble>, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags,
    pub min: Option<OtlpDouble>, pub max: Option<OtlpDouble> }
#[non_exhaustive] pub struct ExponentialBuckets { pub offset: i32, pub bucket_counts: Vec<u64> }
#[non_exhaustive]
pub struct ExponentialHistogramDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: Option<OtlpDouble>, pub scale: i32,
    pub zero_count: u64, pub zero_threshold: OtlpDouble, pub positive: ExponentialBuckets,
    pub negative: ExponentialBuckets, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags,
    pub min: Option<OtlpDouble>, pub max: Option<OtlpDouble> }
#[non_exhaustive] pub struct ValueAtQuantile { pub quantile: OtlpDouble, pub value: OtlpDouble }
#[non_exhaustive]
pub struct SummaryDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: OtlpDouble,
    pub quantile_values: Vec<ValueAtQuantile>, pub flags: DataPointFlags }
#[non_exhaustive]
pub struct Exemplar { pub filtered_attributes: KeyValues, pub time: Timestamp,
    pub value: NumberValue, pub trace_id: Option<TraceId>, pub span_id: Option<SpanId> }

#[non_exhaustive]
pub struct ProfilesDictionary {
    pub mapping_table: Vec<Mapping>, pub location_table: Vec<Location>,
    pub function_table: Vec<Function>, pub link_table: Vec<ProfileLink>,
    pub string_table: Vec<String>, pub attribute_table: Vec<KeyValueAndUnit>,
    pub stack_table: Vec<Stack>,
}
#[non_exhaustive]
pub struct Profile {
    pub sample_type: Option<ValueType>, pub samples: Vec<Sample>, pub time: Timestamp,
    pub duration_nanos: u64, pub period_type: Option<ValueType>, pub period: i64,
    pub profile_id: [u8; 16], pub dropped_attributes_count: u32,
    pub original_payload_format: Option<String>, pub original_payload: Vec<u8>,
    pub attribute_indices: Vec<i32>,
}
// Sample, ValueType, Mapping, Stack, Location, Line, Function, ProfileLink and
// KeyValueAndUnit mirror the v1.10.0 fields listed in the inventory.
impl ProfilesDictionary {
    /// Index 0 of every table is the required zero-value entry; every index
    /// in `profiles` must be in bounds for its table.
    pub fn validate_references(&self, profiles: &[ResourceRecord<Profile>])
        -> Result<(), SignalValidationError>;
}
```

Point validation (all in `try_new`, and therefore in deserialization):

- Histogram: increasing finite bounds, `bucket_counts.len() == bounds.len() + 1`,
  and the bucket sum equals `count`.
- Exponential histogram: `zero_count` plus the positive and negative bucket
  sums equals `count`; `scale` is in `-10..=20`.
- Summary: quantiles are in `[0, 1]` and strictly increasing.
- Sum: a monotonic sum is never negative.
- `start_time <= time`, and a delta interval is nonempty.

Errors are `SignalValidationError` (`#[non_exhaustive]`, `Box<ErrorContext>`,
code prefix `SC_OBSERVABILITY_TYPES_SIGNAL_`).

**uint policy.** OTLP `AnyValue` has only `int64`. `AnyValue::UInt(v)` with
`v <= i64::MAX` encodes as `intValue`. A larger `v` fails envelope validation
with `SubmissionError::ValueOutOfRange { path }` before admission. It is never
coerced to a double or a string.

**Non-finite policy.** Every proto `double` maps to `OtlpDouble` and accepts
NaN, `Infinity` and `-Infinity` (lead ruling P5). The only exceptions are the
data-model ordering rules above: histogram `explicit_bounds` must be strictly
increasing (so NaN is rejected there, while a final `Infinity` is allowed),
quantiles must be in `[0, 1]`, and `zero_threshold` must be non-negative.
Those return `SignalValidationError`, never a silent drop or coercion.

**String-index policy.** `AnyValue::StringIndex` and `AttributeKey::Index` are
kept as indices, never resolved to strings, so they round-trip unchanged.
Outside a profiles payload they fail envelope validation with
`SubmissionError::Validation { path }`. Inside one,
`ProfilesDictionary::validate_references` bounds-checks them against
`string_table`.

**bytes.** `AnyValue::Bytes` serializes as lowercase hex in the neutral JSON.
d-34 encodes it as base64 `bytesValue` in OTLP/JSON. Profile IDs, trace IDs
and span IDs use the existing hex newtypes.

**Conversion amendment (lead ruling 2026-10-01, `01M3VWCCBHT906CNTV50SVGPZ7`).**
Conversions from `LogEvent`, `SpanRecord<SpanEnded>`, v2 `MetricRecord`,
`OtlpResource` and `OtlpInstrumentationScope` use `TryFrom` with
`SignalValidationError::Validation` at the exact offending attribute path.
Existing null attributes are rejected, never dropped or coerced. Unsigned
values remain `AnyValue::UInt`; the envelope applies the uint range policy.

### Input value forms

`AnyValue` and `KeyValues` deserialize from two forms. Both conversions run in
Rust inside `from_json`; no front end converts values itself.

- **Canonical form** (what `to_canonical_json` writes): `KeyValues` is a JSON
  array of `[key, value]` pairs, and `AnyValue` is `{"kind": ..., "data": ...}`.
  Profile `AttributeKey::Index` keys are expressible only in this form.
- **Plain form** (for callers): `KeyValues` is a JSON object
  `{"name": value, ...}`. Duplicate names are rejected with
  `SubmissionError::Validation { path }`, and entries are ordered by name.
  A plain value maps as follows: string → `String`, bool → `Bool`, an integer
  within `i64` → `Int`, a larger integer within `u64` → `UInt` (then the uint
  policy applies), any number with a fraction or exponent → `Double`, array →
  `Array`, object → `KvList`. `null` is rejected with `Validation { path }`.
  Bytes, non-finite doubles, `uint` values within `i64` and string indexes
  need the tagged form: an object with exactly the keys `kind` and `data`,
  where `kind` is one of the `AnyValue` tags, is always read as a tagged
  value. To send such an object as a literal kvlist, use the canonical form.

### Submission contract (`sc_observability_types::otlp::submission`)

```rust
use super::signals; // sibling module in sc_observability_types::otlp

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvelopeVersion(u16);
impl EnvelopeVersion { pub const CURRENT: Self = Self(1); }

/// Caller-stable idempotency key (1..=256 bytes UTF-8); duplicate admission returns
/// the original receipt with `duplicate = true`.
#[non_exhaustive] pub struct RecordKey(String);
/// UUIDv7 text assigned at admission by the client (`uuid` crate, feature `v7`;
/// see "Dependency set").
#[non_exhaustive] pub struct SubmissionId(String);

/// The one input shape for every front end (Python `build_envelope`/`emit`,
/// `sc-otel validate`/`emit`) and every golden `input.json`.
/// Unknown keys are rejected (`deny_unknown_fields`).
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct SubmissionInput {
    pub version: EnvelopeVersion,               // required
    pub record_key: Option<RecordKey>,
    pub resource: Option<signals::Resource>,    // envelope default
    pub scope: Option<signals::InstrumentationScope>, // envelope default
    #[serde(default)] pub logs: Vec<LogInput>,
    #[serde(default)] pub spans: Vec<SpanInput>,
    #[serde(default)] pub metrics: Vec<MetricInput>,
    pub profiles: Option<ProfilesInput>,
}

/// Defaults in brackets. Plain or canonical value forms are accepted.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct LogInput {
    pub time: Option<Timestamp>,
    pub observed_time: Option<Timestamp>,        // [IdSource::now()]
    pub severity_number: Option<SeverityNumber>, // [Unspecified = 0]
    pub severity_text: Option<String>,
    pub event_name: Option<String>,
    pub body: Option<signals::AnyValue>,
    #[serde(default)] pub attributes: signals::KeyValues,
    #[serde(default)] pub dropped_attributes_count: u32,
    #[serde(default)] pub flags: u32,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
    /// Input-only. A log and a span with the same value in one submission share
    /// the span's trace_id/span_id (generated when absent). Not stored.
    pub correlation_id: Option<String>,
    pub resource: Option<signals::Resource>,     // [envelope resource]
    pub scope: Option<signals::InstrumentationScope>, // [envelope scope]
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct SpanInput {
    pub trace_id: Option<TraceId>,               // [generated]
    pub span_id: Option<SpanId>,                 // [generated]
    pub trace_state: Option<String>,             // validated into TraceState
    pub parent_span_id: Option<SpanId>,
    #[serde(default)] pub flags: u32,
    pub name: String,
    pub kind: Option<signals::SpanKindPoint>,    // [Unspecified]
    pub start_time: Timestamp,                   // required: an actual start time
    /// Exactly one of end_time / duration_nanos is required. Both present and
    /// agreeing is accepted; both present and disagreeing -> TimingConflict;
    /// neither -> Validation { path: "spans[i].end_time" }.
    pub end_time: Option<Timestamp>,
    pub duration_nanos: Option<u64>,
    #[serde(default)] pub attributes: signals::KeyValues,
    #[serde(default)] pub dropped_attributes_count: u32,
    #[serde(default)] pub events: Vec<signals::SpanEventPoint>,
    #[serde(default)] pub dropped_events_count: u32,
    #[serde(default)] pub links: Vec<signals::SpanLinkPoint>,
    #[serde(default)] pub dropped_links_count: u32,
    pub status: Option<signals::SpanStatusPoint>, // [Unset]
    pub correlation_id: Option<String>,
    pub resource: Option<signals::Resource>,
    pub scope: Option<signals::InstrumentationScope>,
}

/// A `MetricStream` plus optional per-record resource/scope overrides.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct MetricInput {
    #[serde(flatten)] pub stream: signals::MetricStream,
    pub resource: Option<signals::Resource>,
    pub scope: Option<signals::InstrumentationScope>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct ProfilesInput {
    pub dictionary: signals::ProfilesDictionary,
    pub profiles: Vec<signals::Profile>,
    pub resource: Option<signals::Resource>,
    pub scope: Option<signals::InstrumentationScope>,
}

/// Canonical, validated, durable form.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmissionEnvelope {
    pub version: EnvelopeVersion,
    pub record_key: Option<RecordKey>,
    pub logs: Vec<signals::ResourceRecord<signals::LogPoint>>,
    pub spans: Vec<signals::ResourceRecord<signals::SpanPoint>>,
    pub metrics: Vec<signals::ResourceRecord<signals::MetricStream>>,
    pub profiles: Option<ProfilesSubmission>,
}
#[non_exhaustive]
pub struct ProfilesSubmission {
    pub dictionary: signals::ProfilesDictionary,
    pub profiles: Vec<signals::ResourceRecord<signals::Profile>>,
}

/// Generated values. Golden fixtures use a deterministic source whose values
/// appear as `$GENERATED_TRACE_ID`, `$GENERATED_SPAN_ID` and `$GENERATED_NOW`
/// placeholders (placeholder equality, not literal equality).
pub trait IdSource {
    fn trace_id(&mut self) -> TraceId;
    fn span_id(&mut self) -> SpanId;
    fn now(&mut self) -> Timestamp;
}

/// Production source used by both front ends (d-30, d-31). std only, no
/// dependency: IDs come from `RandomState` hashing of a process-wide counter
/// (the scheme `sc-observability-log` `context.rs` uses), never all-zero;
/// `now` is `SystemTime::now()`.
#[derive(Debug, Default)]
pub struct SystemIds;
impl IdSource for SystemIds { /* ... */ }

impl SubmissionEnvelope {
    /// Single shared validation/correlation path used by Python and the CLI.
    /// - Rejects version > CURRENT (UnsupportedVersion).
    /// - Logs and spans sharing a correlation_id share the span's IDs; supplied
    ///   inconsistent IDs -> CorrelationConflict.
    /// - Span timing per SpanInput -> TimingConflict / Validation.
    /// - A log without an actual start time never becomes a span.
    /// - UInt > i64::MAX -> ValueOutOfRange; StringIndex outside profiles ->
    ///   Validation; ProfilesDictionary::validate_references failure ->
    ///   DictionaryReference.
    /// - At least one signal present, else EmptySubmission.
    pub fn from_input(input: SubmissionInput, ids: &mut dyn IdSource)
        -> Result<Self, SubmissionError>;
    /// serde_json syntax/EOF errors -> InvalidJson. serde_json data errors,
    /// including every SignalValidationError raised by a `try_from`
    /// deserializer, -> Validation { path: "<line>:<column>" } with the
    /// validation message in the context. Then from_input.
    pub fn from_json(json: &str, ids: &mut dyn IdSource) -> Result<Self, SubmissionError>;
    /// Deterministic canonical JSON (sorted keys, UTC RFC 3339 nanos) - the golden-fixture form.
    pub fn to_canonical_json(&self) -> String;
    pub fn signals(&self) -> SignalSet;
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Signal { Logs, Traces, Metrics, Profiles }

#[non_exhaustive]
pub struct AdmissionReceipt {
    pub submission_id: SubmissionId,
    pub record_key: Option<RecordKey>,
    pub admitted_at: Timestamp,
    pub signals: Vec<Signal>,
    pub duplicate: bool,
}

#[non_exhaustive]
pub enum DeliveryState {
    Pending,
    Claimed { holder: String, attempts: u32 },
    RetryScheduled { attempts: u32, next_attempt_at: Timestamp, last_error: ErrorCode },
    Delivered { at: Timestamp, attempts: u32 },
    Failed { attempts: u32, error: ErrorCode },        // terminal backend rejection (e.g. 400)
    EvictedByDiskBound { at: Timestamp },               // only under DiskBoundPolicy::EvictOldest
}
#[non_exhaustive]
pub struct DeliveryStatus { pub submission_id: SubmissionId, pub signals: Vec<(Signal, DeliveryState)> }

#[non_exhaustive]
pub struct StoreStatus {
    pub schema_version: u32,
    pub store_bytes: u64,
    pub max_store_bytes: u64,
    pub pending: SignalCounts,
    pub retry_scheduled: SignalCounts,
    pub delivered_retained: SignalCounts,
    pub failed: SignalCounts,
    pub evicted_by_disk_bound: u64,
    pub rejected_by_disk_bound: u64,
    pub unreadable_newer_envelopes: u64,
    pub lease: Option<LeaseInfo>,
    pub submissions: Vec<DeliveryStatus>, // only for StatusQuery::Submissions
}
#[non_exhaustive] pub enum StatusQuery { Summary, Submissions(Vec<SubmissionId>), RecordKeys(Vec<RecordKey>) }
/// Counts cover only the rows in the call's scope (see "Flush and shutdown results").
#[non_exhaustive]
pub struct FlushReport { pub delivered: SignalCounts, pub still_pending: SignalCounts,
    pub failed: SignalCounts, pub evicted: SignalCounts }
```

Supporting types, all `#[non_exhaustive]`:

```rust
/// Mirrors otlp's `ExporterBackend`; the durable client maps it in
/// `durable::adapter`.
pub enum ExporterBackendId { SyncHttp, OpenTelemetrySdk }
pub enum Representation { Log, Span, Gauge, Sum, Histogram, ExponentialHistogram, Summary, Exemplar, Profile }
pub struct SignalSet(/* bitset of Signal */);
pub struct SignalCounts { pub logs: u64, pub traces: u64, pub metrics: u64, pub profiles: u64 }
pub struct LeaseInfo { pub holder: String, pub expires_at: Timestamp }
pub struct Secret(String);                                       // redacted Debug/Display
/// One `Option` per `TelemetryClientConfig` field (caller args / CLI flags).
pub struct ConfigOverrides {
    pub service_name: Option<String>, pub endpoint: Option<String>,
    pub backend: Option<ExporterBackendId>, pub request_timeout: Option<Duration>,
    pub auth_header: Option<Secret>, pub store_path: Option<PathBuf>,
    pub max_store_bytes: Option<u64>, pub disk_bound_policy: Option<DiskBoundPolicy>,
    pub delivered_retention: Option<Duration>, pub record_key_retention: Option<Duration>,
    pub emit_flush_deadline: Option<Duration>, pub flush_deadline: Option<Duration>,
    pub lease_duration: Option<Duration>, pub sync_http_retry: Option<SyncHttpRetryPolicyDto>,
}
/// Field-for-field mirror of otlp's `SyncHttpRetryPolicy` (integrate/phase-d):
/// every field, same names, `DurationMs` carried as milliseconds.
pub struct SyncHttpRetryPolicyDto {
    pub max_retries: Option<u32>,
    pub initial_backoff_ms: Option<u64>,
    pub max_backoff_ms: Option<u64>,
    pub retry_sequence_timeout_ms: Option<u64>,
    pub retry_after_cap_ms: Option<u64>,
    pub retry_jitter_percent: Option<u8>,
}
```

Construction from other crates. The types above are `#[non_exhaustive]`, so
d-30 and d-31 build them only through these:

```rust
#[derive(Debug, Clone, Default)] pub struct ConfigOverrides { /* as above */ } // all None
impl<'a> ConfigSources<'a> {
    pub fn new(explicit: &'a ConfigOverrides, file: Option<&'a TelemetryFileConfig>,
               env: &'a dyn Fn(&str) -> Option<String>) -> Self;
}
impl Secret { pub fn new(value: String) -> Self; pub fn expose(&self) -> &str; }
impl FromStr for SubmissionId { type Err = SubmissionError; } // Validation { path: "submission_id" }
impl FromStr for RecordKey { type Err = SubmissionError; }    // Validation { path: "record_key" }
impl Display for SubmissionId, RecordKey;                      // the text form
impl Serialize for AdmissionReceipt, FlushReport, StoreStatus, DeliveryStatus,
    DeliveryState, SignalCounts, LeaseInfo;                    // the sc-otel.result/v1 field shapes
```

### Errors and codes

Every variant carries `context: Box<ErrorContext>`, matching the d-12
pattern. The codes are new `ErrorCode::new_static` constants in the crate's
registry module `crates/sc-observability-types/src/error_codes.rs`, plus an
enumerable registry, re-exported as
`sc_observability_types::otlp::submission::error_codes` (ADR-005). Defaults
and limits named in this doc are constants in
`crates/sc-observability-types/src/constants.rs`.

```rust
#[non_exhaustive]
pub enum SubmissionError {
    InvalidJson { context },          // SC_OBSERVABILITY_SUBMIT_INVALID_JSON
    UnsupportedVersion { found: EnvelopeVersion, context }, // SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION
    EmptySubmission { context },      // SC_OBSERVABILITY_SUBMIT_EMPTY
    Validation { path: String, context }, // SC_OBSERVABILITY_SUBMIT_VALIDATION
    ValueOutOfRange { path: String, context }, // SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE
    CorrelationConflict { context },  // SC_OBSERVABILITY_SUBMIT_CORRELATION_CONFLICT
    TimingConflict { context },       // SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT
    DictionaryReference { path: String, context }, // SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE
}
#[non_exhaustive]
pub enum AdmissionError {
    StoreUnavailable { context },     // SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE
    DiskBoundExceeded { context },    // SC_OBSERVABILITY_ADMIT_DISK_BOUND
    Persistence { context },          // SC_OBSERVABILITY_ADMIT_PERSISTENCE
    SchemaTooNew { found: u32, supported: u32, context }, // SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW
    Closed { context },               // SC_OBSERVABILITY_ADMIT_CLOSED
}
#[non_exhaustive]
pub enum DeliveryError {
    DeadlineExceeded { report: FlushReport, context }, // SC_OBSERVABILITY_DELIVERY_DEADLINE
    TerminalFailure { report: FlushReport, context },  // SC_OBSERVABILITY_DELIVERY_FAILED
}
#[non_exhaustive]
pub enum TelemetryConfigError {
    ConfigFile { path: PathBuf, context },   // SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE
    MissingField { field: &'static str, context }, // SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING
    InvalidField { field: &'static str, context }, // SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID
    UnsupportedCombination { backend: ExporterBackendId, signal: Signal,
        representation: Representation, context }, // SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED
}
#[non_exhaustive]
pub enum TelemetryClientError {
    Submission(SubmissionError),
    Admission(AdmissionError),
    Delivery(DeliveryError),
    Config(TelemetryConfigError),
}
impl TelemetryClientError { pub fn code(&self) -> &ErrorCode; }
```

The types crate carries no CLI exit policy. The exit-code mapping is the
table in "CLI contract", implemented by d-31 in `sc-otel-cli`.

### Configuration and precedence

```rust
#[non_exhaustive]
pub struct TelemetryClientConfig {
    pub service_name: String,
    pub endpoint: String,                  // OTLP/HTTP base, e.g. http://localhost:4318
    pub backend: ExporterBackendId,        // default SyncHttp
    pub request_timeout: Duration,         // per-export timeout -> OtelConfig.timeout_ms
    pub auth_header: Option<Secret>,       // never from checked-in YAML; never Debug-printed
    pub store_path: PathBuf,
    pub max_store_bytes: u64,
    pub disk_bound_policy: DiskBoundPolicy,
    pub delivered_retention: Duration,
    pub record_key_retention: Duration,
    pub emit_flush_deadline: Duration,     // CLI emit
    pub flush_deadline: Duration,
    pub lease_duration: Duration,          // renewed every lease_duration / 3
    pub sync_http_retry: Option<SyncHttpRetryPolicyDto>,
}
#[non_exhaustive] pub enum DiskBoundPolicy { RejectNew, EvictOldest }

#[non_exhaustive]
pub struct ConfigSources<'a> {
    pub explicit: &'a ConfigOverrides,         // caller args / CLI flags
    pub file: Option<&'a TelemetryFileConfig>, // parsed telemetry.yaml
    pub env: &'a dyn Fn(&str) -> Option<String>,
}
/// Precedence per field: explicit > telemetry.yaml > environment > built-in
/// default, restricted to the sources listed for that field below.
pub fn resolve_config(sources: ConfigSources<'_>) -> Result<TelemetryClientConfig, TelemetryConfigError>;

/// Core keys of telemetry.yaml; serde `Deserialize`, unknown keys ignored.
/// `base_dir` is the YAML file's directory, set by the loader.
#[non_exhaustive]
pub struct TelemetryFileConfig {
    pub base_dir: PathBuf,
    pub service: Option<String>,
    pub otlp: Option<FileOtlp>,     // endpoint, timeout_ms
    pub store: Option<FileStore>,   // path, max_bytes, disk_bound_policy, delivered_retention_hours
}

// crates/sc-observability-otlp/src/durable/config_file.rs (feature durable-store)
// d-29 stages the signature (stub returns ConfigFile { path }); d-33 implements it.
pub fn load_telemetry_file(path: &Path) -> Result<TelemetryFileConfig, TelemetryConfigError>;
```

Sources per field ("—" means that source cannot set the field):

| Field | Explicit (`ConfigOverrides`) | telemetry.yaml key | Environment | Default |
| --- | --- | --- | --- | --- |
| `service_name` | yes | `service` | `OTEL_SERVICE_NAME` | `unknown_service` |
| `endpoint` | yes | `otlp.endpoint` | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` |
| `backend` | yes | — | — | `SyncHttp` |
| `request_timeout` | yes | `otlp.timeout_ms` | — | 10 s |
| `auth_header` | yes | — (never from YAML) | `SC_OTEL_AUTH_HEADER` | none |
| `store_path` | yes | `store.path` (relative to the YAML file's directory) | — | none: `MissingField` |
| `max_store_bytes` | yes | `store.max_bytes` | — | 256 MiB |
| `disk_bound_policy` | yes | `store.disk_bound_policy` (`reject_new`\|`evict_oldest`) | — | `RejectNew` |
| `delivered_retention` | yes | `store.delivered_retention_hours` | — | 24 h |
| `record_key_retention` | yes | — | — | 30 days |
| `emit_flush_deadline` | yes | — | — | 5 s |
| `flush_deadline` | yes | — | — | 30 s |
| `lease_duration` | yes | — | — | 30 s |
| `sync_http_retry` | yes | — | — | none (otlp's documented retry defaults) |

`sc-observability-types` has no YAML dependency. The YAML parser is
`serde-saphyr =1.3.0` (MIT OR Apache-2.0, MSRV 1.89), used only by
`load_telemetry_file` under `durable-store`. `serde_yaml` is deprecated
upstream. On `feat/qa-sanity-telemetry-config`, nothing parses
`.sc/telemetry.yaml` yet; the d-35 Python importer reads `sources[]` with the
repository's existing PyYAML (`yaml.safe_load`). Both the CLI `--config` flag
and Python `Telemetry.open(config=...)` go through `load_telemetry_file`, so the
Rust side needs its own parser (lead ruling P4).

The keys read from telemetry.yaml are exactly the ones in the table. Every
other key (`team`, `github.*`, `sources[]`) belongs to the consumer and is
ignored by the core without error.

### TelemetryClient

```rust
pub trait TelemetryClient: Send + Sync {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> where Self: Sized;
    /// Validates (already canonical), durably commits, then returns the receipt.
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError>;
    /// Store-wide flush up to `deadline`; scope and result rules below.
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    /// Flush only the rows of one submission (used by `sc-otel emit`).
    fn flush_submission(&self, id: &SubmissionId, deadline: Duration)
        -> Result<FlushReport, TelemetryClientError>;
    /// Flush, stop the drain worker, release the lease. Idempotent.
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError>;
}
```

**Flush and shutdown results.** One rule applies to `flush`,
`flush_submission` and `shutdown`:

- Scope. For `flush` and `shutdown`, the scope is every delivery row that was
  not terminal (`delivered`, `failed` or `evicted`) when the call started.
  Older terminal rows are outside the scope, so one historical failure never
  fails a later flush. For `flush_submission(id)`, the scope is every delivery
  row of that submission, whatever its state at the start.
- Result. `Err(Delivery(TerminalFailure { report }))` if any row in scope is
  `failed` or `evicted` when the call returns. Otherwise
  `Err(Delivery(DeadlineExceeded { report }))` if any row in scope is not
  terminal at the deadline. Otherwise `Ok(report)`. TerminalFailure takes
  precedence over DeadlineExceeded (CLI exit 7 over 6).
- `report` counts only rows in scope.
- `shutdown` stops the worker and releases the lease even when it returns
  `Err`. A second `shutdown` returns `Ok` with an empty report.

**Test double.** `InMemoryTelemetryClient` (`sc-observability-types` feature
`test-double`) implements this trait with the same receipt, duplicate-key,
status and flush-result semantics. It records envelopes. Its delivery and
admission outcomes are scripted:

```rust
/// Also the JSON schema of the file named by SC_OTEL_TEST_DOUBLE (d-31) and of
/// the Python test-hooks script argument (d-30). deny_unknown_fields.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DoubleScript {
    /// Consumed in order by `emit`; when exhausted, emit admits.
    #[serde(default)] pub admissions: Vec<ScriptedAdmission>,
    /// Consumed in order per signal by delivery; when exhausted, rows deliver.
    #[serde(default)] pub deliveries: Vec<ScriptedDelivery>,
    /// Every flush, flush_submission and shutdown call sleeps this long before
    /// it evaluates results (GIL-release proof).
    #[serde(default)] pub flush_delay_ms: u64,
}
#[non_exhaustive]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ScriptedAdmission { Admit, Reject { kind: AdmissionErrorKind } }
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum AdmissionErrorKind { StoreUnavailable, DiskBoundExceeded, Persistence, SchemaTooNew, Closed }
#[non_exhaustive]
pub struct ScriptedDelivery { pub signal: Signal, pub outcome: DeliveryOutcome }
/// Deliver -> delivered; Fail -> failed (code SC_OBSERVABILITY_TEST_DOUBLE_SCRIPTED_FAILURE);
/// Stall -> stays pending, so the call reaches its deadline.
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum DeliveryOutcome { Deliver, Fail, Stall }

impl DoubleScript { pub fn from_json(json: &str) -> Result<Self, TelemetryConfigError>; }
impl InMemoryTelemetryClient {
    pub fn with_script(config: TelemetryClientConfig, script: DoubleScript) -> Self;
    pub fn push_script(&self, script: DoubleScript);
    pub fn deliver_all(&self);                         // clears scripted deliveries
    pub fn fail_next(&self, signal: Signal, code: ErrorCode);
    pub fn envelopes(&self) -> Vec<SubmissionEnvelope>;
}
```

`open(config)` is `with_script(config, DoubleScript::default())`. It is the
only double.

**Conformance suite.** `testing::conformance::run_all` holds the trait-level
cases. It drives any client through a harness:

```rust
pub trait ConformanceHarness {
    type Client: TelemetryClient;
    /// A fresh client over an empty store (or a fresh double).
    fn open(&mut self) -> Self::Client;
    /// What the backend does with the next delivery attempt for `signal`.
    /// The double scripts it; d-33 configures its `ScriptedExporter`
    /// (Deliver = Ok, Fail = Terminal, Stall = blocks past the deadline).
    fn set_outcome(&mut self, signal: Signal, outcome: DeliveryOutcome);
}
pub fn run_all<H: ConformanceHarness>(harness: &mut H);
```

The double passes `run_all` in `tests/otlp_submission_contract.rs`, and d-33
runs the same function against `DurableTelemetryClient`. Cases:
`duplicate_record_key_returns_original_receipt`, `status_reports_scripted_failure`,
`flush_ok_when_all_delivered`, `flush_deadline_when_pending`,
`flush_terminal_when_failed_during_call`, `flush_ignores_historical_failure`,
`terminal_precedes_deadline`, `flush_submission_scoped_to_id`,
`flush_submission_reports_prior_failure_of_same_submission` and
`shutdown_idempotent`.

**Python GIL release.** These calls release the GIL: `emit` (durable commit),
`flush`, `flush_submission`, `shutdown`, `status` and `open` (store open and
migration check).

### CLI contract (`sc-otel`)

Global flags: `--config <telemetry.yaml>`, `--store <path>`,
`--endpoint <url>` and `--output json|text` (default `json`). Each becomes the
matching `ConfigOverrides` field (`--config` is passed to
`load_telemetry_file`).

| Subcommand | Flags | Behavior |
| --- | --- | --- |
| `emit` | `--stdin` (read one `SubmissionInput` JSON document from stdin); or one or more of `--log <json\|@file>`, `--span <json\|@file>`, `--metric <json\|@file>`, `--profile <json\|@file>`; plus `--record-key <key>` (fragment input only: with `--stdin` the document's own `record_key` applies and the flag is a usage error) and `--no-flush`. No input source is a usage error. | Flag fragments are a `LogInput`, `SpanInput`, `MetricInput` or `ProfilesInput` object (`@file` reads it from a file). They are assembled into one `SubmissionInput` (`version` = CURRENT, fragments appended to `logs`/`spans`/`metrics`, one `--profile` at most) and go through `SubmissionEnvelope::from_json`, exactly like `--stdin`. `--stdin` and fragment flags are mutually exclusive. After admission, `flush_submission(receipt.submission_id, emit_flush_deadline)` runs unless `--no-flush`. |
| `validate` | `--stdin`, or the same fragment flags | Prints the canonical envelope; opens no store. |
| `flush` | `--timeout <seconds>` (default `flush_deadline`) | Store-wide `flush`. |
| `status` | `--submission <id>` (repeatable) or `--record-key <key>` (repeatable), mutually exclusive | `status` with `Submissions`, `RecordKeys`, or `Summary` when neither flag is given. |

| Exit | Meaning | Source |
| --- | --- | --- |
| 0 | Success. For `emit`: admitted, and delivered within `emit_flush_deadline`, or admitted with `--no-flush`. | - |
| 1 | Unexpected internal error | panic guard |
| 2 | Usage error (bad flags) | argument parser (clap) |
| 3 | Invalid input | `SubmissionError::*` |
| 4 | Configuration error or unsupported combination | `TelemetryConfigError::*` |
| 5 | Admission failure (nothing admitted) | `AdmissionError::*` |
| 6 | Admitted but delivery not completed (pending remains) | `DeliveryError::DeadlineExceeded` |
| 7 | Admitted but a signal failed terminally (takes precedence over 6) | `DeliveryError::TerminalFailure` |

Exits 1 and 2 print a message on stderr and nothing on stdout. Every other
exit prints exactly one `sc-otel.result/v1` object.

JSON output schema (`sc-otel.result/v1`), one object on stdout:

```json
{
  "schema": "sc-otel.result/v1",
  "command": "emit",
  "exit_code": 6,
  "state": "admitted_pending",
  "receipt": {"submission_id": "0192...", "record_key": "sanity:run-1:sanity-llm",
              "admitted_at": "2026-10-01T12:00:00.000000000Z", "signals": ["logs","traces"], "duplicate": false},
  "flush": {"delivered": {"logs": 1}, "still_pending": {"traces": 1}, "failed": {}, "evicted": {}},
  "status": null,
  "envelope": null,
  "error": {"code": "SC_OBSERVABILITY_DELIVERY_DEADLINE", "message": "..."}
}
```

`state` is one of `validated`, `admitted_delivered`, `admitted_pending`,
`admitted_failed`, `rejected` or `status`. Credentials never appear in output
or errors.

### Store schema and versioning (`durable/schema.sql`)

```sql
PRAGMA user_version = 1;
CREATE TABLE store_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE submissions (
  submission_id TEXT PRIMARY KEY,
  record_key TEXT UNIQUE,
  envelope_version INTEGER NOT NULL,
  envelope BLOB NOT NULL,              -- SubmissionEnvelope::to_canonical_json
  envelope_bytes INTEGER NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL
);
CREATE TABLE signal_deliveries (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  signal TEXT NOT NULL CHECK (signal IN ('logs','traces','metrics','profiles')),
  state TEXT NOT NULL CHECK (state IN ('pending','claimed','retry','delivered','failed','evicted')),
  attempts INTEGER NOT NULL DEFAULT 0,
  claimed_by TEXT,
  claim_expires_at_unix_nano INTEGER,
  next_attempt_at_unix_nano INTEGER,
  last_error_code TEXT,
  delivered_at_unix_nano INTEGER,
  PRIMARY KEY (submission_id, signal)
);
CREATE INDEX signal_deliveries_ready ON signal_deliveries (signal, state, next_attempt_at_unix_nano);
CREATE TABLE record_keys (record_key TEXT PRIMARY KEY, submission_id TEXT NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL);
CREATE TABLE drain_lease (id INTEGER PRIMARY KEY CHECK (id = 1), holder TEXT NOT NULL,
  acquired_at_unix_nano INTEGER NOT NULL, expires_at_unix_nano INTEGER NOT NULL);
CREATE TABLE store_counters (name TEXT PRIMARY KEY, value INTEGER NOT NULL);
```

Versioning policy:

- The schema version is `PRAGMA user_version`. Opening a store with a newer
  version returns `AdmissionError::SchemaTooNew` (reject-newer). The store is
  never modified in that case.
- An older version migrates forward inside one transaction. v1 has no
  predecessor.
- A row whose `envelope_version` is newer than `EnvelopeVersion::CURRENT` is
  never deleted. It is skipped and counted in `unreadable_newer_envelopes`.
- Connections use `journal_mode=WAL`, `synchronous=FULL` and
  `busy_timeout=5000`.

### Crate-private seams (staged for d-33 and d-34)

d-29 stages these items in `sc-observability-otlp`, so that d-33 routes the
drain through the existing sync-http bounded admission, d-34 adds the
OTLP/JSON export and profiles, and neither edits an unowned file or builds a
second queue (ADR-021). The drain and the exporter meet only at
`SubmissionExporter`, so d-33 and d-34 run in parallel. Each item that is
unused until d-33 or d-34 wires it carries
`#[expect(dead_code, reason = "staged by d-29; wired by d-33/d-34 under durable-store")]`.
d-33 and d-34 may change only the items listed in their docs.

| File | Item | d-29 change |
| --- | --- | --- |
| `src/lifecycle.rs` | `pub(crate) enum SignalKind` | Add `Profiles` (index 3). `LifecycleHealth.dropped_by_signal`/`degraded_by_signal` and the `CoreState` per-signal arrays grow from 3 to 4. `dropped_total()` keeps its meaning. Existing lifecycle tests unchanged except a 4th zero Profiles slot (lead `01M3VYE3D296E5M322VBAJKBF0`); SDK helper per-signal return types widen to four under `01M3VYFVM5W64ZWPXCCJ51T9M3`. |
| `src/contracts.rs` | module list | Two lines: `pub(crate) mod profiles;` and `pub(crate) mod submission;` (additive; d-18 fence, P2). `ExporterSet` is unchanged: it has nine construction sites, some inside the d-18 fence. |
| `src/contracts/profiles.rs` (new) | `pub(crate) trait ProfileExporter<T>: Send + Sync { fn export_profiles(&self, batch: &[T]) -> Result<(), ExportError>; }` | Same shape as `LogExporter`/`TraceExporter`/`MetricExporter`. d-34 implements it for the sync-http exporter, and `SyncHttpSubmissionExporter::export` dispatches profiles through it. |
| `src/contracts/submission.rs` (new) | `pub(crate) trait SubmissionExporter: Send + Sync { fn export(&self, signal: Signal, envelopes: &[SubmissionEnvelope]) -> Result<(), SubmissionExportFailure>; }` and `pub(crate) enum SubmissionExportFailure { Retryable(ExportError), Terminal(ExportError) }` | The only call the d-33 drain makes. `Retryable` leaves the row for retry; `Terminal` marks it failed. d-33 tests through a `ScriptedExporter`; d-34 implements the production exporter. |
| `src/contracts/credits.rs` | `impl AdmissionCredits { pub(crate) fn wait_for_release(&self, timeout: Duration) -> bool; }` | Signature only; the stub returns `false` (no release observed). d-33 implements it (the budget's `Condvar`, `CreditLease::drop` notification and their tests). |
| `src/sync_http/mod.rs` | module list | One line: `#[cfg(feature = "durable-store")] pub(crate) mod submission;`. |
| `src/sync_http/submission.rs` (new, staged) | `pub(crate) struct SyncHttpSubmissionExporter`, `pub(crate) fn exporter_for(config: SyncHttpConfig, bounds: ValidatedTransportBounds) -> Arc<dyn SubmissionExporter>` | `exporter_for` stages both the validated worker config and transport bounds; D34 wraps the existing sync-http exporter without changing this signature. The staged `export` returns `SubmissionExportFailure::Terminal` with `SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED`, so nothing reports false success before d-34 lands. d-34 implements it. |
| `src/durable/adapter.rs` (staged) | `pub(crate) fn otel_config_from(config: &TelemetryClientConfig) -> Result<OtelConfig, TelemetryConfigError>;` | Signature only; the stub body returns `TelemetryConfigError::InvalidField { field: "otlp" }`. d-33 implements it and then calls the existing `SyncHttpConfig::from_otel(&OtelConfig)`. Mapping: `backend` → `ExporterBackend`, `endpoint`, `auth_header`, `request_timeout` → `timeout_ms`, `sync_http_retry` → `SyncHttpRetryPolicy` field for field. |
| `src/durable/mod.rs` (staged) | `DurableTelemetryClient::open` | Evaluates `adapter::otel_config_from` and, on success, `exporter_for(worker, bounds)` from the destructured `SyncHttpConfig::from_otel(..)` result, discards both and returns `AdmissionError::StoreUnavailable`, so neither staged function is dead code under `durable-store`. d-33 implements it. |
| `src/constants.rs` | wave-5 entries | `DRAIN_BATCH_SIZE`, the lease renewal divisor, the store `busy_timeout` (5000 ms) and `PROFILES_EXPORT_PATH = "/v1development/profiles"` (ADR-005). |
| `src/error_codes.rs` | wave-5 entries | `SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED`, added to the crate's enumerable registry. |

Lead correction (2026-10-01, `01M3VY0G7ABHB7QBG5GBKYJWYA`):
`SyncHttpConfig::from_otel` was test-gated. D29 changes only that gate to
`cfg(any(test, feature = "durable-store"))`, preserving its existing body and
`(SyncHttpConfig, ValidatedTransportBounds)` result; callers destructure it.
The related imports of `OtelConfig`, `prepared_backend_connection`, and
`validated_transport_bounds` receive the same gate; `ExporterBackend` and
`OtlpProtocol` remain test-only (lead `01M3VY4ZQK7NR2NKTV0BHNJ2GG`).
Finally `config/mod.rs` reexports `validated_transport_bounds` under that gate,
leaving `validate_config_typed` test-only (lead `01M3VY7JWH90RDXYDQV8GNYT0J`).
These are the only three gate adjustments; validation bodies are unchanged.
Lead `01M3VYZAQDEPX67M2180KKGH45` freezes `exporter_for(config, bounds)` now so D33 and D34 can implement in parallel without changing their shared call signature. The staged wrapper retains both and always returns terminal UNWIRED.

### Dependency set

These are the only dependencies wave 5 adds. Pins and licenses were checked
against crates.io on 2026-10-01. Wave-5.2 sprints add none (see Handoffs).

Workspace `Cargo.toml`:

| Change | Entry | License / MSRV | Used by |
| --- | --- | --- | --- |
| member | `crates/sc-otel-cli` | — | — |
| new pin | `uuid = { version = "=1.26.1", default-features = false, features = ["std", "v7"] }` | Apache-2.0 OR MIT / 1.85. `v7` enables `rng`, which uses `getrandom` 0.4 and resolves to the existing `=0.4.3` | types (`test-double`), otlp (`durable-store`) |
| new pin | `rusqlite = { version = "=0.40.2", default-features = false, features = ["bundled"] }` | MIT; builds `libsqlite3-sys 0.38.2` (SQLite: public domain) | otlp (`durable-store`) |
| new pin | `serde-saphyr = { version = "=1.3.0", default-features = false, features = ["deserialize"] }` | MIT OR Apache-2.0 / 1.89 | otlp (`durable-store`) |
| new pin | `clap = { version = "=4.6.7", default-features = false, features = ["std", "derive", "help", "usage", "error-context"] }` | MIT OR Apache-2.0 / 1.85 | sc-otel-cli |
| reused | `serde_json = "1"` (lock 1.0.151), `tempfile = "3"` (lock 3.27.0), `getrandom = "=0.4.3"` | unchanged | — |

All MSRVs are at or below the workspace `rust-version` 1.94.1.

Per crate:

| Crate | Section | Entry |
| --- | --- | --- |
| `sc-observability-types` | `[dependencies]` | `uuid = { workspace = true, optional = true }` |
| | `[features]` | `test-double = ["dep:uuid"]` |
| `sc-observability-otlp` | `[dependencies]` | `rusqlite`, `serde-saphyr`, `uuid`, each `{ workspace = true, optional = true }` |
| | `[features]` | `durable-store = ["sync-http", "dep:rusqlite", "dep:serde-saphyr", "dep:uuid"]` |
| | `[dev-dependencies]` | `sc-observability-types = { workspace = true, features = ["test-double"] }`, `tempfile.workspace = true` (existing dev rows unchanged) |
| `sc-observability-py` | `[dependencies]` | `sc-observability-otlp = { workspace = true, optional = true }` |
| | `[features]` | `otlp-telemetry = ["dep:sc-observability-otlp", "sc-observability-otlp/durable-store"]`; `test-hooks` = existing entry + `"sc-observability-types/test-double"` |
| `sc-otel-cli` (new) | `[package]` | workspace-inherited fields, `publish = false`; `[[bin]] name = "sc-otel"` |
| | `[dependencies]` | `sc-observability-types.workspace = true`, `sc-observability-otlp = { workspace = true, features = ["durable-store"] }`, `clap.workspace = true`, `serde_json.workspace = true` |
| | `[features]` | `test-double = ["sc-observability-types/test-double"]` |
| | `[dev-dependencies]` | `tempfile.workspace = true` |

Hand-rolled, with no dependency:

- Base64 for `bytesValue`: a private RFC 4648 encoder in d-34's sync-http
  encoder.
- Lease holder ID: `<pid>:<uuid v7>` (`std::process::id()` plus `uuid`), no
  hostname.
- OTLP/JSON decoding in d-34's round-trip tests: a d-34-owned proto-JSON
  reader over `serde_json::Value` at
  `crates/sc-observability-otlp/src/sync_http/submission/tests/proto_json.rs`. The
  `opentelemetry-proto =0.33.0` `with-serde` decoders accept `"NaN"`,
  `"Infinity"` and `"-Infinity"` only for `ValueAtQuantile.quantile` and
  `.value`; every other double (`asDouble`, histogram `sum`/`min`/`max`,
  `explicitBounds`, `doubleValue`, exemplars) rejects them, and the
  `AnyValue` decoder ignores `stringValueStrindex`. So no
  `opentelemetry-proto` dev-dependency is added.
- Loopback HTTP capture: a d-34-owned `std::net::TcpListener` HTTP/1.1
  server at `crates/sc-observability-otlp/src/sync_http/submission/tests/capture.rs`.

`policy/otlp-transport.toml` additions:

```toml
[transport.rusqlite]
version = "=0.40.2"
backends = ["durable-store"]
features = ["bundled"]
default_features = false

[transport.serde-saphyr]
version = "=1.3.0"
backends = ["durable-store"]
features = ["deserialize"]
default_features = false

[transport.uuid]
version = "=1.26.1"
backends = ["durable-store"]
features = ["std", "v7"]
default_features = false

[dev_dependencies]
# existing rows unchanged, plus:
sc-observability-types = { features = ["test-double"], default_features = true }
tempfile = { features = [], default_features = true }
```

`scripts/ci/otlp_dependencies.py` checks backend binding only for
`otlp-sdk` and `sync-http`, and the repository validators hard-code their
crate lists, so they do not see `sc-otel-cli`. d-29 does not edit those
scripts (wave-5 ruling R16). Instead, the `durable-store` binding is checked
by the named test `durable_store_binding` in
`crates/sc-observability-otlp/tests/contract_manifest.rs`. It runs
`cargo metadata --locked --format-version 1` (via the `CARGO` environment
variable) and asserts, with `serde_json`, that `rusqlite`, `serde-saphyr` and
`uuid` are optional dependencies of `sc-observability-otlp` enabled only
through `durable-store`, and that `durable-store` includes `sync-http`. The
`sc-otel-cli` edges are checked by the cargo-tree criteria below.

Boundary allowlists: `types.toml` `allowed_dependents` += `sc-otel-cli`,

`allowed_test_double_paths` += `crates/sc-observability-types/src/otlp/submission/testing/**`;
`otlp.toml` `allowed_dependents` += `sc-observability-py`, `sc-otel-cli`;
`python.toml` `allowed_dependencies` += `sc-observability-otlp`; new
`boundaries/sc-otel-cli/cli.toml` with `allowed_dependencies` =
[`sc-observability-types`, `sc-observability-otlp`],
`forbidden_edges` = `[{ from = "sc-otel-cli", to = "sc-observe" }]` and
`allowed_dependents` = [].

Boundary `allowed_dependencies` are first-party only; external pins are enforced by policy rows, the manifest test and cargo-tree checks (lead `01M3VYV0BND025EZH744N0SD9D`).

### Platform matrix and dependency audit

The audit config `policy/deny-durable-store.toml` exists because the
repository has no cargo-deny config, and `rusqlite` with `bundled` is the
first bundled C dependency in a released crate. It allows the licenses in
the `durable-store` and `sc-otel-cli` graphs, denies yanked crates and
advisories, and bans duplicate `libsqlite3-sys`. Its consumers are this
sprint's audit criterion and the d-32 re-run. It is retired once a
workspace-wide cargo-deny config covers the same graphs.

Platforms: linux x86_64 and aarch64, macOS x86_64 and arm64, windows x86_64
and arm64 (the d-10 wheel target). `.github/workflows/telemetry-platforms.yml`,
added by d-33 (wave-5 ruling R20), is `workflow_dispatch` only, with input `source_commit`. Its matrix:

| Target | Runner |
| --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` |
| `aarch64-apple-darwin` | `macos-14` |
| `x86_64-apple-darwin` | `macos-14` (cross target) |
| `x86_64-pc-windows-msvc` | `windows-2022` |
| `aarch64-pc-windows-msvc` | `windows-11-arm` |

Each cell runs, on toolchain 1.94.1,
`cargo check -p sc-observability-otlp --features durable-store --locked --target <t>`
and `cargo build -p sc-otel-cli --locked --target <t>`. The abi3 wheel matrix
is proven by d-30 through `b4a-python-distributions.yml`.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityTypes (D1, D2):
  `cargo test -p sc-observability-types --test otlp_signals_contract --locked`
  runs nonzero cases for: every `AnyValue` variant's serde round trip,
  including bytes, `uint` and `StringIndex`, in both the canonical and the
  plain input form; duplicate plain-map keys and `null` rejected;
  `OtlpDouble` round trips of NaN, `Infinity` and `-Infinity` through serde
  JSON (asserting the proto-JSON strings and bitwise equality) in `AnyValue`,
  `NumberValue`, histogram `sum`/`min`/`max`, summary values and exemplars;
  NaN rejected in `explicit_bounds`; each of the five metric point forms with
  temporality and monotonicity validation, including rejection through
  deserialization (not only `try_new`); exemplars;
  `ProfilesDictionary::validate_references` accepting a valid set and rejecting
  an out-of-range index, including an out-of-range `StringIndex` and
  `AttributeKey::Index`; `TraceState` grammar; and `TryFrom` conversions from
  `LogEvent`, `SpanRecord<SpanEnded>`, v2 `MetricRecord`, `OtlpResource` and
  `OtlpInstrumentationScope`.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D3, D4):
  `cargo test -p sc-observability-types --features test-double --locked --test otlp_submission_contract`
  passes every golden fixture. Each `input.json` is a `SubmissionInput`
  document; it canonicalizes to its `expected.envelope.json`, or fails with
  the code in `expected.error.json`. Fixtures that also have `flags.args`
  describe the equivalent `sc-otel` fragment flags. Fixtures include
  `uint_over_i64_max`, `correlation_conflict`, `timing_conflict`,
  `unsupported_version`, `strindex_outside_profiles`, `non_finite_doubles`,
  `plain_attribute_map`, `paired_log_span_generated_ids` (placeholder
  equality for `$GENERATED_TRACE_ID`, `$GENERATED_SPAN_ID` and
  `$GENERATED_NOW`), one fixture per signal and point form, and the error
  fixtures `histogram_bucket_count_mismatch`, `summary_quantile_out_of_range`,
  `monotonic_sum_negative`, `delta_empty_interval` and
  `exponential_scale_out_of_range` (each `SC_OBSERVABILITY_SUBMIT_VALIDATION`)
  and `profile_index_out_of_range` (`SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE`).
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D3): the precedence tests
  cover every row of the per-field source table: each source that can set a
  field wins over the lower ones, and a source marked "—" is ignored for that
  field. Auth comes only from explicit input or env; `store_path` missing
  gives `MissingField`; `SyncHttpRetryPolicyDto` carries all six fields; and
  the error-code registry is unique.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D4): `run_all` passes against
  the double, covering every named conformance case, including each flush
  result rule and the TerminalFailure-over-DeadlineExceeded precedence. The
  scripted-double cases also pass: `scripted_admission_rejection_each_kind`,
  `scripted_stall_yields_deadline`, `scripted_fail_yields_terminal`,
  `flush_delay_applies_to_flush_and_shutdown` (elapsed ≥ the scripted delay),
  `double_script_json_round_trip` and `double_script_unknown_field_rejected`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D5): `load_telemetry_file` and
  `TelemetryFileConfig` have the signatures in "Configuration and precedence", and the
  staged body returns `TelemetryConfigError::ConfigFile { path }` without
  reading the file. `durable/config_file.rs` has no YAML parsing and no test
  (d-33 adds both, with fixture YAML).
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D5):
  `cargo test -p sc-observability-otlp --features durable-store --locked --test contract_schema`
  loads `schema.sql` into an empty in-memory SQLite database and asserts
  `user_version = 1` and the table and index set.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D10): with `durable-store` and
  with default features, `cargo test -p sc-observability-otlp --locked --lib`
  passes the existing lifecycle tests. `SignalKind::Profiles`,
  `ProfileExporter`, `SubmissionExporter`, `SubmissionExportFailure`,
  `wait_for_release`, `SyncHttpSubmissionExporter`, `exporter_for` and
  `otel_config_from` exist with the signatures in "Crate-private seams". The
  diff against the d-29 base is exactly two added lines in `contracts.rs` and
  one in `sync_http/mod.rs`. The staged `export` returns
  `SubmissionExportFailure::Terminal` with
  `SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED`.
- [ ] boundary:ADR-005 (D3, D10): every new error code is defined in
  `crates/sc-observability-types/src/error_codes.rs` or
  `crates/sc-observability-otlp/src/error_codes.rs` and is listed in that
  crate's registry; every new default and limit is defined in the matching
  `src/constants.rs`.
  `rg -n 'ErrorCode::new_static' crates/sc-observability-types/src/otlp crates/sc-observability-otlp/src/durable crates/sc-observability-otlp/src/sync_http crates/sc-observability-otlp/src/contracts`
  finds nothing.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D5, D6, D10):
  `cargo check --workspace --all-features --locked`,
  `cargo check -p sc-observability-otlp --features durable-store --tests --locked`,
  `cargo check -p sc-observability-py --features otlp-telemetry --locked`,
  `cargo check -p sc-observability-py --features test-hooks,otlp-telemetry --tests --locked` and
  `cargo check -p sc-otel-cli --all-features --tests --locked` pass with the
  committed dependency set. The `durable/` and CLI stubs contain no `todo!`
  or `unimplemented!`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D6):
  `cargo test -p sc-observability-otlp --locked --test contract_manifest durable_store_binding`
  passes. The two scoped cargo-deny audits in Required validation pass.
  `bash scripts/ci/validate_repo_boundaries.sh` and
  `bash scripts/ci/validate_dependency_bans.sh` pass with the new
  `policy/otlp-transport.toml` rows (these scripts do not check
  `sc-otel-cli`; the next criterion does).
- [ ] boundary:BOUNDARY-ScOtelCli (D6): `cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'`
  lists exactly `sc-otel-cli`, `sc-observability-types`,
  `sc-observability-otlp`, `clap` and `serde_json` (versions aside), and
  `cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'`
  contains no `sc-observe`, `pyo3` or `agent-team-mail`. Neither
  `sc-otel-cli` nor `sc-observability-py` declares `tokio` directly, so no
  front end needs a caller-owned runtime (the runtime-free behavior itself is
  proven by d-31 and d-32).
- [ ] boundary:ADR-021 (D9): the approval file records, for each crate whose
  surface changes (`sc-observability-types`, `sc-observability-otlp`) and
  for `sc-observability-py`, the `api_sha256` from
  `python3 scripts/ci/validate_public_api.py diff --crate <crate>`
  (`target/public-api/public-api-diff.json`) at the d-29 head, plus
  `feature_api_sha256` entries computed as
  `cargo public-api --manifest-path <manifest> -sss --features <f> | shasum -a 256`
  for `test-double` (types), `durable-store` (otlp) and `otlp-telemetry`
  (py).
- [ ] boundary:ADR-021 (D7): `bash scripts/ci/validate_docs_consistency.sh`
  passes, and ADR-021, PHD-005–013 and the §6 rows match this doc.

## Required validation

```sh
cargo fmt --check --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check --workspace --all-features --locked
cargo check -p sc-observability-otlp --features durable-store --tests --locked
cargo check -p sc-observability-py --features test-hooks,otlp-telemetry --tests --locked
cargo check -p sc-otel-cli --all-features --tests --locked
cargo test -p sc-observability-types --test otlp_signals_contract --locked
cargo test -p sc-observability-types --features test-double --locked --test otlp_submission_contract
cargo test -p sc-observability-otlp --locked --lib
cargo test -p sc-observability-otlp --features durable-store --locked --lib
cargo test -p sc-observability-otlp --features durable-store --locked --test contract_schema
cargo test -p sc-observability-otlp --locked --test contract_manifest durable_store_binding
cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'
cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'
cargo deny --manifest-path crates/sc-observability-otlp/Cargo.toml --features durable-store --config policy/deny-durable-store.toml check licenses bans advisories
cargo deny --manifest-path crates/sc-otel-cli/Cargo.toml --all-features --config policy/deny-durable-store.toml check licenses bans advisories
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_docs_consistency.sh
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-types --crate sc-observability-otlp --crate sc-observability-py
```

## Closeout gate

The user signs `docs/api-approvals/phase-d-wave5-telemetry-submission.json`
(D9) before d-29 closes (lead ruling P6). This is a d-29 closeout gate, not a
plan blocker: d-33, d-34, d-30 and d-31 may start from the sanity-passed contract
while the signature is pending. The recorded hashes freeze the wave-5 public
surface: d-33, d-34, d-30 and d-31 each prove their crate's hashes still match
(wave-5 ruling R17), and any public addition returns to d-29 as a contract
change. d-32 re-runs the D18 gate against the signed record.

## Handoffs

- To d-33 (wave 5.2): `crates/sc-observability-otlp/src/durable/mod.rs`,
  `durable/adapter.rs` and `durable/config_file.rs`, staged by d-29 and owned
  by d-33 from wave 5.2; the `wait_for_release` body in
  `src/contracts/credits.rs`; and the staging attributes on
  `SubmissionExporter` and `SignalKind::Profiles`. `schema.sql` stays
  read-only; a change to it is a contract change routed to the lead.
- To d-34 (wave 5.2): `crates/sc-observability-otlp/src/sync_http/submission.rs`
  (and new files under `src/sync_http/submission/`), and the staging
  attribute on `ProfileExporter`.
- To d-31 (wave 5.2): `crates/sc-otel-cli/src/main.rs`, staged by d-29 and
  owned by d-31. `crates/sc-otel-cli/Cargo.toml` stays d-29's.
- Wave-5.2 sprints add no dependency and do not edit any `Cargo.toml`,
  `Cargo.lock` or `policy/otlp-transport.toml`. Everything they use is in
  "Dependency set". A missing dependency is a contract defect, routed to the
  lead.

### Compiler diagnostic fixture amendment (2026-10-01)

Lead approval 01M3VZ3XAV749YN0HVRA0T9XE5 permits only the four-line
rustc help/note insertion in
`crates/sc-observability-log/tests/ui/fixture_attachment_no_owner_authority.stderr`.
The new `TelemetryClient::shutdown` trait adds a compiler suggestion; all four
E0599 errors and their rejected authority operations remain unchanged.

### CI boundary schema amendment (2026-10-01)

Lead ruling `01M3WFKN13MNAX7JZKY6KZD6WQ` corrects the CLI boundary
`forbidden_edges` to workspace-package `{ from, to }` objects, as required
by CI-pinned sc-lint `ba2d9bf622c1604e3f017c728040906b90e71bce`.
The edge prohibits `sc-otel-cli` from depending on `sc-observe`. External
`pyo3` and `agent-team-mail-*` exclusions remain in dependency policy and
cargo-deny checks, rather than the first-party boundary table.

### Scoped sc-lint directives (2026-10-01)

Rand’s ruling relayed in `01M3WG0WNAM8Y0667TGQD6X2BY` adds the published
`sc-lint-attributes = "=0.4.0"` dependency to types and OTLP only.
`cycle.recursive_value_container` is allowed on `KeyValues` and `AnyValue`
because OTLP requires their recursive representation; the checker requires
all owners in this component to carry the directive.
`cycle.type_method_self_loop` is allowed only on the two client `open` methods
and the test-double `with_script` constructor, which return `Self`.
The dependency validators admit this compile-time attribute dependency; the
workspace and two standalone consumer-example lockfiles include its closure
so their existing `--locked` checks remain reproducible. Existing cargo-deny
license rules already cover it and require no change.
No data shapes or public signatures change.
